// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use weaver_semconv::group::ImportsWithProvenance;
use weaver_semconv::schema_url::SchemaUrl;

use crate::attribute::AttributeCatalog;
use crate::dependency::ResolvedDependency;
use crate::registry::resolve_registry_with_dependencies;
use weaver_common::result::WResult;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::semconv::SemConvSpecWithProvenance;

mod attribute;
mod dependency;
mod dependency_resolution;
mod error;
mod loader;
pub(crate) mod merge;
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

    // Actually resolves a definition registry.
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
                LoadedSemconvRegistry::Unresolved {
                    repo,
                    specs,
                    imports,
                    dependencies,
                } => {
                    opt_resolved_dependencies.push(
                        Self::resolve_registry(
                            repo,
                            specs,
                            imports,
                            dependencies,
                            include_unreferenced,
                        )
                        .map(|s| s.into()),
                    );
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
            ResolvedTelemetrySchema {
                file_format: "1.0.0".to_owned(),
                schema_url: schema_url.as_str().to_owned(),
                registry_id: schema_url.name().to_owned(),
                registry: resolved_registry,
                catalog: attr_catalog.into(),
                resource: None,
                instrumentation_library: None,
                dependencies,
                versions: None, // ToDo LQ: Implement this!
                registry_manifest: manifest,
            }
        })
    }

    /// Load a semantic convention repository with no HTTP credentials
    /// configured. May load a raw (definition) repo with its dependencies, or
    /// a pre-resolved one.
    pub fn load_semconv_repository(
        registry_repo: RegistryRepo,
        follow_symlinks: bool,
    ) -> WResult<LoadedSemconvRegistry, Error> {
        loader::load_semconv_repository(
            registry_repo,
            follow_symlinks,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        )
    }

    /// Load a semantic convention repository, resolving credentials via `auth`
    /// for dependencies and resolved-schema fetches.
    pub fn load_semconv_repository_with_auth(
        registry_repo: RegistryRepo,
        follow_symlinks: bool,
        auth: &weaver_common::http_auth::HttpAuthResolver,
    ) -> WResult<LoadedSemconvRegistry, Error> {
        loader::load_semconv_repository(registry_repo, follow_symlinks, auth)
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
    use weaver_semconv::group::{GroupType, ImportsWithProvenance};
    use weaver_semconv::registry_repo::RegistryRepo;

    #[test]
    fn test_multi_registry() -> Result<(), weaver_semconv::Error> {
        /// Helper to load a specific repository and resolve with the given include flag.
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
                        let group = resolved_registry.group("custom.group");
                        assert!(group.is_some());
                        let group = resolved_registry.group("custom.span");
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
                                // The brief should come from the original definition in otel.registry,
                                // NOT from db.client.metrics which refines it with a different brief.
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
                WResult::FatalErr(fatal) => {
                    panic!("Fatal error: {fatal}");
                }
            }
        }

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
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
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
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
                let registry_names = loaded.registry_names();

                assert!(
                    registry_names.contains(&"app.com/schemas".to_owned()),
                    "Missing app registry specs, available registries: {:?}",
                    registry_names
                );
                assert!(
                    registry_names.contains(&"acme.com/schemas".to_owned()),
                    "Missing acme registry specs, available registries: {:?}",
                    registry_names
                );
                assert!(
                    registry_names.contains(&"opentelemetry.io/schemas".to_owned()),
                    "Missing otel registry specs, available registries: {:?}",
                    registry_names
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

    #[test]
    fn test_v2_dependency_resolution() -> Result<(), weaver_semconv::Error> {
        // Test that a consumer registry can resolve attribute refs from a pre-resolved V2 dependency.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/consumer_registry".to_owned(),
        };

        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut diag_msgs = DiagnosticMessages::empty();
        let loaded = SchemaResolver::load_semconv_repository(registry_repo, false)
            .capture_non_fatal_errors(&mut diag_msgs)
            .expect("Failed to load consumer registry");

        let resolved = SchemaResolver::resolve(loaded, false);
        match resolved {
            WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                let metrics = resolved_registry.groups(GroupType::Metric);
                let metric = metrics
                    .get("metric.consumer.request.count")
                    .expect("metric.consumer.request.count not found");

                assert_eq!(metric.attributes.len(), 2);

                let mut attr_names = HashSet::new();
                for attr_ref in &metric.attributes {
                    let attr = resolved_registry
                        .catalog
                        .attribute(attr_ref)
                        .expect("Failed to resolve attribute ref");
                    _ = attr_names.insert(attr.name.clone());
                    match attr.name.as_str() {
                        "server.address" => {
                            // requirement_level overridden to required in consumer
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
                            // brief overridden locally in consumer
                            assert_eq!(attr.brief, "The server port used by the consumer.");
                            // type still comes from the V2 dependency
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
            }
            WResult::FatalErr(fatal) => {
                panic!("Failed to resolve consumer registry: {fatal}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_v2_three_layer_dependency_resolution() -> Result<(), weaver_semconv::Error> {
        // TODO: this only works with definition registry, but not with
        // resolved one, because resolved does not know how to get
        // attributes from transitive dependencies.
        // Test that briefs are correctly inherited through two levels of V2 dependencies:
        // app_registry -> consumer_registry -> published (server definitions)
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut diag_msgs = DiagnosticMessages::empty();
        let loaded = SchemaResolver::load_semconv_repository(registry_repo, false)
            .capture_non_fatal_errors(&mut diag_msgs)
            .expect("Failed to load app registry");

        let resolved = SchemaResolver::resolve(loaded, false);
        match resolved {
            WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                let metrics = resolved_registry.groups(GroupType::Metric);
                let metric = metrics
                    .get("metric.app.request.count")
                    .expect("metric.app.request.count not found");
                assert_eq!(metric.attributes.len(), 2);
                for attr_ref in &metric.attributes {
                    let attr = resolved_registry
                        .catalog
                        .attribute(attr_ref)
                        .expect("Failed to resolve attribute ref");
                    match attr.name.as_str() {
                        // Briefs must come from the original definitions in published/,
                        // two V2 dependency hops away.
                        "server.address" => assert_eq!(attr.brief, "Server address."),
                        "server.port" => assert_eq!(attr.brief, "Server port."),
                        _ => panic!("Unexpected attribute: {}", attr.name),
                    }
                }
            }
            WResult::FatalErr(fatal) => {
                panic!("Failed to resolve app registry: {fatal}");
            }
        }

        Ok(())
    }

    fn resolve_at(
        path: &str,
    ) -> WResult<weaver_resolved_schema::ResolvedTelemetrySchema, crate::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: path.to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to create registry repo");
        let mut diag_msgs = DiagnosticMessages::empty();
        let loaded = SchemaResolver::load_semconv_repository(registry_repo, false)
            .capture_non_fatal_errors(&mut diag_msgs)
            .expect("Failed to load registry");
        SchemaResolver::resolve(loaded, false)
    }

    fn resolve_inline_with_parent(
        consumer_yaml: &str,
        parent_path: &str,
    ) -> WResult<weaver_resolved_schema::ResolvedTelemetrySchema, crate::Error> {
        let parent_vpath = VirtualDirectoryPath::LocalFolder {
            path: parent_path.to_owned(),
        };
        let parent_repo = RegistryRepo::try_new(None, &parent_vpath, &mut vec![])
            .expect("Failed to create parent registry repo");
        let mut diag_msgs = DiagnosticMessages::empty();
        let parent_loaded = SchemaResolver::load_semconv_repository(parent_repo, false)
            .capture_non_fatal_errors(&mut diag_msgs)
            .expect("Failed to load parent registry");

        let consumer = crate::LoadedSemconvRegistry::create_from_string(consumer_yaml)
            .expect("Failed to load consumer yaml");
        let with_dep = match consumer {
            crate::LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                ..
            } => {
                // create_from_string does not extract `imports:` from the spec;
                // do it here so this helper handles imports-driven scenarios.
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
                crate::LoadedSemconvRegistry::Unresolved {
                    repo,
                    specs,
                    imports: all_imports,
                    dependencies: vec![parent_loaded],
                }
            }
            _ => panic!("Expected unresolved consumer registry"),
        };

        SchemaResolver::resolve(with_dep, false)
    }

    fn assert_exclusion_errors(
        result: WResult<weaver_resolved_schema::ResolvedTelemetrySchema, crate::Error>,
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
                    crate::Error::ExcludedFromDependencyResolution { id, used_in, .. }
                        if id == expected_id && used_in == expected_used_in
                )
            });
            assert!(
                found,
                "expected ExcludedFromDependencyResolution(id={expected_id}, used_in={expected_used_in}); got {errors:#?}"
            );
        }
    }

    fn collect_errors<'a>(err: &'a crate::Error, out: &mut Vec<&'a crate::Error>) {
        match err {
            crate::Error::CompoundError(inner) => {
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
    requirement_level: recommended
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
    requirement_level: recommended
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
    ) -> WResult<weaver_resolved_schema::registry::Registry, crate::Error> {
        let loaded = crate::LoadedSemconvRegistry::create_from_string(registry_spec)
            .expect("Failed to load semconv spec");
        SchemaResolver::resolve(loaded, false).map(|schema| schema.registry)
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
            Err(crate::Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        crate::Error::ExcludedFromDependencyResolution { id, used_in, .. }
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
            Err(crate::Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        crate::Error::ExcludedFromDependencyResolution { id, used_in, .. }
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
            Err(crate::Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        crate::Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.leaks"
                    )),
                    "expected exclusion error on span.leaks, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }
}
