// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use weaver_semconv::group::ImportsWithProvenance;

use crate::attribute::AttributeCatalog;
use crate::dependency::ResolvedDependency;
use crate::registry::resolve_registry_with_dependencies;
use weaver_common::result::WResult;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::semconv::SemConvSpecWithProvenance;

mod attribute;
mod dependency;
mod error;
mod loader;
mod registry;

// Make helper portions of this create public APIs.
pub use crate::error::Error;
pub use crate::loader::LoadedSemconvRegistry;

/// A resolver that can be used to load and resolve telemetry schemas.
/// All references to semantic conventions will be resolved.
pub struct SchemaResolver {}

impl SchemaResolver {
    /// Resolves a loaded semantic convention registry and returns the corresponding resolved schema.
    pub fn resolve(
        loaded: LoadedSemconvRegistry,
        include_unreferenced: bool,
    ) -> WResult<ResolvedTelemetrySchema, Error> {
        // TODO - can we deprecate include_unreferenced?
        match loaded {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                dependencies,
            } => Self::resolve_registry(repo, specs, imports, dependencies, include_unreferenced),
            LoadedSemconvRegistry::Resolved(resolved_telemetry_schema) => {
                WResult::Ok(resolved_telemetry_schema)
            }
            LoadedSemconvRegistry::ResolvedV2(_) => {
                todo!("Converting V2 schema back into V1 is unsupported")
            }
        }
    }

    // Actually resolves a defiinition registry.
    fn resolve_registry(
        repo: RegistryRepo,
        specs: Vec<SemConvSpecWithProvenance>,
        imports: Vec<ImportsWithProvenance>,
        dependencies: Vec<LoadedSemconvRegistry>,
        include_unreferenced: bool,
    ) -> WResult<ResolvedTelemetrySchema, Error> {
        // First, let's make sure all dependencies are resolved.
        let mut opt_resolved_dependencies: Vec<WResult<ResolvedDependency, Error>> = vec![];
        // TODO - do this in multiple threads w/ `.par_bridge()` and `+ Send`.
        for d in dependencies {
            match d {
                LoadedSemconvRegistry::Unresolved { .. } => {
                    opt_resolved_dependencies
                        .push(Self::resolve(d, include_unreferenced).map(|s| s.into()));
                }
                LoadedSemconvRegistry::Resolved(schema) => {
                    opt_resolved_dependencies.push(WResult::Ok(schema.into()));
                }
                LoadedSemconvRegistry::ResolvedV2(schema) => {
                    opt_resolved_dependencies.push(WResult::Ok(schema.into()));
                }
            }
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
        let registry_id: String = repo.id().to_string();
        let manifest = repo.manifest().cloned();
        let mut attr_catalog = AttributeCatalog::default();
        // TODO - Do something with non_fatal_errors if we need to.
        resolve_registry_with_dependencies(
            &mut attr_catalog,
            repo,
            specs,
            imports,
            resolved_dependencies,
            include_unreferenced,
        )
        .map(move |resolved_registry| {
            let catalog = Catalog::from_attributes(attr_catalog.drain_attributes());

            ResolvedTelemetrySchema {
                file_format: "1.0.0".to_owned(),
                schema_url: "".to_owned(),
                registry_id,
                registry: resolved_registry,
                catalog,
                resource: None,
                instrumentation_library: None,
                dependencies: vec![],
                versions: None, // ToDo LQ: Implement this!
                registry_manifest: manifest,
            }
        })
    }

    /// Loads a semantic convention repository.
    ///
    /// Note: This may load in a definition (raw) repository *or* an already resolved repository.
    ///       When loading a raw repository, dependencies will also be loaded.
    pub fn load_semconv_repository(
        registry_repo: RegistryRepo,
        follow_symlinks: bool,
    ) -> WResult<LoadedSemconvRegistry, weaver_semconv::Error> {
        loader::load_semconv_repository(registry_repo, follow_symlinks)
    }
}

#[cfg(test)]
mod tests {
    use crate::SchemaResolver;
    use std::collections::HashSet;
    use weaver_common::diagnostic::DiagnosticMessages;
    use weaver_common::result::WResult;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::group::GroupType;
    use weaver_semconv::registry_repo::RegistryRepo;

