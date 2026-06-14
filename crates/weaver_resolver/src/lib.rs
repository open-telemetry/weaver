// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::group::ImportsWithProvenance;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::semconv::SemConvSpecWithProvenance;

use crate::attribute::AttributeCatalog;
use crate::dependency::ResolvedDependency;
use crate::registry::resolve_registry_with_dependencies;

mod attribute;
mod dependency;
mod dependency_resolution;
mod error;
mod loader;
pub(crate) mod merge;
mod registry;

pub use crate::error::Error;
pub use crate::loader::LoadedSemconvRegistry;

// -----------------------------------------------------------------------------
// Core Enums and Traits
// -----------------------------------------------------------------------------

/// A unified, version-agnostic bundle representing an optimized OpenTelemetry schema (either V1 or V2).
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum WeaverResolvedSchema {
    /// An optimized OpenTelemetry V1 schema.
    V1(ResolvedTelemetrySchema),
    /// An optimized OpenTelemetry V2 schema.
    V2(V2Schema),
}

impl WeaverResolvedSchema {
    /// Returns the active schema URL string for this bundle.
    #[must_use]
    pub fn schema_url_str(&self) -> &str {
        match self {
            Self::V1(s) => s.schema_url.as_str(),
            Self::V2(s) => s.schema_url.as_str(),
        }
    }

    /// Converts this bundle into an OpenTelemetry V1 schema if compatible, or returns an error.
    pub fn into_v1(self) -> Result<ResolvedTelemetrySchema, Error> {
        match self {
            Self::V1(s) => Ok(s),
            Self::V2(_) => Err(Error::ConvertingV2ToV1Unsupported),
        }
    }

    /// Returns an OpenTelemetry V1 schema reference if this bundle holds a V1 schema.
    #[must_use]
    pub fn as_v1(&self) -> Option<&ResolvedTelemetrySchema> {
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

    /// Explicit overrides mapping a requested SchemaUrl to an alternative VirtualDirectoryPath.
    /// Used to redirect dependency graph requests to local clones, forks, or custom archives.
    pub schema_url_overrides: HashMap<SchemaUrl, weaver_common::vdir::VirtualDirectoryPath>,
}

impl Default for WeaverResolverConfig {
    fn default() -> Self {
        Self {
            cache_capacity: NonZeroUsize::new(32).expect("32 is valid non-zero capacity"),
            follow_symlinks: false,
            include_unreferenced: false,
            auth: HttpAuthResolver::empty(),
            schema_url_overrides: HashMap::new(),
        }
    }
}

/// A visitor trait allowing callers to intercept key stages of the semantic convention loading
/// and resolution process, custom-process intermediate specifications, or capture external errors.
pub trait SchemaLoadingVisitor {
    /// Allows inspecting raw loaded files (unresolved specifications) immediately after successfully loading
    /// a semantic convention repository from disk or virtual directory, before final graph deduplication.
    ///
    /// If this method returns `false`, resolution is instantly aborted.
    fn check_raw_loaded_schema_files(&mut self, _loaded: &LoadedSemconvRegistry) -> bool {
        true
    }
}

/// A default, no-op schema loading visitor that performs no actions.
#[derive(Debug, Clone, Default)]
pub struct DefaultSchemaVisitor;

impl SchemaLoadingVisitor for DefaultSchemaVisitor {}

// -----------------------------------------------------------------------------
// WeaverResolver Engine
// -----------------------------------------------------------------------------

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
    #[must_use]
    pub fn new(config: WeaverResolverConfig) -> Self {
        Self {
            cache: LruCache::new(config.cache_capacity),
            config,
        }
    }

    /// Configures an explicit location override for a specific SchemaUrl.
    /// When Weaver resolves this SchemaUrl, it will redirect the fetch to the target VirtualDirectoryPath.
    pub fn add_schema_url_override(
        &mut self,
        schema_url: SchemaUrl,
        target_path: weaver_common::vdir::VirtualDirectoryPath,
    ) {
        _ = self
            .config
            .schema_url_overrides
            .insert(schema_url, target_path);
    }

