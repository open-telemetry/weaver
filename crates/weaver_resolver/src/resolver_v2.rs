// SPDX-License-Identifier: Apache-2.0

//! Synchronous caching schema resolver engine (`WeaverResolver`).

use std::num::NonZeroUsize;
use std::sync::Arc;
use lru::LruCache;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use crate::loader::LoadedSemconvRegistry;
use crate::Error;

/// A unified, version-agnostic bundle representing an optimized OpenTelemetry schema (either V1 or V2).
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum WeaverResolvedSchema {
    /// An optimized OpenTelemetry V1 schema.
    V1(V1Schema),
    /// An optimized OpenTelemetry V2 schema.
    V2(V2Schema),
}

impl WeaverResolvedSchema {
    /// Returns the active schema URL string for this bundle.
    pub fn schema_url_str(&self) -> &str {
        match self {
            Self::V1(s) => s.schema_url.as_str(),
            Self::V2(s) => s.schema_url.as_str(),
        }
    }

    /// Converts this bundle into an OpenTelemetry V1 schema if compatible, or returns an error.
    pub fn into_v1(self) -> Result<V1Schema, Error> {
        match self {
            Self::V1(s) => Ok(s),
            Self::V2(_) => Err(Error::ConvertingV2ToV1Unsupported),
        }
    }


    /// Returns an OpenTelemetry V1 schema reference if this bundle holds a V1 schema.
    pub fn as_v1(&self) -> Option<&V1Schema> {
        match self {
            Self::V1(s) => Some(s),
            Self::V2(_) => None,
        }
    }
}

/// Encapsulates all runtime configuration parameters for the Weaver resolution engine.
/// Designed to evolve over time without breaking caller API contracts.
#[derive(Debug, Clone)]
pub struct WeaverResolverConfig {
    /// Maximum number of fully resolved schemas to retain in the internal LRU cache.
    pub cache_capacity: NonZeroUsize,
    
    /// Whether to follow symbolic links during directory traversal.
    pub follow_symlinks: bool,

    /// Whether to include unreferenced groups in the resolved catalog.
    pub include_unreferenced: bool,
    
    /// HTTP authentication credentials resolver for remote registry fetches.
    pub auth: HttpAuthResolver,
}

impl Default for WeaverResolverConfig {
    fn default() -> Self {
        Self {
            cache_capacity: NonZeroUsize::new(32).expect("32 is valid non-zero capacity"),
            follow_symlinks: false,
            include_unreferenced: false,
            auth: HttpAuthResolver::empty(),
        }
    }
}

/// A visitor trait allowing callers to intercept key stages of the semantic convention loading
/// and resolution process, custom-process intermediate specifications, or capture external errors.
pub trait SchemaLoadingVisitor {
    /// Allows inspecting raw loaded files (unresolved specifications) immediately after successfully loading
    /// a semantic convention repository from disk or virtual directory, before final graph deduplication.
    ///
    /// If this method returns `Err(())`, resolution is instantly aborted.
    fn check_raw_loaded_schema_files(&mut self, _loaded: &LoadedSemconvRegistry) -> Result<(), ()> {
        Ok(())
    }
}

/// A default, no-op schema loading visitor that performs no actions.
#[derive(Debug, Clone, Default)]
pub struct DefaultSchemaVisitor;

impl SchemaLoadingVisitor for DefaultSchemaVisitor {}



/// A centralized, synchronous engine responsible for loading and resolving telemetry schemas,
/// populating an internal LRU cache, and automatically serving pre-resolved dependency schemas.
pub struct WeaverResolver {
    /// Bounded LRU cache mapping exact SchemaUrls to reference-counted resolved schema bundles.
    cache: LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>,
    
    /// Internal engine configuration.
    config: WeaverResolverConfig,
}

impl WeaverResolver {
    /// Instantiates a new WeaverResolver engine from explicit configuration settings.
    pub fn new(config: WeaverResolverConfig) -> Self {
        Self {
            cache: LruCache::new(config.cache_capacity),
            config,
        }
    }