    #[test]
    fn test_multi_registry() -> Result<(), weaver_semconv::Error> {
        /// Helper to load a specific repository and reoslve with the given include flag.
        fn check_semconv_load_and_resolve(registry_repo: RegistryRepo, include_unreferenced: bool) {
            let mut diag_msgs = DiagnosticMessages::empty();
            let loaded = SchemaResolver::load_semconv_repository(registry_repo, false)
                .capture_non_fatal_errors(&mut diag_msgs)
                .expect("Failed to load the registry");
            // println!("Loaded registry: {loaded}");
            let resolved = SchemaResolver::resolve(loaded, include_unreferenced);
            match resolved {
                WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                    // TODO - handle includes *and* include unreferenced.
                    if include_unreferenced {
                        // The group `otel.unused` shouldn't be garbage collected
                        let group = resolved_registry.group("otel.unused");
                        assert!(group.is_some());

                        // These groups are referenced in the `imports` and should not be garbage
                        // collected
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
                    } else {
                        // These groups should be garbage collected because they are not referenced
                        // anywhere (in ref or imports)
                        let group = resolved_registry.group("otel.unused");
                        assert!(group.is_none());
                        let group = resolved_registry.group("event.session.end");
                        assert!(group.is_none());

                        // These groups are referenced in the `imports` and should not be garbage
                        // collected
                        let group = resolved_registry.group("metric.example.counter");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.application");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.service");
                        assert!(group.is_some());
                        let group = resolved_registry.group("event.session.start");
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
                        let attr = resolved_registry
                            .catalog
                            .attribute(attr_ref)
                            .expect("Failed to resolve attribute");
                        _ = attr_names.insert(attr.name.clone());
                        match attr.name.as_str() {
                            "auction.name" => {}
                            "auction.id" => {}
                            "error.type" => {
                                // Check requirement level is properly overridden.
                                // Initially, it was set to `recommended` in the otel registry.
                                // It should be overridden to `required` in the custom registry.
                                assert_eq!(
                                    attr.requirement_level,
                                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                                );
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
                WResult::FatalErr(fatal) => {
                    panic!("Fatal error: {fatal}");
                }
            }
        }

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("main", &registry_path)?;
        // test with the `include_unreferenced` flag set to false
        check_semconv_load_and_resolve(registry_repo.clone(), false);
        // test with the `include_unreferenced` flag set to true
        check_semconv_load_and_resolve(registry_repo.clone(), true);
        Ok(())
    }

    #[test]
    fn test_three_registry_chain_works() -> Result<(), weaver_semconv::Error> {
        // Test the three-registry chain: app -> acme -> otel
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("app", &registry_path)?;
        let result = SchemaResolver::load_semconv_repository(registry_repo, true);

        match result {
            WResult::Ok(loaded) | WResult::OkWithNFEs(loaded, _) => {
                // Should successfully load specs from all three registries
                assert_eq!(
                    loaded.dependency_depth(),
                    3,
                    "Expected specs from at least 3 registries, got {}",
                    loaded
                );

                // Verify we have specs from all three registries
                let registry_ids = loaded.registry_ids();

                assert!(
                    registry_ids.contains(&"app".to_owned()),
                    "Missing app registry specs"
                );
                assert!(
                    registry_ids.contains(&"acme".to_owned()),
                    "Missing acme registry specs"
                );
                assert!(
                    registry_ids.contains(&"otel".to_owned()),
                    "Missing otel registry specs"
                );

                // Now test the resolved registry content
                let resolved_result = SchemaResolver::resolve(loaded, false);

                match resolved_result {
                    WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                        // Check that ONLY the app.example group exists (no imported groups should be in the resolved registry)
                        use weaver_semconv::group::GroupType;
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

                        // Should have the app.example group and the imported example.counter metric
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

                        // Check that app.example group exists and has exactly the expected attributes
                        let app_group = resolved_registry
                            .group("app.example")
                            .expect("app.example group should exist");

                        // Collect attribute names for verification
                        let mut attr_names = HashSet::new();
                        for attr_ref in &app_group.attributes {
                            let attr = resolved_registry
                                .catalog
                                .attribute(attr_ref)
                                .expect("Failed to resolve attribute");
                            let _ = attr_names.insert(attr.name.clone());
                        }

                        // Verify we have exactly the expected attributes
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
                    }
                    WResult::FatalErr(fatal) => {
                        panic!("Failed to resolve registry: {fatal}");
                    }
                }
            }
            WResult::FatalErr(fatal) => {
                panic!("Unexpected fatal error in three-registry chain: {fatal}");
            }
        }

        Ok(())
    }
}