    /// Loads a semantic convention repository without executing the final resolution step.
    ///
    /// Allows inspecting unresolved specifications or evaluating policy rules (e.g., BeforeResolution).
    pub(crate) fn load_repository(
        &mut self,
        registry_repo: RegistryRepo,
    ) -> WResult<LoadedSemconvRegistry, Error> {
        loader::load_semconv_repository_with_cache(
            Some(&self.cache),
            registry_repo,
            self.config.follow_symlinks,
            &self.config.auth,
        )
    }

    /// Dynamically resolves a LoadedSemconvRegistry dependency, serving pre-resolved schemas from cache if available.
    fn resolve_dependency(
        &mut self,
        loaded: LoadedSemconvRegistry,
    ) -> WResult<ResolvedDependency, Error> {
        let schema_url = match &loaded {
            LoadedSemconvRegistry::Unresolved { repo, .. } => {
                if let Some(m) = repo.manifest() {
                    m.schema_url().clone()
                } else {
                    match SchemaUrl::try_from_name_version(repo.name(), repo.version()) {
                        Ok(url) => url,
                        Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
                    }
                }
            }
            LoadedSemconvRegistry::Resolved(s) => {
                match SchemaUrl::try_from(s.schema_url.as_str()) {
                    Ok(url) => url,
                    Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
                }
            }
            LoadedSemconvRegistry::ResolvedV2(s) => s.schema_url.clone(),
        };

        if let Some(cached) = self.cache.get(&schema_url) {
            match &**cached {
                WeaverResolvedSchema::V1(s) => return WResult::Ok(s.clone().into()),
                WeaverResolvedSchema::V2(s) => return WResult::Ok(s.clone().into()),
            }
        }

        let loaded = if let Some(override_path) = self.config.schema_url_overrides.get(&schema_url)
        {
            let mut nfes = vec![];
            match RegistryRepo::try_new_with_auth(
                Some(schema_url.clone()),
                override_path,
                &mut nfes,
                &self.config.auth,
            ) {
                Ok(repo) => match self.load_repository(repo) {
                    WResult::Ok(l) => l,
                    WResult::OkWithNFEs(l, _) => l,
                    WResult::FatalErr(e) => return WResult::FatalErr(e),
                },
                Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
            }
        } else {
            loaded
        };

        match self.resolve_loaded(loaded) {
            WResult::Ok(arc) => match &*arc {
                WeaverResolvedSchema::V1(s) => WResult::Ok(s.clone().into()),
                WeaverResolvedSchema::V2(s) => WResult::Ok(s.clone().into()),
            },
            WResult::OkWithNFEs(arc, nfes) => match &*arc {
                WeaverResolvedSchema::V1(s) => WResult::OkWithNFEs(s.clone().into(), nfes),
                WeaverResolvedSchema::V2(s) => WResult::OkWithNFEs(s.clone().into(), nfes),
            },
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    /// Actually resolves an internal definition registry and all its manifested dependencies.
    fn resolve_registry_internal(
        &mut self,
        repo: RegistryRepo,
        specs: Vec<SemConvSpecWithProvenance>,
        imports: Vec<ImportsWithProvenance>,
        dependencies: Vec<LoadedSemconvRegistry>,
    ) -> WResult<ResolvedTelemetrySchema, Error> {
        // First, let's make sure all dependencies are resolved.
        let mut opt_resolved_dependencies: Vec<WResult<ResolvedDependency, Error>> = vec![];
        for d in dependencies {
            opt_resolved_dependencies.push(self.resolve_dependency(d));
        }

        // Now resolve warnings/errors.
        let mut resolved_dependencies = vec![];
        let mut non_fatal_errors = vec![];
        for r in opt_resolved_dependencies {
            match r {
                WResult::Ok(d) => resolved_dependencies.push(d),
                WResult::OkWithNFEs(d, nfes) => {
                    resolved_dependencies.push(d);
                    non_fatal_errors.extend(nfes);
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        let manifest = repo.manifest().cloned();
        let schema_url = if let Some(m) = manifest.as_ref() {
            m.schema_url().clone()
        } else {
            match SchemaUrl::try_from_name_version(repo.name(), repo.version()) {
                Ok(url) => url,
                Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
            }
        };
        let mut attr_catalog = AttributeCatalog::default();

        let mut dependencies = std::collections::BTreeSet::new();
        for d in &resolved_dependencies {
            match d {
                ResolvedDependency::V1(schema) => {
                    if let Ok(url) = SchemaUrl::try_from(schema.schema_url.as_str()) {
                        _ = dependencies.insert(url);
                    }
                }
                ResolvedDependency::V2(schema) => {
                    _ = dependencies.insert(schema.schema_url.clone());
                }
            }
        }

        let include_unreferenced = self.config.include_unreferenced;
        resolve_registry_with_dependencies(
            &mut attr_catalog,
            repo,
            specs,
            imports,
            resolved_dependencies,
            include_unreferenced,
        )
        .map(move |resolved_registry| ResolvedTelemetrySchema {
            file_format: "1.0.0".to_owned(),
            schema_url: schema_url.as_str().to_owned(),
            registry_id: schema_url.name().to_owned(),
            registry: resolved_registry,
            catalog: attr_catalog.into(),
            resource: None,
            instrumentation_library: None,
            dependencies,
            versions: None,
            registry_manifest: manifest,
        })
    }

    /// Resolves an already loaded semantic convention repository, executing graph deduplication
    /// and populating the internal LRU cache.
    pub(crate) fn resolve_loaded(
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
                let res = self.resolve_registry_internal(repo, specs, imports, dependencies);
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

        let mut nfes = vec![];
        let repo = if let Some(override_path) = self.config.schema_url_overrides.get(schema_url) {
            match RegistryRepo::try_new_with_auth(
                Some(schema_url.clone()),
                override_path,
                &mut nfes,
                &self.config.auth,
            ) {
                Ok(r) => r,
                Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
            }
        } else {
            let dep = weaver_semconv::manifest::Dependency {
                schema_url: schema_url.clone(),
                registry_path: None,
            };
            match RegistryRepo::try_new_dependency_with_auth(&dep, &mut nfes, &self.config.auth) {
                Ok(r) => r,
                Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
            }
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

        if !visitor.check_raw_loaded_schema_files(&loaded) {
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
    pub fn cache_schema(&mut self, schema_bundle: WeaverResolvedSchema) -> WeaverResolvedSchema {
        let url = SchemaUrl::try_from(schema_bundle.schema_url_str())
            .expect("WeaverResolvedSchema contains valid schema_url");
        let arc = Arc::new(schema_bundle);
        _ = self.cache.put(url, arc.clone());
        Arc::unwrap_or_clone(arc)
    }
}

// -----------------------------------------------------------------------------
// Test Suites
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::group::{GroupType, ImportsWithProvenance};
    use weaver_semconv::registry_repo::RegistryRepo;

    #[test]
    fn test_weaver_resolver_caching() {
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to create RegistryRepo");

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
    fn test_multi_registry() -> Result<(), weaver_semconv::Error> {
        fn check_semconv_load_and_resolve(registry_repo: RegistryRepo, include_unreferenced: bool) {
            let config = WeaverResolverConfig {
                include_unreferenced,
                ..Default::default()
            };
            let mut resolver = WeaverResolver::new(config);
            let resolved =
                match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
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
    fn test_three_registry_chain_works() -> Result<(), weaver_semconv::Error> {
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
    fn test_v2_dependency_resolution() -> Result<(), weaver_semconv::Error> {
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
    fn test_v2_three_layer_dependency_resolution() -> Result<(), weaver_semconv::Error> {
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

    #[test]
    fn test_schema_url_overrides() -> Result<(), Error> {
        let schema_url = SchemaUrl::try_from("https://app.example.com/schemas/1.0.0").unwrap();
        let override_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry".to_owned(),
        };

        let mut config = WeaverResolverConfig::default();
        _ = config
            .schema_url_overrides
            .insert(schema_url.clone(), override_path);

        let mut resolver = WeaverResolver::new(config);

        // Resolving the schema URL should successfully load it from the override local folder
        // rather than trying to fetch from remote network!
        let resolved = match resolver.resolve_schema(&schema_url) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to resolve overridden schema: {e}"),
        };

        assert_eq!(
            resolved.schema_url_str(),
            "https://app.example.com/schemas/1.0.0"
        );
        Ok(())
    }

    fn resolve_at(path: &str) -> WResult<ResolvedTelemetrySchema, Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: path.to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to create registry repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) => r
                .into_v1()
                .map(WResult::Ok)
                .unwrap_or_else(WResult::FatalErr),
            WResult::OkWithNFEs(r, nfes) => match r.into_v1() {
                Ok(v1) => WResult::OkWithNFEs(v1, nfes),
                Err(e) => WResult::FatalErr(e),
            },
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    fn resolve_inline_with_parent(
        consumer_yaml: &str,
        parent_path: &str,
    ) -> WResult<ResolvedTelemetrySchema, Error> {
        let parent_vpath = VirtualDirectoryPath::LocalFolder {
            path: parent_path.to_owned(),
        };
        let parent_repo = RegistryRepo::try_new(None, &parent_vpath, &mut vec![])
            .expect("Failed to create parent registry repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let parent_loaded = match resolver.load_repository(parent_repo) {
            WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
            WResult::FatalErr(e) => panic!("Failed to load parent: {e}"),
        };

        let consumer = LoadedSemconvRegistry::create_from_string(consumer_yaml)
            .expect("Failed to load consumer yaml");
        let with_dep = match consumer {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                ..
            } => {
                let mut all_imports = imports;
                for s in &specs {
                    let v1 = s.clone().into_v1();
                    if let Some(i) = v1.spec.imports() {
                        all_imports.push(ImportsWithProvenance {
                            imports: i.clone(),
                            provenance: v1.provenance.clone(),
                        });
                    }
                }
                LoadedSemconvRegistry::Unresolved {
                    repo,
                    specs,
                    imports: all_imports,
                    dependencies: vec![parent_loaded],
                }
            }
            _ => panic!("Expected unresolved consumer registry"),
        };

        match resolver.resolve_loaded(with_dep) {
            WResult::Ok(arc) => Arc::unwrap_or_clone(arc)
                .into_v1()
                .map(WResult::Ok)
                .unwrap_or_else(WResult::FatalErr),
            WResult::OkWithNFEs(arc, nfes) => match Arc::unwrap_or_clone(arc).into_v1() {
                Ok(v1) => WResult::OkWithNFEs(v1, nfes),
                Err(e) => WResult::FatalErr(e),
            },
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    fn assert_exclusion_errors(
        result: WResult<ResolvedTelemetrySchema, Error>,
        expected: &[(&str, &str)],
    ) {
        let err = match result {
            WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {
                panic!("Expected an exclusion error, got Ok");
            }
            WResult::FatalErr(e) => e,
        };
        let mut errors = vec![];
        collect_errors(&err, &mut errors);
        for (expected_id, expected_used_in) in expected {
            let found = errors.iter().any(|e| {
                matches!(
                    e,
                    Error::ExcludedFromDependencyResolution { id, used_in, .. }
                        if id == expected_id && used_in == expected_used_in
                )
            });
            assert!(
                found,
                "expected ExcludedFromDependencyResolution(id={expected_id}, used_in={expected_used_in}); got {errors:#?}"
            );
        }
    }

    fn collect_errors<'a>(err: &'a Error, out: &mut Vec<&'a Error>) {
        match err {
            Error::CompoundError(inner) => {
                for e in inner {
                    collect_errors(e, out);
                }
            }
            other => out.push(other),
        }
    }

    const PUBLISHED_V2_PATH: &str = "data/registry-test-dep-exclusion/published_v2";

    #[test]
    fn test_dep_exclusion_v2_fails() {
        // Resolver is fail-fast across stages (extends, attr refs, imports),
        // so each leak path is exercised in its own minimal consumer spec.
        // Inline YAML keeps the test bodies adjacent to their assertions.
        let ref_yaml = r#"
file_format: definition/2
metrics:
  - name: consumer.requests
    brief: References an excluded parent attribute.
    instrument: counter
    unit: "{request}"
    stability: stable
    attributes:
      - ref: parent.excluded
        requirement_level: required
"#;
        assert_exclusion_errors(
            resolve_inline_with_parent(ref_yaml, PUBLISHED_V2_PATH),
            &[("parent.excluded", "metric.consumer.requests")],
        );

        let extends_yaml = r#"
groups:
  - id: consumer.requests
    type: metric
    metric_name: consumer.requests
    instrument: counter
    unit: "1"
    metric_requirement_level: recommended
    stability: stable
    brief: Extends an excluded parent metric.
    extends: parent.excluded.metric
"#;
        assert_exclusion_errors(
            resolve_inline_with_parent(extends_yaml, PUBLISHED_V2_PATH),
            &[("parent.excluded.metric", "consumer.requests")],
        );

        let imports_yaml = r#"
file_format: definition/2
imports:
  metrics:
    - parent.excluded.metric
"#;
        assert_exclusion_errors(
            resolve_inline_with_parent(imports_yaml, PUBLISHED_V2_PATH),
            &[("parent.excluded.metric", "imports")],
        );
    }

    #[test]
    fn test_dep_exclusion_v2_excluded_user_still_fails() {
        // Cross-registry references to an excluded item ALWAYS fail, even when
        // the consumer's own using item is marked excluded. The boundary rule
        // is absolute: excluded items in a dependency are invisible during
        // resolution. (The within-registry "both excluded → ok" relaxation is
        // covered by `test_within_registry_both_excluded`.)
        let consumer = r#"
file_format: definition/2
metrics:
  - name: consumer.also.excluded
    brief: Consumer metric that itself is excluded.
    instrument: counter
    unit: "{request}"
    stability: stable
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: parent.excluded
        requirement_level: required
"#;
        assert_exclusion_errors(
            resolve_inline_with_parent(consumer, PUBLISHED_V2_PATH),
            &[("parent.excluded", "metric.consumer.also.excluded")],
        );
    }

    fn create_registry_from_string(
        registry_spec: &str,
    ) -> WResult<weaver_resolved_schema::registry::Registry, Error> {
        let loaded = LoadedSemconvRegistry::create_from_string(registry_spec)
            .expect("Failed to load semconv spec");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        match resolver.resolve_loaded(loaded) {
            WResult::Ok(arc) => Arc::unwrap_or_clone(arc)
                .into_v1()
                .map(|s| WResult::Ok(s.registry))
                .unwrap_or_else(WResult::FatalErr),
            WResult::OkWithNFEs(arc, nfes) => match Arc::unwrap_or_clone(arc).into_v1() {
                Ok(v1) => WResult::OkWithNFEs(v1.registry, nfes),
                Err(e) => WResult::FatalErr(e),
            },
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    #[test]
    fn test_within_registry_leak_v1_ref() {
        // Same registry: an excluded attribute is defined inside an excluded
        // host group (so its definition is fine), but a non-excluded span
        // refs it. The ref is the leak path.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.public
      type: span
      span_kind: internal
      stability: stable
      brief: A public span that leaks an excluded attribute.
      attributes:
        - ref: secret.value
          requirement_level: required",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.public"
                    )),
                    "expected exclusion error on span.public, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_within_registry_leak_inline_id() {
        // A non-excluded group defines an attribute with `id:` that is marked
        // excluded. Defining it inline means the group inlines that attribute
        // into its resolved form — a leak.
        let result = create_registry_from_string(
            "
groups:
    - id: span.public
      type: span
      span_kind: internal
      stability: stable
      brief: Public span with an inlined excluded attribute.
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.public"
                    )),
                    "expected exclusion error from inline-id, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_within_registry_both_excluded() {
        // When the using group is also excluded, leak validation is skipped.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.also.excluded
      type: span
      span_kind: internal
      stability: stable
      brief: Also excluded.
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - ref: secret.value
          requirement_level: required",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => {}
            Err(e) => panic!("expected success when both are excluded; got {e:?}"),
        }
    }

    #[test]
    fn test_dep_exclusion_migration_redefine() {
        // Parent registry has an attribute, a metric, and a span — all
        // deprecated and excluded from dependency resolution. The consumer
        // depends on the parent and redefines exactly the same items.
        // Resolution must succeed: the parent items are hidden from
        // dependents, so the consumer's redefinitions take effect.
        let result = resolve_at("data/registry-test-dep-exclusion/migration_consumer");
        let resolved = match result {
            WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s,
            WResult::FatalErr(e) => panic!("expected success; got {e:?}"),
        };

        let attrs: Vec<&str> = resolved
            .catalog
            .attributes()
            .map(|a| a.name.as_str())
            .collect();
        assert!(
            attrs.contains(&"moved.attr"),
            "expected moved.attr in catalog, got {attrs:?}"
        );

        let metric = resolved
            .groups(GroupType::Metric)
            .get("metric.moved.metric")
            .cloned()
            .expect("metric.moved.metric should be present");
        assert_eq!(
            metric.deprecated, None,
            "consumer metric must not inherit parent deprecation"
        );

        let span = resolved
            .groups(GroupType::Span)
            .get("span.moved.span")
            .cloned()
            .expect("span.moved.span should be present");
        assert_eq!(
            span.deprecated, None,
            "consumer span must not inherit parent deprecation"
        );

        // Greenfield metric (no parent equivalent, no refs) resolves cleanly
        // alongside the redefined items.
        assert!(
            resolved
                .groups(GroupType::Metric)
                .contains_key("metric.greenfield.requests"),
            "greenfield metric should resolve"
        );
    }

    #[test]
    fn test_within_registry_leak_v2_refinement() {
        // V2: a public metric_refinement targets an excluded base metric in
        // the same registry. Exercises the `extends` exclusion path on V2.
        assert_exclusion_errors(
            resolve_at("data/registry-test-dep-exclusion/within_registry_v2_leak_ref"),
            &[("metric.parent.base", "child.refined")],
        );
    }

    #[test]
    fn test_within_registry_internal_group_with_inline_excluded_attr() {
        // An `attribute_group` with `visibility: internal` is dropped before
        // the resolved schema is emitted, so an excluded inline attribute on
        // it cannot leak. The same-registry leak check must therefore treat
        // an internal group like an excluded one.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.internal
      type: attribute_group
      brief: Internal-only group.
      visibility: internal
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true",
        );

        if let Err(e) = result.into_result_failing_non_fatal() {
            panic!("expected success for internal group with inline excluded attr; got {e:?}");
        }
    }

    #[test]
    fn test_within_registry_internal_group_extends_excluded_parent() {
        // Internal consumer extending an excluded parent group must also be
        // exempt — same rationale as the inline-attr case, but exercised via
        // the `extends` (excluded_parent_error) path.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: attrs.internal_consumer
      type: attribute_group
      brief: Internal group extending an excluded parent.
      visibility: internal
      extends: attrs.private",
        );

        if let Err(e) = result.into_result_failing_non_fatal() {
            panic!("expected success for internal group extending excluded parent; got {e:?}");
        }
    }

    #[test]
    fn test_within_registry_internal_group_transitive_leak_still_fails() {
        // Internal groups exempt themselves, not their consumers. A public
        // span that pulls the internal group in via `include_groups` inherits
        // the excluded attribute ref and must still trip the leak check.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.internal
      type: attribute_group
      brief: Internal-only group hosting an excluded attribute.
      visibility: internal
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.leaks
      type: span
      span_kind: internal
      stability: stable
      brief: Public span that transitively pulls in an excluded attribute.
      extends: attrs.internal",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error on span.leaks"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.leaks"
                    )),
                    "expected exclusion error on span.leaks, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }
}