    /// Loads a semantic convention repository without executing the final resolution step.
    ///
    /// Allows inspecting unresolved specifications or evaluating policy rules (e.g., BeforeResolution).
    fn load_repository(
        &mut self,
        registry_repo: RegistryRepo,
    ) -> WResult<LoadedSemconvRegistry, Error> {
        crate::loader::load_semconv_repository(
            registry_repo,
            self.config.follow_symlinks,
            &self.config.auth,
        )
    }

    /// Resolves an already loaded semantic convention repository, executing graph deduplication
    /// and populating the internal LRU cache.
    fn resolve_loaded(
        &mut self,
        loaded: LoadedSemconvRegistry,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        match loaded {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                dependencies,
            } => {
                let res = crate::SchemaResolver::resolve_registry(
                    repo,
                    specs,
                    imports,
                    dependencies,
                    self.config.include_unreferenced,
                );
                match res {
                    WResult::Ok(resolved) => {
                        let arc = Arc::new(WeaverResolvedSchema::V1(resolved));
                        if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                            _ = self.cache.put(url, arc.clone());
                        }
                        WResult::Ok(arc)
                    }
                    WResult::OkWithNFEs(resolved, nfes) => {
                        let arc = Arc::new(WeaverResolvedSchema::V1(resolved));
                        if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                            _ = self.cache.put(url, arc.clone());
                        }
                        WResult::OkWithNFEs(arc, nfes)
                    }
                    WResult::FatalErr(e) => WResult::FatalErr(e),
                }
            }
            LoadedSemconvRegistry::Resolved(schema) => {
                let arc = Arc::new(WeaverResolvedSchema::V1(schema));
                if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                    _ = self.cache.put(url, arc.clone());
                }
                WResult::Ok(arc)
            }
            LoadedSemconvRegistry::ResolvedV2(schema) => {
                let arc = Arc::new(WeaverResolvedSchema::V2(schema));
                if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                    _ = self.cache.put(url, arc.clone());
                }
                WResult::Ok(arc)
            }
        }
    }

    /// Dynamically resolves and caches a given SchemaUrl on demand.
    ///
    /// - If the schema exists in the internal LRU cache, its LRU order is refreshed
    ///   and a cloned Arc is returned immediately.
    /// - If missing, it fetches the definition repository, loads its dependency tree,
    ///   recursively calls `resolve_schema` to draw dependencies from this exact cache,
    ///   resolves the parent schema, caches it, and returns the reference-counted schema.
    ///
    /// Primary Consumers: `weaver_live_check` and `weaver_serve`.
    pub fn resolve_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        if let Some(cached) = self.cache.get(schema_url) {
            return WResult::Ok(cached.clone());
        }

        let dep = weaver_semconv::manifest::Dependency {
            schema_url: schema_url.clone(),
            registry_path: None,
        };
        let mut nfes = vec![];
        let repo = match RegistryRepo::try_new_dependency_with_auth(&dep, &mut nfes, &self.config.auth) {
            Ok(r) => r,
            Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
        };

        let res = self.load_and_resolve_schema(repo, DefaultSchemaVisitor);
        match res {
            WResult::Ok(_) => {
                let arc = self.cache.get(schema_url).expect("Just cached").clone();
                WResult::OkWithNFEs(arc, nfes.into_iter().map(Error::from).collect())
            }
            WResult::OkWithNFEs(_, more_nfes) => {
                let arc = self.cache.get(schema_url).expect("Just cached").clone();
                let mut all_nfes = nfes.into_iter().map(Error::from).collect::<Vec<_>>();
                all_nfes.extend(more_nfes);
                WResult::OkWithNFEs(arc, all_nfes)
            }
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    /// Loads and resolves a semantic convention repository directly from disk or virtual directory.
    ///
    /// - Evaluates the provided repository.
    /// - Executes any configured interceptor actions (e.g., BeforeResolution policy evaluation).
    /// - Draws any manifested dependencies directly from the internal cache.
    /// - Inserts the final resolved schema into the cache under its identified SchemaUrl.
    ///
    /// Primary Consumer: Weaver CLI (`weaver registry generate`, `weaver registry resolve`).
    pub fn load_and_resolve_schema<V: SchemaLoadingVisitor>(
        &mut self,
        registry_repo: RegistryRepo,
        mut visitor: V,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let (loaded, mut load_nfes) = match self.load_repository(registry_repo) {
            WResult::Ok(loaded) => (loaded, vec![]),
            WResult::OkWithNFEs(loaded, nfes) => (loaded, nfes),
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        };

        if visitor.check_raw_loaded_schema_files(&loaded).is_err() {
            return WResult::FatalErr(Error::LoadingAbortedByVisitor);
        }

        match self.resolve_loaded(loaded) {
            WResult::Ok(arc) => {
                let owned = Arc::unwrap_or_clone(arc);
                if load_nfes.is_empty() {
                    WResult::Ok(owned)
                } else {
                    WResult::OkWithNFEs(owned, load_nfes)
                }
            }
            WResult::OkWithNFEs(arc, res_nfes) => {
                load_nfes.extend(res_nfes);
                WResult::OkWithNFEs(Arc::unwrap_or_clone(arc), load_nfes)
            }
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    /// Manually injects a pre-resolved schema bundle into the LRU cache.
    /// Strictly restricted to unit and integration test suites.
    #[cfg(test)]
    pub fn cache_schema(
        &mut self,
        schema_bundle: WeaverResolvedSchema,
    ) -> WeaverResolvedSchema {
        let url = SchemaUrl::try_from(schema_bundle.schema_url_str())
            .expect("WeaverResolvedSchema contains valid schema_url");
        let arc = Arc::new(schema_bundle);
        _ = self.cache.put(url, arc.clone());
        Arc::unwrap_or_clone(arc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::group::GroupType;
    use weaver_semconv::registry_repo::RegistryRepo;

    #[test]
    fn test_weaver_resolver_v2_caching() {
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![]).expect("Failed to create RegistryRepo");

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let url = SchemaUrl::try_from(resolved.schema_url.as_str()).expect("Valid schema url");
        let cached1 = match resolver.resolve_schema(&url) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to get from cache: {e}"),
        };
        let cached2 = match resolver.resolve_schema(&url) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to get from cache: {e}"),
        };

        assert!(Arc::ptr_eq(&cached1, &cached2));
    }

    #[test]
    fn test_multi_registry_v2() -> Result<(), weaver_semconv::Error> {
        fn check_semconv_load_and_resolve(registry_repo: RegistryRepo, include_unreferenced: bool) {
            let config = WeaverResolverConfig {
                include_unreferenced,
                ..Default::default()
            };
            let mut resolver = WeaverResolver::new(config);
            let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
                WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
            };

            let resolved_registry = &resolved;

            if include_unreferenced {
                let group = resolved_registry.group("otel.unused");
                assert!(group.is_some());

                let group = resolved_registry.group("metric.example.counter");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.application");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.service");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.start");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.end");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.group");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.span");
                assert!(group.is_some());
            } else {
                let group = resolved_registry.group("otel.unused");
                assert!(group.is_none());
                let group = resolved_registry.group("event.session.end");
                assert!(group.is_none());

                let group = resolved_registry.group("metric.example.counter");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.application");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.service");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.start");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.group");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.span");
                assert!(group.is_some());
            }

            let metrics = resolved_registry.groups(GroupType::Metric);
            let metric = metrics
                .get("metric.auction.bid.count")
                .expect("Metric not found");
            let attributes = &metric.attributes;
            assert_eq!(attributes.len(), 3);
            let mut attr_names = HashSet::new();
            for attr_ref in attributes {
                let attr = resolved
                    .catalog
                    .attribute(attr_ref)
                    .expect("Failed to resolve attribute");
                _ = attr_names.insert(attr.name.clone());
                match attr.name.as_str() {
                    "auction.name" => {}
                    "auction.id" => {}
                    "error.type" => {
                        assert_eq!(
                            attr.requirement_level,
                            RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                        );
                        assert_eq!(attr.brief, "The error type.".to_owned());
                    }
                    _ => {
                        panic!("Unexpected attribute name: {}", attr.name);
                    }
                }
            }
            assert_eq!(metric.attributes.len(), 3);
            assert!(attr_names.contains("auction.name"));
            assert!(attr_names.contains("auction.id"));
            assert!(attr_names.contains("error.type"));
        }

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        check_semconv_load_and_resolve(registry_repo.clone(), false);
        check_semconv_load_and_resolve(registry_repo.clone(), true);
        Ok(())
    }

    #[test]
    fn test_three_registry_chain_works_v2() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig {
            follow_symlinks: true,
            ..Default::default()
        };
        let mut resolver = WeaverResolver::new(config);
        
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;

        let all_groups: Vec<String> = [
            GroupType::AttributeGroup,
            GroupType::Metric,
            GroupType::Event,
            GroupType::Span,
        ]
        .iter()
        .flat_map(|group_type| {
            resolved_registry
                .groups(group_type.clone())
                .keys()
                .map(|k| (*k).to_owned())
                .collect::<Vec<_>>()
        })
        .collect();

        assert_eq!(
            all_groups.len(),
            2,
            "Expected 2 groups (app.example and metric.example.counter), but found {}: {:?}",
            all_groups.len(),
            all_groups
        );
        assert!(
            all_groups.contains(&"app.example".to_owned()),
            "Missing app.example group, found: {all_groups:?}"
        );
        assert!(
            all_groups.contains(&"metric.example.counter".to_owned()),
            "Missing metric.example.counter group, found: {all_groups:?}"
        );

        let app_group = resolved_registry
            .group("app.example")
            .expect("app.example group should exist");

        let mut attr_names = HashSet::new();
        for attr_ref in &app_group.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute");
            let _ = attr_names.insert(attr.name.clone());
        }

        assert!(
            attr_names.contains("app.name"),
            "Missing app.name attribute"
        );
        assert!(
            attr_names.contains("error.type"),
            "Missing error.type attribute"
        );
        assert!(
            attr_names.contains("auction.name"),
            "Missing auction.name attribute"
        );
        assert_eq!(attr_names.len(), 3,
            "Expected exactly 3 attributes (app.name, error.type, auction.name), got: {attr_names:?}");
        Ok(())
    }

    #[test]
    fn test_v2_dependency_resolution_v2() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/consumer_registry".to_owned(),
        };

        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;
        let metrics = resolved_registry.groups(GroupType::Metric);
        let metric = metrics
            .get("metric.consumer.request.count")
            .expect("metric.consumer.request.count not found");

        assert_eq!(metric.attributes.len(), 2);

        let mut attr_names = HashSet::new();
        for attr_ref in &metric.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute ref");
            _ = attr_names.insert(attr.name.clone());
            match attr.name.as_str() {
                "server.address" => {
                    assert_eq!(
                        attr.requirement_level,
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                    );
                    assert_eq!(attr.brief, "Server address.");
                    assert_eq!(
                        attr.r#type,
                        weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                            weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String
                        )
                    );
                }
                "server.port" => {
                    assert_eq!(attr.brief, "The server port used by the consumer.");
                    assert_eq!(
                        attr.r#type,
                        weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                            weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::Int
                        )
                    );
                }
                _ => panic!("Unexpected attribute: {}", attr.name),
            }
        }

        assert!(attr_names.contains("server.address"));
        assert!(attr_names.contains("server.port"));
        Ok(())
    }

    #[test]
    fn test_v2_three_layer_dependency_resolution_v2() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;
        let metrics = resolved_registry.groups(GroupType::Metric);
        let metric = metrics
            .get("metric.app.request.count")
            .expect("metric.app.request.count not found");
        assert_eq!(metric.attributes.len(), 2);
        for attr_ref in &metric.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute ref");
            match attr.name.as_str() {
                "server.address" => assert_eq!(attr.brief, "Server address."),
                "server.port" => assert_eq!(attr.brief, "Server port."),
                _ => panic!("Unexpected attribute: {}", attr.name),
            }
        }
        Ok(())
    }
}
