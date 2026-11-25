// SPDX-License-Identifier: Apache-2.0

//! Parity tests comparing `ResolutionTarget::V2` against `ResolutionTarget::V1` (converted to V2) on V2 registries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolver::{
    DefaultSchemaVisitor, Error, ResolutionTarget, WeaverResolvedSchema, WeaverResolver,
    WeaverResolverConfig,
};
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
struct TestSettings {
    schema_url: Option<String>,
    #[serde(default)]
    schema_url_overrides: HashMap<String, String>,
}

fn load_settings(test_dir: &str) -> TestSettings {
    let direct = Path::new(test_dir).join("settings.yaml");
    let path = if direct.exists() {
        Some(direct)
    } else if let Some(parent) = Path::new(test_dir).parent() {
        let parent_settings = parent.join("settings.yaml");
        if parent_settings.exists() {
            Some(parent_settings)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(path) = path {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
        serde_saphyr::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()))
    } else {
        TestSettings::default()
    }
}

fn resolve_fixture_raw(
    test_dir: &str,
    target: ResolutionTarget,
) -> WResult<WeaverResolvedSchema, Error> {
    let settings = load_settings(test_dir);
    let registry_subfolder = Path::new(test_dir).join("registry");
    let target_path = if registry_subfolder.is_dir() {
        registry_subfolder.display().to_string().replace('\\', "/")
    } else {
        test_dir.replace('\\', "/")
    };

    let schema_url: Option<SchemaUrl> = settings
        .schema_url
        .as_deref()
        .map(|s| s.try_into().expect("valid schema_url in settings"))
        .or_else(|| {
            Some(
                "https://default/0.1.0"
                    .try_into()
                    .expect("valid default schema url"),
            )
        });

    let location = VirtualDirectoryPath::LocalFolder { path: target_path };
    let mut repo_nfes = Vec::new();
    let repo = match RegistryRepo::try_new(schema_url, &location, &mut repo_nfes) {
        Ok(repo) => repo,
        Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
    };

    let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
    for (url_str, local_path) in &settings.schema_url_overrides {
        let url: SchemaUrl = url_str
            .as_str()
            .try_into()
            .expect("valid override schema_url");
        resolver.add_schema_url_override(
            url,
            VirtualDirectoryPath::LocalFolder {
                path: local_path.clone(),
            },
        );
    }

    resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor)
}

fn resolve_fixture(test_dir: &str, target: ResolutionTarget) -> Result<V2Schema, Error> {
    let resolved = match resolve_fixture_raw(test_dir, target) {
        WResult::Ok(r) => Ok(r),
        WResult::OkWithNFEs(r, nfes) => {
            for nfe in nfes {
                let allowed = matches!(
                    nfe,
                    Error::MixedRegistryUsesV1Path { .. }
                        | Error::V1DependencyUsesV1Path { .. }
                        | Error::DeprecatedIncludeUnreferencedWarning { .. }
                        | Error::DuplicateMetricName { .. }
                        | Error::DuplicateGroupName { .. }
                        | Error::DuplicateGroupId { .. }
                        | Error::UnmatchedImport { .. }
                        | Error::FailToResolveDefinition(
                            weaver_semconv::Error::UnstableFileFormat { .. }
                                | weaver_semconv::Error::LegacyRegistryManifest { .. }
                                | weaver_semconv::Error::DeprecatedVersionField { .. }
                                | weaver_semconv::Error::MissingManifestFileFormat { .. }
                                | weaver_semconv::Error::MissingRequirementLevelWarning { .. }
                                | weaver_semconv::Error::InvalidGroupStability { .. }
                        )
                );
                if !allowed {
                    return Err(nfe);
                }
            }
            Ok(r)
        }
        WResult::FatalErr(e) => Err(e),
    }?;

    match resolved {
        WeaverResolvedSchema::V2(v2) => Ok(v2),
        WeaverResolvedSchema::V1(v1) => match V2Schema::try_from(v1) {
            Ok(v2) => Ok(v2),
            Err(e) => panic!("V1 to V2 schema conversion failed for `{test_dir}`: {e}"),
        },
    }
}

#[test]
fn test_parity_registry_test_v2_1_everything() {
    let legacy = resolve_fixture("data/registry-test-v2-1-everything", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/registry-test-v2-1-everything", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_eq!(
        native.attribute_catalog, legacy.attribute_catalog,
        "attribute_catalog mismatch"
    );
    assert_eq!(
        native.registry.attributes, legacy.registry.attributes,
        "registry.attributes mismatch"
    );
    assert_eq!(
        native.registry.metrics, legacy.registry.metrics,
        "registry.metrics mismatch"
    );
    assert_eq!(
        native.refinements.metrics, legacy.refinements.metrics,
        "refinements.metrics mismatch"
    );
    assert_eq!(
        native.dependencies, legacy.dependencies,
        "dependencies mismatch"
    );
}

#[test]
fn test_parity_registry_test_v2_2_multifile() {
    let legacy = resolve_fixture("data/registry-test-v2-2-multifile", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/registry-test-v2-2-multifile", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_eq!(
        native.attribute_catalog, legacy.attribute_catalog,
        "attribute_catalog mismatch"
    );
    assert_eq!(
        native.registry.attributes, legacy.registry.attributes,
        "registry.attributes mismatch"
    );
    assert_eq!(
        native.registry.metrics, legacy.registry.metrics,
        "registry.metrics mismatch"
    );
    assert_eq!(
        native.refinements.metrics, legacy.refinements.metrics,
        "refinements.metrics mismatch"
    );
}

#[test]
fn test_parity_signal_name_preservation() {
    let legacy = resolve_fixture("data/signal-name-preservation/v2", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/signal-name-preservation/v2", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_eq!(
        native.attribute_catalog, legacy.attribute_catalog,
        "attribute_catalog mismatch"
    );
    assert_eq!(
        native.registry.attributes, legacy.registry.attributes,
        "registry.attributes mismatch"
    );
    assert_eq!(
        native.registry.entities, legacy.registry.entities,
        "registry.entities mismatch"
    );
    assert_eq!(
        native.registry.spans, legacy.registry.spans,
        "registry.spans mismatch"
    );
    assert_eq!(
        native.registry.events, legacy.registry.events,
        "registry.events mismatch"
    );
    assert_eq!(
        native.registry.metrics, legacy.registry.metrics,
        "registry.metrics mismatch"
    );
}

#[test]
fn test_span_name_attribute_not_on_span() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");
    std::fs::write(
        reg_dir.join("span.yaml"),
        r#"file_format: definition/2
attributes:
  - key: span.attr
    type: string
    stability: stable
    brief: span attr
spans:
  - type: my.span
    brief: my span
    stability: stable
    kind: client
    name:
      templates:
        - "{missing.attr}"
    attributes:
      - ref: span.attr
"#,
    )
    .expect("write");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let res = resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor);
    match res {
        WResult::FatalErr(Error::SpanNameAttributeNotOnSpan {
            span_id,
            attribute_key,
            ..
        }) => {
            assert_eq!(span_id, "my.span");
            assert_eq!(attribute_key, "missing.attr");
        }
        WResult::FatalErr(other) => panic!("Expected SpanNameAttributeNotOnSpan, got: {other}"),
        WResult::Ok(_) | WResult::OkWithNFEs(..) => panic!("Expected FatalErr, got Ok"),
    }
}

#[test]
fn test_entity_missing_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");
    std::fs::write(
        reg_dir.join("entity.yaml"),
        r#"file_format: definition/2
attributes:
  - key: desc.attr
    type: string
    stability: stable
    brief: desc attr
entities:
  - type: my.entity
    brief: my entity
    stability: stable
    identity: []
    description:
      - ref: desc.attr
"#,
    )
    .expect("write");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let res = resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor);
    match res {
        WResult::FatalErr(Error::EntityMissingIdentity { entity_type, .. }) => {
            assert_eq!(entity_type, "my.entity");
        }
        WResult::FatalErr(Error::FailToResolveDefinition(semconv_err)) => {
            let msg = semconv_err.to_string();
            assert!(
                msg.contains("my.entity") && msg.contains("identity"),
                "Expected entity identity error, got: {msg}"
            );
        }
        WResult::FatalErr(other) => panic!("Expected EntityMissingIdentity, got: {other}"),
        WResult::Ok(_) | WResult::OkWithNFEs(..) => panic!("Expected FatalErr, got Ok"),
    }
}

#[test]
fn test_public_vs_internal_groups_and_precedence() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");
    std::fs::write(
        reg_dir.join("groups.yaml"),
        r#"file_format: definition/2
attributes:
  - key: a.key
    type: string
    stability: stable
    brief: original A
  - key: b.key
    type: string
    stability: stable
    brief: original B
attribute_groups:
  - id: internal.group
    visibility: internal
    attributes:
      - ref: a.key
        brief: internal override A
      - ref: b.key
  - id: public.group
    visibility: public
    brief: public group
    stability: stable
    attributes:
      - ref_group: internal.group
      - ref: a.key
        brief: inline override on public group
"#,
    )
    .expect("write");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let res = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
        WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
        WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
    };

    let v2 = res.as_v2().expect("V2 schema");
    assert_eq!(v2.registry.attribute_groups.len(), 1);
    let pub_g = &v2.registry.attribute_groups[0];
    assert_eq!(&*pub_g.id, "public.group");
    assert_eq!(pub_g.attributes.len(), 2);
    let a_attr = &v2.attribute_catalog[pub_g.attributes[0].base.0 as usize];
    assert_eq!(a_attr.key, "a.key");
    assert_eq!(
        a_attr.common.brief.as_str(),
        "inline override on public group"
    );
}

fn resolve_from_yaml(yaml: &str, target: ResolutionTarget) -> WResult<WeaverResolvedSchema, Error> {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write");
    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
    resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor)
}

fn into_v2(schema: &WeaverResolvedSchema) -> V2Schema {
    match schema {
        WeaverResolvedSchema::V2(v2) => v2.clone(),
        WeaverResolvedSchema::V1(v1) => V2Schema::try_from(v1.clone()).expect("convert v1 to v2"),
    }
}

#[test]
fn test_parity_field_level_merging_inline_ref_overrides_ref_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: server.address
    type: string
    brief: Server address
    stability: stable
attribute_groups:
  - id: attributes.http.client.authority
    visibility: internal
    attributes:
      - ref: server.address
        requirement_level: required
        note: "HTTP/1.1 note..."
spans:
  - type: http.client
    kind: internal
    brief: Http client span
    stability: stable
    name:
      note: test
    attributes:
      - ref_group: attributes.http.client.authority
      - ref: server.address
        sampling_relevant: true
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let run_resolver = |target: ResolutionTarget| {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => into_v2(&r),
            WResult::FatalErr(e) => panic!("Target {target:?} failed: {e}"),
        }
    };

    let legacy_v2 = run_resolver(ResolutionTarget::V1);
    let native_v2 = run_resolver(ResolutionTarget::V2);

    assert_eq!(legacy_v2.registry.spans.len(), 1);
    assert_eq!(native_v2.registry.spans.len(), 1);

    let leg_span = &legacy_v2.registry.spans[0];
    let nat_span = &native_v2.registry.spans[0];

    assert_eq!(leg_span.attributes.len(), 1);
    assert_eq!(nat_span.attributes.len(), 1);

    assert_eq!(leg_span.attributes[0].sampling_relevant, Some(true));
    assert_eq!(nat_span.attributes[0].sampling_relevant, Some(true));

    assert_eq!(
        leg_span.attributes[0].requirement_level,
        weaver_semconv::v2::attribute::RequirementLevel::Basic(
            weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required
        )
    );
    assert_eq!(
        nat_span.attributes[0].requirement_level,
        weaver_semconv::v2::attribute::RequirementLevel::Basic(
            weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required
        )
    );

    let leg_attr = &legacy_v2.attribute_catalog[leg_span.attributes[0].base.0 as usize];
    let nat_attr = &native_v2.attribute_catalog[nat_span.attributes[0].base.0 as usize];

    assert_eq!(leg_attr.common.note.as_deref(), Some("HTTP/1.1 note..."));
    assert_eq!(nat_attr.common.note.as_deref(), Some("HTTP/1.1 note..."));

    assert_eq!(legacy_v2.attribute_catalog, native_v2.attribute_catalog);
    assert_eq!(legacy_v2.registry.spans, native_v2.registry.spans);
}

#[test]
fn test_parity_excluded_public_attribute_group() {
    let yaml = r#"file_format: definition/2
attributes:
  - key: test.attr
    type: string
    brief: attr
    stability: stable
attribute_groups:
  - id: attrs.excluded
    visibility: public
    brief: Excluded group.
    stability: stable
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: test.attr
spans:
  - type: leaky.span
    kind: internal
    name:
      note: leaky
    brief: Public span that pulls in an excluded group.
    stability: stable
    attributes:
      - ref_group: attrs.excluded
"#;

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(yaml, target);
        match res {
            WResult::FatalErr(Error::CompoundError(errs)) => {
                assert!(
                    errs.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "attrs.excluded" && used_in == "span.leaky.span"
                    )),
                    "Target {target:?} expected ExcludedFromDependencyResolution for attrs.excluded in span.leaky.span, got: {errs:?}"
                );
            }
            WResult::FatalErr(Error::ExcludedFromDependencyResolution { id, used_in, .. }) => {
                assert_eq!(id, "attrs.excluded");
                assert_eq!(used_in, "span.leaky.span");
            }
            WResult::Ok(_) | WResult::OkWithNFEs(..) => {
                panic!("Target {target:?} expected FatalErr, got Ok")
            }
            WResult::FatalErr(other) => panic!("Target {target:?} unexpected FatalErr: {other}"),
        }
    }
}

#[test]
fn test_parity_internal_group_with_inline_excluded_attr() {
    let safe_yaml = r#"file_format: definition/2
attributes:
  - key: secret.value
    type: string
    stability: stable
    brief: Hidden detail.
    annotations:
      dependency_resolution:
        exclude: true
attribute_groups:
  - id: attrs.internal
    visibility: internal
    attributes:
      - ref: secret.value
"#;
    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(safe_yaml, target);
        assert!(
            matches!(res, WResult::Ok(_) | WResult::OkWithNFEs(..)),
            "Target {target:?} expected success for internal group with excluded attr"
        );
    }

    let leak_yaml = r#"file_format: definition/2
attributes:
  - key: secret.value
    type: string
    stability: stable
    brief: Hidden detail.
    annotations:
      dependency_resolution:
        exclude: true
attribute_groups:
  - id: attrs.internal
    visibility: internal
    attributes:
      - ref: secret.value
spans:
  - type: leaks
    kind: internal
    name:
      note: leak
    stability: stable
    brief: Public span that transitively pulls in an excluded attribute.
    attributes:
      - ref_group: attrs.internal
"#;
    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(leak_yaml, target);
        match res {
            WResult::FatalErr(Error::CompoundError(errs)) => {
                assert!(
                    errs.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.leaks"
                    )),
                    "Target {target:?} expected ExcludedFromDependencyResolution for secret.value in span.leaks, got: {errs:?}"
                );
            }
            WResult::FatalErr(Error::ExcludedFromDependencyResolution { id, used_in, .. }) => {
                assert_eq!(id, "secret.value");
                assert_eq!(used_in, "span.leaks");
            }
            WResult::Ok(_) | WResult::OkWithNFEs(..) => {
                panic!("Target {target:?} expected FatalErr, got Ok")
            }
            WResult::FatalErr(other) => panic!("Target {target:?} unexpected FatalErr: {other}"),
        }
    }
}

#[test]
fn test_parity_v2_dependency_attribute_ref() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep_dir = dir.path().join("dep/registry");
    let root_dir = dir.path().join("root/registry");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::create_dir_all(&root_dir).expect("create root dir");

    std::fs::write(
        dep_dir.join("manifest.yaml"),
        r#"file_format: definition_manifest/2.0
schema_url: https://dep.example.com/schemas/1.0.0
description: V2 Dependency
stability: stable
"#,
    )
    .expect("write dep manifest");

    std::fs::write(
        dep_dir.join("attrs.yaml"),
        r#"file_format: definition/2
attributes:
  - key: dep.attr
    type: string
    brief: Attribute defined in V2 dependency
    stability: stable
"#,
    )
    .expect("write dep attrs");

    let dep_rel_path = dep_dir.display().to_string();
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            r#"file_format: definition_manifest/2.0
schema_url: https://root.example.com/schemas/1.0.0
description: Root registry
dependencies:
  - schema_url: https://dep.example.com/schemas/1.0.0
    registry_path: {dep_rel_path}
stability: stable
"#
        ),
    )
    .expect("write root manifest");

    std::fs::write(
        root_dir.join("span.yaml"),
        r#"file_format: definition/2
spans:
  - type: consumer.span
    kind: internal
    brief: Consumer span referencing dep attr
    stability: stable
    name:
      note: test
    attributes:
      - ref: dep.attr
        brief: Overridden brief on consumer span
"#,
    )
    .expect("write root span");

    let location = VirtualDirectoryPath::LocalFolder {
        path: root_dir.display().to_string(),
    };

    let run_resolver = |target: ResolutionTarget| {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => into_v2(&r),
            WResult::FatalErr(e) => panic!("Target {target:?} failed: {e}"),
        }
    };

    let legacy_v2 = run_resolver(ResolutionTarget::V1);
    let native_v2 = run_resolver(ResolutionTarget::V2);

    assert_eq!(legacy_v2.dependencies, native_v2.dependencies);
    assert_eq!(legacy_v2.attribute_catalog, native_v2.attribute_catalog);
    assert_eq!(legacy_v2.registry.spans, native_v2.registry.spans);
}

#[test]
fn test_parity_v1_dependency_attribute_ref() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep_dir = dir.path().join("v1dep/registry");
    let root_dir = dir.path().join("root/registry");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::create_dir_all(&root_dir).expect("create root dir");

    std::fs::write(
        dep_dir.join("registry_manifest.yaml"),
        r#"name: v1dep
schema_url: https://v1dep.example.com/schemas/1.0.0
description: V1 Dependency
"#,
    )
    .expect("write v1 dep manifest");

    std::fs::write(
        dep_dir.join("v1_group.yaml"),
        r#"groups:
  - id: v1.group
    type: attribute_group
    brief: V1 attribute group
    attributes:
      - id: dep.v1.attr
        type: string
        brief: Attribute defined in V1 dependency
        stability: stable
"#,
    )
    .expect("write v1 dep group");

    let dep_rel_path = dep_dir.display().to_string();
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            r#"file_format: definition_manifest/2.0
schema_url: https://root.example.com/schemas/1.0.0
description: Root registry
dependencies:
  - schema_url: https://v1dep.example.com/schemas/1.0.0
    registry_path: {dep_rel_path}
stability: stable
"#
        ),
    )
    .expect("write root manifest");

    std::fs::write(
        root_dir.join("span.yaml"),
        r#"file_format: definition/2
spans:
  - type: consumer.span
    kind: internal
    brief: Consumer span referencing v1 dep attr
    stability: stable
    name:
      note: test
    attributes:
      - ref: dep.v1.attr
        brief: Overridden brief on consumer span
"#,
    )
    .expect("write root span");

    let location = VirtualDirectoryPath::LocalFolder {
        path: root_dir.display().to_string(),
    };

    let run_resolver = |target: ResolutionTarget| {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => into_v2(&r),
            WResult::FatalErr(e) => panic!("Target {target:?} failed: {e}"),
        }
    };

    let legacy_v2 = run_resolver(ResolutionTarget::V1);
    let native_v2 = run_resolver(ResolutionTarget::V2);

    assert_eq!(legacy_v2.dependencies, native_v2.dependencies);
    assert_eq!(legacy_v2.attribute_catalog, native_v2.attribute_catalog);
    assert_eq!(legacy_v2.registry.spans, native_v2.registry.spans);
}

fn assert_v2_schema_eq(native: &V2Schema, legacy: &V2Schema) {
    assert_eq!(
        native.file_format, legacy.file_format,
        "file_format mismatch"
    );
    assert_eq!(native.schema_url, legacy.schema_url, "schema_url mismatch");
    assert_eq!(
        native.dependencies, legacy.dependencies,
        "dependencies mismatch"
    );
    assert_eq!(
        native.attribute_catalog, legacy.attribute_catalog,
        "attribute_catalog mismatch"
    );
    assert_eq!(native.registry, legacy.registry, "registry mismatch");
    assert_eq!(
        native.refinements, legacy.refinements,
        "refinements mismatch"
    );
}

#[test]
fn test_parity_registry_test_v2_4_refinements() {
    let legacy = resolve_fixture("data/registry-test-v2-4-refinements", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/registry-test-v2-4-refinements", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_registry_test_v2_dep_metric_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/metric_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/metric_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_registry_test_v2_dep_span_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/span_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/span_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_registry_test_v2_dep_event_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/event_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/event_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_registry_test_v2_dep_entity_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/entity_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/entity_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_registry_test_v2_dep_deep_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/deep_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/deep_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_entity_identity_inheritance() {
    let legacy = resolve_fixture("data/entity-identity-inheritance", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/entity-identity-inheritance", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_entity_identity_dependency_role_consumer() {
    let legacy = resolve_fixture(
        "data/entity-identity-dependency-role/consumer",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/entity-identity-dependency-role/consumer",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_inherited_ref_test_base() {
    let legacy = resolve_fixture("data/inherited-ref-test/base", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/inherited-ref-test/base", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_inherited_ref_test_middle() {
    let legacy = resolve_fixture("data/inherited-ref-test/middle", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/inherited-ref-test/middle", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_inherited_ref_test_refiner() {
    let legacy = resolve_fixture("data/inherited-ref-test/refiner", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/inherited-ref-test/refiner", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_refinement_attribute_precedence() {
    let registry_spec = r#"
file_format: definition/2
attributes:
  - key: test.a
    type: string
    brief: Def a
    stability: stable
  - key: test.b
    type: string
    brief: Def b
    stability: stable
attribute_groups:
  - id: parent.group
    visibility: public
    brief: Parent group
    stability: stable
    attributes:
      - ref: test.a
        brief: parent a
      - ref: test.b
        brief: parent b
  - id: override.group
    visibility: public
    brief: Override group
    stability: stable
    attributes:
      - ref: test.a
        brief: override a
      - ref: test.b
        brief: override b
spans:
  - type: base.span
    requirement_level: recommended
    kind: client
    name:
      note: base span
    brief: Base span
    stability: stable
    attributes:
      - ref_group: parent.group
span_refinements:
  - id: span.refined
    ref: base.span
    brief: Refined span
    attributes:
      - ref_group: override.group
      - ref: test.b
        brief: inline b
"#;

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = temp_dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");
    std::fs::write(reg_dir.join("schema.yaml"), registry_spec).expect("write");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };
    let run_resolver = |target: ResolutionTarget| {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => into_v2(&r),
            WResult::FatalErr(e) => panic!("Target {target:?} failed: {e}"),
        }
    };

    let legacy_v2 = run_resolver(ResolutionTarget::V1);
    let native_v2 = run_resolver(ResolutionTarget::V2);

    assert_v2_schema_eq(&native_v2, &legacy_v2);
}

#[test]
fn test_parity_error_entity_identity_inheritance_unref() {
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_fixture("data/entity-identity-inheritance-unref", engine);
        assert!(res.is_err(), "Expected error for {engine:?}");
    }
}

#[test]
fn test_parity_error_identity_demote() {
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_fixture("data/registry-test-v2-6-identity-demote", engine);
        assert!(res.is_err(), "Expected error for {engine:?}");
    }
}

#[test]
fn test_parity_error_identity_promote() {
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_fixture("data/registry-test-v2-7-identity-promote", engine);
        assert!(res.is_err(), "Expected error for {engine:?}");
    }
}

#[test]
fn test_parity_error_identity_add() {
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_fixture("data/registry-test-v2-8-identity-add", engine);
        assert!(res.is_err(), "Expected error for {engine:?}");
    }
}

#[test]
fn test_parity_error_identity_refinement_no_identity() {
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_fixture(
            "data/registry-test-v2-9-identity-refinement-no-identity",
            engine,
        );
        assert!(res.is_err(), "Expected error for {engine:?}");
    }
}

#[test]
fn test_parity_refinement_ref_group_validation_errors() {
    let unref_dup_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
    attributes:
      - ref: a
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - unref: a
      - unref: a
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(unref_dup_yaml, engine).into_result_failing_non_fatal();
        assert!(res.is_err(), "duplicate unref should fail in {engine:?}");
    }

    let unref_inline_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
    attributes:
      - ref: a
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - unref: a
      - ref: a
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(unref_inline_yaml, engine).into_result_failing_non_fatal();
        assert!(res.is_err(), "unref + inline ref should fail in {engine:?}");
    }

    let unref_group_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
attribute_groups:
  - id: g
    visibility: internal
    attributes:
      - ref: a
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
    attributes:
      - ref: a
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - unref: a
      - ref_group: g
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(unref_group_yaml, engine).into_result_failing_non_fatal();
        assert!(res.is_err(), "unref + ref_group should fail in {engine:?}");
    }

    let unref_required_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
    attributes:
      - ref: a
        requirement_level: required
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - unref: a
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(unref_required_yaml, engine).into_result_failing_non_fatal();
        assert!(
            res.is_err(),
            "unref required attr should fail in {engine:?}"
        );
    }

    let unref_non_inherited_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - unref: a
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res =
            resolve_from_yaml(unref_non_inherited_yaml, engine).into_result_failing_non_fatal();
        assert!(
            res.is_err(),
            "unref non-inherited attr should fail in {engine:?}"
        );
    }

    let unresolved_ref_group_yaml = r#"
file_format: definition/2
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - ref_group: does.not.exist
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res =
            resolve_from_yaml(unresolved_ref_group_yaml, engine).into_result_failing_non_fatal();
        assert!(
            res.is_err(),
            "unresolved ref_group should fail in {engine:?}"
        );
    }

    let excluded_ref_group_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
attribute_groups:
  - id: g.excluded
    visibility: public
    brief: g
    stability: stable
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: a
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - ref_group: g.excluded
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res =
            resolve_from_yaml(excluded_ref_group_yaml, engine).into_result_failing_non_fatal();
        assert!(res.is_err(), "excluded ref_group should fail in {engine:?}");
    }

    let sibling_collision_yaml = r#"
file_format: definition/2
attributes:
  - key: a
    type: string
    brief: a
    stability: stable
attribute_groups:
  - id: g1
    visibility: internal
    attributes:
      - ref: a
  - id: g2
    visibility: internal
    attributes:
      - ref: a
metrics:
  - name: base.metric
    brief: base
    instrument: counter
    unit: '1'
    stability: stable
metric_refinements:
  - id: refined.metric
    ref: base.metric
    attributes:
      - ref_group: g1
      - ref_group: g2
"#;
    for engine in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let res = resolve_from_yaml(sibling_collision_yaml, engine).into_result_failing_non_fatal();
        assert!(
            res.is_err(),
            "sibling ref_group collision should fail in {engine:?}"
        );
    }
}

#[test]
fn test_parity_group_import_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/group_import_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/group_import_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_signal_import_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/signal_import_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/signal_import_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_v1_dep_registry() {
    let legacy = resolve_fixture(
        "data/registry-test-v2-dep/v1_dep_registry",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/registry-test-v2-dep/v1_dep_registry",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_top_diamond() {
    let legacy = resolve_fixture(
        "data/entity-assoc-imports/top_diamond",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/entity-assoc-imports/top_diamond",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_top_rival_import() {
    let _legacy = resolve_fixture(
        "data/entity-assoc-imports/top_rival_import",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native_err = resolve_fixture(
        "data/entity-assoc-imports/top_rival_import",
        ResolutionTarget::V2,
    )
    .expect_err("Native V2 treats rival imported definitions as fatal");
    let msg = native_err.to_string();
    assert!(
        msg.contains("entity.host") && msg.contains("host"),
        "Expected duplicate entity.host / host error, got: {msg}"
    );
}

#[test]
fn test_parity_top_rebind() {
    let legacy = resolve_fixture("data/entity-assoc-imports/top_rebind", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/entity-assoc-imports/top_rebind", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_top_legacy_import() {
    let legacy = resolve_fixture(
        "data/entity-assoc-imports/top_legacy_import",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/entity-assoc-imports/top_legacy_import",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_middle() {
    let legacy = resolve_fixture("data/entity-assoc-imports/middle", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/entity-assoc-imports/middle", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_middle_reexport() {
    let legacy = resolve_fixture(
        "data/entity-assoc-imports/middle_reexport",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/entity-assoc-imports/middle_reexport",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_compatible_version_conflict() {
    let legacy = resolve_fixture(
        "data/compatible-version-conflict/main",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/compatible-version-conflict/main",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_transitive_provenance() {
    let legacy = resolve_fixture("data/transitive-provenance/main", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/transitive-provenance/main", ResolutionTarget::V2)
        .expect("Native resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_include_unreferenced() {
    let test_dir = "data/registry-test-v2-dep/deep_registry";
    let registry_subfolder = Path::new(test_dir).join("registry");
    let target_path = if registry_subfolder.is_dir() {
        registry_subfolder.display().to_string().replace('\\', "/")
    } else {
        test_dir.replace('\\', "/")
    };
    let location = VirtualDirectoryPath::LocalFolder { path: target_path };

    let resolve_with_include_unref = |target: ResolutionTarget| -> (V2Schema, Vec<Error>) {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo creation");
        let config = WeaverResolverConfig::default()
            .with_target(target)
            .with_include_unreferenced(true);
        let mut resolver = WeaverResolver::new(config);
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::OkWithNFEs(schema, nfes) => (into_v2(&schema), nfes),
            WResult::Ok(schema) => (into_v2(&schema), Vec::new()),
            WResult::FatalErr(e) => {
                panic!("Expected Ok or OkWithNFEs for {target:?}, got FatalErr({e})")
            }
        }
    };

    let (mut legacy_schema, legacy_nfes) = resolve_with_include_unref(ResolutionTarget::V1);
    let (native_schema, native_nfes) = resolve_with_include_unref(ResolutionTarget::V2);

    assert!(
        legacy_nfes
            .iter()
            .any(|e| matches!(e, Error::DeprecatedIncludeUnreferencedWarning {})),
        "Expected DeprecatedIncludeUnreferencedWarning for Legacy"
    );
    assert!(
        native_nfes
            .iter()
            .any(|e| matches!(e, Error::DeprecatedIncludeUnreferencedWarning {})),
        "Expected DeprecatedIncludeUnreferencedWarning for Native"
    );

    // Native V2 sorts refinements by normalized `id`, matching forge output order.
    let metric_ids: Vec<&str> = native_schema
        .refinements
        .metrics
        .iter()
        .map(|r| r.id.as_ref())
        .collect();
    let mut sorted_metric_ids = metric_ids.clone();
    sorted_metric_ids.sort();
    assert_eq!(metric_ids, sorted_metric_ids);

    legacy_schema
        .refinements
        .metrics
        .sort_by(|a, b| a.id.cmp(&b.id));
    legacy_schema
        .refinements
        .spans
        .sort_by(|a, b| a.id.cmp(&b.id));
    legacy_schema
        .refinements
        .events
        .sort_by(|a, b| a.id.cmp(&b.id));
    legacy_schema
        .refinements
        .entities
        .sort_by(|a, b| a.id.cmp(&b.id));

    assert_v2_schema_eq(&native_schema, &legacy_schema);
}

#[test]
fn test_parity_unmatched_imports() {
    for fixture in ["top_bad_import", "top_bad_imports_all_types"] {
        let path = format!("data/entity-assoc-imports/{fixture}");
        let location = VirtualDirectoryPath::LocalFolder { path };

        let mut legacy_unmatched = Vec::new();
        let mut native_unmatched = Vec::new();

        for (target, out) in [
            (ResolutionTarget::V1, &mut legacy_unmatched),
            (ResolutionTarget::V2, &mut native_unmatched),
        ] {
            let mut repo_nfes = Vec::new();
            let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).unwrap();
            let config = WeaverResolverConfig::default().with_target(target);
            let mut resolver = WeaverResolver::new(config);
            match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
                WResult::OkWithNFEs(_, nfes) => {
                    for nfe in nfes {
                        if let Error::UnmatchedImport { pattern, signal } = nfe {
                            out.push((pattern, signal));
                        }
                    }
                }
                WResult::FatalErr(e) => {
                    panic!("Expected OkWithNFEs for {target:?} on {fixture}, got {e}")
                }
                WResult::Ok(_) => {
                    panic!("Expected OkWithNFEs for {target:?} on {fixture}, got Ok without NFEs")
                }
            }
            out.sort();
        }

        assert_eq!(
            native_unmatched, legacy_unmatched,
            "Unmatched imports mismatch for {fixture}"
        );
    }
}

#[test]
fn test_parity_registry_test_v2_dep_additional() {
    for fixture in [
        "app_registry",
        "bare_ref_registry",
        "consumer_published",
        "consumer_registry",
        "span_import_registry",
    ] {
        let path = format!("data/registry-test-v2-dep/{fixture}");
        let legacy = resolve_fixture(&path, ResolutionTarget::V1)
            .unwrap_or_else(|e| panic!("Legacy resolution failed for `{fixture}`: {e}"));
        let native = resolve_fixture(&path, ResolutionTarget::V2)
            .unwrap_or_else(|e| panic!("Native resolution failed for `{fixture}`: {e}"));
        assert_v2_schema_eq(&native, &legacy);
    }
}

#[test]
fn test_parity_entity_assoc_success_cases() {
    for fixture in [
        "local_shapes",
        "local_private_pair",
        "middle_refined_signal",
        "middle_refinement_assoc",
        "top_diamond_assoc",
        "top_private_first",
        "top_public_first",
    ] {
        let path = format!("data/entity-assoc-imports/{fixture}");
        let legacy = resolve_fixture(&path, ResolutionTarget::V1)
            .unwrap_or_else(|e| panic!("Legacy resolution failed for `{fixture}`: {e}"));
        let native = resolve_fixture(&path, ResolutionTarget::V2)
            .unwrap_or_else(|e| panic!("Native resolution failed for `{fixture}`: {e}"));
        assert_v2_schema_eq(&native, &legacy);
    }
}

fn unwrap_compound_error(mut cur: &Error) -> &Error {
    while let Error::CompoundError(errs) = cur {
        if let Some(first) = errs.first() {
            cur = first;
        } else {
            break;
        }
    }
    cur
}

#[test]
fn test_parity_entity_assoc_fatal_error_cases() {
    for fixture in [
        "middle_bad_assoc",
        "middle_excluded_assoc",
        "top_ambiguous_assoc",
        "top_transitive_assoc",
        "local_private_assoc",
        "middle_name_clash",
        "legacy_by_id",
    ] {
        let path = format!("data/entity-assoc-imports/{fixture}");
        let legacy_err = resolve_fixture(&path, ResolutionTarget::V1)
            .expect_err(&format!("Legacy expected error for {fixture}"));
        let native_err = resolve_fixture(&path, ResolutionTarget::V2)
            .expect_err(&format!("Native expected error for {fixture}"));

        if fixture == "middle_excluded_assoc" {
            assert!(
                matches!(
                    unwrap_compound_error(&native_err),
                    Error::UnresolvedEntityAssociation { entity_type, .. } if entity_type == "host"
                ),
                "Native expected UnresolvedEntityAssociation for middle_excluded_assoc: {native_err:?}"
            );
            assert!(
                matches!(
                    unwrap_compound_error(&legacy_err),
                    Error::ExcludedFromDependencyResolution { id, .. } if id == "host"
                ),
                "Legacy expected ExcludedFromDependencyResolution for middle_excluded_assoc: {legacy_err:?}"
            );
            continue;
        }

        match (
            unwrap_compound_error(&native_err),
            unwrap_compound_error(&legacy_err),
        ) {
            (
                Error::UnresolvedEntityAssociation {
                    entity_type: n_type,
                    ..
                },
                Error::UnresolvedEntityAssociation {
                    entity_type: l_type,
                    ..
                },
            ) => {
                assert_eq!(
                    n_type, l_type,
                    "UnresolvedEntityAssociation type mismatch for {fixture}"
                );
            }
            (
                Error::ExcludedFromDependencyResolution {
                    id: n_id,
                    used_in: n_used,
                    r#type: n_type,
                    ..
                },
                Error::ExcludedFromDependencyResolution {
                    id: l_id,
                    used_in: l_used,
                    r#type: l_type,
                    ..
                },
            ) => {
                assert_eq!(
                    n_id, l_id,
                    "ExcludedFromDependencyResolution id mismatch for {fixture}"
                );
                assert_eq!(
                    n_used, l_used,
                    "ExcludedFromDependencyResolution used_in mismatch for {fixture}"
                );
                assert_eq!(
                    n_type, l_type,
                    "ExcludedFromDependencyResolution type mismatch for {fixture}"
                );
            }
            (
                Error::AmbiguousEntityAssociation {
                    entity_type: n_type,
                    registries: n_regs,
                    ..
                },
                Error::AmbiguousEntityAssociation {
                    entity_type: l_type,
                    registries: l_regs,
                    ..
                },
            ) => {
                assert_eq!(
                    n_type, l_type,
                    "AmbiguousEntityAssociation type mismatch for {fixture}"
                );
                let mut n_sorted = n_regs.clone();
                n_sorted.sort();
                let mut l_sorted = l_regs.clone();
                l_sorted.sort();
                assert_eq!(
                    n_sorted, l_sorted,
                    "AmbiguousEntityAssociation registries mismatch for {fixture}"
                );
            }
            _ => panic!(
                "Error variant mismatch for {fixture}:\nNative: {native_err:?}\nLegacy: {legacy_err:?}"
            ),
        }
    }
}

#[test]
fn test_parity_compatible_version_conflict_published() {
    let legacy = resolve_fixture(
        "data/compatible-version-conflict-published/main",
        ResolutionTarget::V1,
    )
    .expect("Legacy resolution succeeded");
    let native = resolve_fixture(
        "data/compatible-version-conflict-published/main",
        ResolutionTarget::V2,
    )
    .expect("Native resolution succeeded");
    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_correct_provenance() {
    let legacy = resolve_fixture("data/correct-provenance/main", ResolutionTarget::V1)
        .expect("Legacy resolution succeeded");
    let native = resolve_fixture("data/correct-provenance/main", ResolutionTarget::V2)
        .expect("Native resolution succeeded");
    assert_v2_schema_eq(&native, &legacy);
}

#[test]
fn test_parity_dep_exclusion_migration() {
    for fixture in ["migration_parent", "migration_consumer"] {
        let path = format!("data/registry-test-dep-exclusion/{fixture}");
        let legacy = resolve_fixture(&path, ResolutionTarget::V1)
            .unwrap_or_else(|e| panic!("Legacy failed for {fixture}: {e}"));
        let native = resolve_fixture(&path, ResolutionTarget::V2)
            .unwrap_or_else(|e| panic!("Native failed for {fixture}: {e}"));
        assert_v2_schema_eq(&native, &legacy);
    }
}

#[test]
fn test_parity_local_vs_imported_collision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep_dir = dir.path().join("dep/registry");
    let root_dir = dir.path().join("root/registry");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::create_dir_all(&root_dir).expect("create root dir");

    std::fs::write(
        dep_dir.join("manifest.yaml"),
        r#"file_format: definition_manifest/2.0
schema_url: https://dep.example.com/schemas/1.0.0
description: Dep
stability: stable
"#,
    )
    .expect("write dep manifest");

    std::fs::write(
        dep_dir.join("metrics.yaml"),
        r#"file_format: definition/2
metrics:
  - name: shared.metric
    brief: Dep metric
    instrument: counter
    unit: '1'
    stability: stable
"#,
    )
    .expect("write dep metric");

    let dep_path = dep_dir.display().to_string();
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            r#"file_format: definition_manifest/2.0
schema_url: https://root.example.com/schemas/1.0.0
description: Root
dependencies:
  - schema_url: https://dep.example.com/schemas/1.0.0
    registry_path: {dep_path}
stability: stable
"#
        ),
    )
    .expect("write root manifest");

    std::fs::write(
        root_dir.join("metrics.yaml"),
        r#"file_format: definition/2
imports:
  metrics:
    - shared.metric
metrics:
  - name: shared.metric
    brief: Root metric
    instrument: counter
    unit: '1'
    stability: stable
"#,
    )
    .expect("write root metric");

    let location = VirtualDirectoryPath::LocalFolder {
        path: root_dir.display().to_string(),
    };

    let collect_dups = |errs: Vec<Error>| -> Vec<String> {
        let mut dups = Vec::new();
        for err in errs {
            match err {
                Error::DuplicateGroupId { group_id, .. } => {
                    dups.push(format!("group:{group_id}"));
                }
                Error::DuplicateMetricName { metric_name, .. } => {
                    dups.push(format!("metric:{metric_name}"));
                }
                Error::CompoundError(inner) => {
                    for sub in inner {
                        match sub {
                            Error::DuplicateGroupId { group_id, .. } => {
                                dups.push(format!("group:{group_id}"));
                            }
                            Error::DuplicateMetricName { metric_name, .. } => {
                                dups.push(format!("metric:{metric_name}"));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        dups.sort();
        dups
    };

    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut legacy_resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V1));
    let legacy_dups = match legacy_resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
        WResult::OkWithNFEs(_, nfes) => collect_dups(nfes),
        WResult::FatalErr(e) => panic!("Expected OkWithNFEs for V1, got FatalErr({e})"),
        WResult::Ok(_) => panic!("Expected OkWithNFEs for V1, got Ok without NFEs"),
    };

    let mut repo_nfes = Vec::new();
    let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
    let mut native_resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let native_dups = match native_resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
        WResult::FatalErr(e) => collect_dups(vec![e]),
        WResult::Ok(_) | WResult::OkWithNFEs(..) => {
            panic!("Expected FatalErr for local-vs-imported collision in native V2")
        }
    };

    assert_eq!(
        legacy_dups,
        vec!["group:metric.shared.metric", "metric:shared.metric"]
    );
    assert_eq!(native_dups, legacy_dups);
}

#[test]
fn test_parity_v1_dependency_signal_refinement_with_entity_associations() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep_dir = dir.path().join("v1dep/registry");
    let root_dir = dir.path().join("root/registry");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::create_dir_all(&root_dir).expect("create root dir");

    std::fs::write(
        dep_dir.join("registry_manifest.yaml"),
        r#"name: v1dep
schema_url: https://v1dep.example.com/schemas/1.0.0
description: V1 Dependency
"#,
    )
    .expect("write v1 dep manifest");

    std::fs::write(
        dep_dir.join("v1_group.yaml"),
        r#"groups:
  - id: resource.device
    type: resource
    name: device
    brief: A device
    attributes:
      - id: device.id
        type: string
        role: identifying
        brief: Device ID
        stability: stable
  - id: metric.device.boot
    type: metric
    metric_name: device.boot
    instrument: counter
    unit: '1'
    brief: Device boot count
    stability: stable
    entity_associations:
      - device
"#,
    )
    .expect("write v1 dep groups");

    let dep_path = dep_dir.display().to_string();
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            r#"file_format: definition_manifest/2.0
schema_url: https://root.example.com/schemas/1.0.0
description: Root registry
dependencies:
  - schema_url: https://v1dep.example.com/schemas/1.0.0
    registry_path: {dep_path}
stability: stable
"#
        ),
    )
    .expect("write root manifest");

    std::fs::write(
        root_dir.join("schema.yaml"),
        r#"file_format: definition/2
attributes:
  - key: boot.type
    type: string
    brief: Type of boot
    stability: stable
metric_refinements:
  - id: custom.device.boot
    ref: device.boot
    brief: Refined device boot
    attributes:
      - ref: boot.type
"#,
    )
    .expect("write root schema");

    let location = VirtualDirectoryPath::LocalFolder {
        path: root_dir.display().to_string(),
    };

    let run_resolver = |target: ResolutionTarget| -> V2Schema {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => into_v2(&r),
            WResult::FatalErr(e) => panic!("Target {target:?} failed: {e}"),
        }
    };

    let legacy_v2 = run_resolver(ResolutionTarget::V1);
    let native_v2 = run_resolver(ResolutionTarget::V2);

    assert_v2_schema_eq(&native_v2, &legacy_v2);
}

#[test]
fn test_parity_deprecated_signal_ref_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: gen_ai.system
    type: string
    brief: The system name.
    stability: stable
  - key: gen_ai.request.model
    type: string
    brief: Model name.
    stability: stable
  - key: event.detail
    type: string
    brief: Additional event details.
    stability: stable
attribute_groups:
  - id: gen_ai.client.attributes
    visibility: internal
    attributes:
      - ref: gen_ai.system
spans:
  - type: gen_ai.client.inference
    kind: client
    brief: Generative AI client inference span.
    stability: stable
    name:
      note: Client inference span
    attributes:
      - ref_group: gen_ai.client.attributes
      - ref: gen_ai.request.model
events:
  - name: gen_ai.client.inference.operation.details
    brief: Details of client inference operation.
    stability: stable
    attributes:
      - ref_group: span.gen_ai.client.inference
      - ref: event.detail
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };

    let run_resolver = |target: ResolutionTarget| -> (V2Schema, Vec<Error>) {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        let result = resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor);

        match result {
            WResult::OkWithNFEs(r, nfes) => {
                let failing_res =
                    WResult::OkWithNFEs((), nfes.clone()).into_result_failing_non_fatal();
                assert!(
                    failing_res.is_err(),
                    "Target {target:?} should fail under --future due to DeprecatedSignalRefGroup"
                );
                (into_v2(&r), nfes)
            }
            WResult::Ok(r) => (into_v2(&r), Vec::new()),
            WResult::FatalErr(e) => panic!("Target {target:?} failed unexpectedly: {e}"),
        }
    };

    let (legacy_v2, legacy_nfes) = run_resolver(ResolutionTarget::V1);
    let (native_v2, native_nfes) = run_resolver(ResolutionTarget::V2);

    assert_v2_schema_eq(&native_v2, &legacy_v2);

    let assert_has_warning = |nfes: &[Error], engine: &str| {
        let found = nfes.iter().any(|e| match e {
            Error::DeprecatedSignalRefGroup {
                used_in,
                ref_group,
                signal_type,
                ..
            } => {
                used_in == "event.gen_ai.client.inference.operation.details"
                    && ref_group == "span.gen_ai.client.inference"
                    && signal_type == "span"
            }
            _ => false,
        });
        assert!(
            found,
            "Engine {engine} missing expected DeprecatedSignalRefGroup warning, got: {nfes:?}"
        );
    };

    assert_has_warning(&legacy_nfes, "Legacy");
    assert_has_warning(&native_nfes, "Native");
}

#[test]
fn test_parity_duplicate_inline_attribute_ref_is_fatal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: experimental.attr
    type: string
    brief: experimental attribute
    stability: stable
metrics:
  - name: stable.metric
    brief: stable metric
    stability: stable
    instrument: counter
    unit: "1"
    attributes:
      - ref: experimental.attr
      - ref: experimental.attr
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let location = VirtualDirectoryPath::LocalFolder {
        path: reg_dir.display().to_string(),
    };

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(None, &location, &mut repo_nfes).expect("repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
        let result = resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor);

        match result {
            WResult::FatalErr(Error::FailToResolveDefinition(
                weaver_semconv::Error::CompoundError(errs),
            )) => {
                assert!(
                    errs.iter().any(|e| matches!(
                        e,
                        weaver_semconv::Error::DuplicateAttributeRef { attr, context, .. }
                            if attr == "experimental.attr" && context == "metric 'stable.metric'"
                    )),
                    "Target {target:?} expected DuplicateAttributeRef in CompoundError, got: {errs:?}"
                );
            }
            WResult::FatalErr(Error::FailToResolveDefinition(
                weaver_semconv::Error::DuplicateAttributeRef { attr, context, .. },
            )) => {
                assert_eq!(attr, "experimental.attr");
                assert_eq!(context, "metric 'stable.metric'");
            }
            WResult::Ok(_) | WResult::OkWithNFEs(..) => {
                panic!("Target {target:?} expected FatalErr for duplicate inline ref, got Ok");
            }
            WResult::FatalErr(other) => {
                panic!("Target {target:?} unexpected FatalErr: {other}");
            }
        }
    }
}

#[test]
fn test_parity_nested_annotation_overrides_in_catalog() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: acme.checkout.stage
    type: string
    brief: Stage the checkout has reached.
    stability: stable
    examples: ["cart"]
    annotations:
      acme:
        source: catalog
attribute_groups:
  - id: acme.step
    visibility: public
    brief: Checkout step attributes.
    stability: stable
    attributes:
      - ref: acme.checkout.stage
        annotations:
          acme:
            source: group-step
spans:
  - type: acme.checkout
    kind: server
    brief: One checkout request.
    stability: stable
    name:
      note: The constant checkout.
    attributes:
      - ref: acme.checkout.stage
        requirement_level: recommended
        annotations:
          acme:
            source: span
metrics:
  - name: acme.cart.items
    instrument: counter
    unit: "{item}"
    brief: Items added to carts.
    stability: stable
    attributes:
      - ref: acme.checkout.stage
        requirement_level: recommended
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let legacy = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V1)
        .expect("V1 resolution succeeded");
    let native = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V2)
        .expect("V2 resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
    assert_eq!(
        native.attribute_catalog.len(),
        3,
        "Catalog must preserve all three distinct nested annotation variants"
    );
    let span_attr_idx = native.registry.spans[0].attributes[0].base.0 as usize;
    assert_eq!(
        native.attribute_catalog[span_attr_idx].common.annotations["acme"].0["source"],
        serde_json::json!("span")
    );
}

#[test]
fn test_parity_diamond_checked_upgrade() {
    let dir = tempfile::tempdir().expect("tempdir");
    let c1_dir = dir.path().join("c_1_1").join("registry");
    let c2_dir = dir.path().join("c_1_2").join("registry");
    let a_dir = dir.path().join("a").join("registry");
    let root_dir = dir.path().join("root").join("registry");

    std::fs::create_dir_all(&c1_dir).expect("create c1");
    std::fs::create_dir_all(&c2_dir).expect("create c2");
    std::fs::create_dir_all(&a_dir).expect("create a");
    std::fs::create_dir_all(&root_dir).expect("create root");

    // C 1.1.0
    std::fs::write(
        c1_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.1.0\n",
    )
    .expect("write c1 manifest");
    std::fs::write(
        c1_dir.join("schema.yaml"),
        r#"file_format: definition/2
attributes:
  - key: c.attr
    type: string
    brief: Attribute c in v1.1.0
    stability: experimental
"#,
    )
    .expect("write c1 schema");

    // C 1.2.0 (upgraded)
    std::fs::write(
        c2_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.2.0\n",
    )
    .expect("write c2 manifest");
    std::fs::write(
        c2_dir.join("schema.yaml"),
        r#"file_format: definition/2
attributes:
  - key: c.attr
    type: string
    brief: Attribute c in v1.2.0 (upgraded)
    stability: stable
"#,
    )
    .expect("write c2 schema");

    // A 0.1.0 (depends on C 1.1.0)
    std::fs::write(
        a_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/a/0.1.0\ndependencies:\n  - schema_url: https://example.com/c/1.1.0\n    registry_path: {}\n",
            c1_dir.display()
        ),
    )
    .expect("write a manifest");
    std::fs::write(
        a_dir.join("schema.yaml"),
        r#"file_format: definition/2
metrics:
  - name: a.metric
    instrument: counter
    unit: "{item}"
    brief: Metric in A
    stability: stable
    attributes:
      - ref: c.attr
        requirement_level: required
"#,
    )
    .expect("write a schema");

    // Root 0.1.0 (depends on A 0.1.0 and C 1.2.0)
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/root/0.1.0\ndependencies:\n  - schema_url: https://example.com/a/0.1.0\n    registry_path: {}\n  - schema_url: https://example.com/c/1.2.0\n    registry_path: {}\n",
            a_dir.display(),
            c2_dir.display()
        ),
    )
    .expect("write root manifest");
    std::fs::write(
        root_dir.join("schema.yaml"),
        r#"file_format: definition/2
imports:
  metrics:
    - a.metric
"#,
    )
    .expect("write root schema");

    let legacy = resolve_fixture(
        root_dir.parent().unwrap().to_str().unwrap(),
        ResolutionTarget::V1,
    )
    .expect("V1 resolution succeeded");
    let native = resolve_fixture(
        root_dir.parent().unwrap().to_str().unwrap(),
        ResolutionTarget::V2,
    )
    .expect("V2 resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
    assert_eq!(native.registry.metrics.len(), 1);
    let m = &native.registry.metrics[0];
    assert_eq!(&*m.name, "a.metric");
    assert_eq!(m.attributes.len(), 1);
    assert_eq!(
        m.attributes[0].requirement_level,
        weaver_semconv::v2::attribute::RequirementLevel::Basic(
            weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required
        )
    );
    let cat_attr = &native.attribute_catalog[m.attributes[0].base.0 as usize];
    assert_eq!(cat_attr.key, "c.attr");
    assert_eq!(cat_attr.common.brief, "Attribute c in v1.2.0 (upgraded)");
    assert_eq!(
        cat_attr.common.stability,
        weaver_semconv::v2::stability::Stability::Stable
    );
}

#[test]
fn test_parity_upgrade_newly_excluded_items_fail() {
    for kind in ["attribute", "entity", "metric"] {
        let dir = tempfile::tempdir().expect("tempdir");
        let c1_dir = dir.path().join("c_1_1").join("registry");
        let c2_dir = dir.path().join("c_1_2").join("registry");
        let a_dir = dir.path().join("a").join("registry");
        let root_dir = dir.path().join("root").join("registry");

        std::fs::create_dir_all(&c1_dir).expect("create c1");
        std::fs::create_dir_all(&c2_dir).expect("create c2");
        std::fs::create_dir_all(&a_dir).expect("create a");
        std::fs::create_dir_all(&root_dir).expect("create root");

        std::fs::write(
            c1_dir.join("manifest.yaml"),
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.1.0\n",
        )
        .expect("write c1 manifest");
        std::fs::write(
            c2_dir.join("manifest.yaml"),
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.2.0\n",
        )
        .expect("write c2 manifest");

        match kind {
            "attribute" => {
                std::fs::write(
                    c1_dir.join("schema.yaml"),
                    "file_format: definition/2\nattributes:\n  - key: c.item\n    type: string\n    brief: item\n    stability: stable\n",
                )
                .expect("write c1 schema");
                std::fs::write(
                    c2_dir.join("schema.yaml"),
                    "file_format: definition/2\nattributes:\n  - key: c.item\n    type: string\n    brief: item\n    stability: stable\n    annotations:\n      dependency_resolution:\n        exclude: true\n",
                )
                .expect("write c2 schema");
                std::fs::write(
                    a_dir.join("schema.yaml"),
                    "file_format: definition/2\nmetrics:\n  - name: a.signal\n    instrument: counter\n    unit: '1'\n    brief: sig\n    stability: stable\n    attributes:\n      - ref: c.item\n",
                )
                .expect("write a schema");
                std::fs::write(
                    root_dir.join("schema.yaml"),
                    "file_format: definition/2\nimports:\n  metrics:\n    - a.signal\n",
                )
                .expect("write root schema");
            }
            "entity" => {
                std::fs::write(
                    c1_dir.join("schema.yaml"),
                    "file_format: definition/2\nattributes:\n  - key: c.id\n    type: string\n    brief: id\n    stability: stable\nentities:\n  - type: c.item\n    brief: entity\n    stability: stable\n    identity:\n      - ref: c.id\n",
                )
                .expect("write c1 schema");
                std::fs::write(
                    c2_dir.join("schema.yaml"),
                    "file_format: definition/2\nattributes:\n  - key: c.id\n    type: string\n    brief: id\n    stability: stable\nentities:\n  - type: c.item\n    brief: entity\n    stability: stable\n    identity:\n      - ref: c.id\n    annotations:\n      dependency_resolution:\n        exclude: true\n",
                )
                .expect("write c2 schema");
                std::fs::write(
                    a_dir.join("schema.yaml"),
                    "file_format: definition/2\nspans:\n  - type: a.signal\n    kind: server\n    brief: sig\n    stability: stable\n    name:\n      note: sig name\n    entity_associations:\n      - c.item\n",
                )
                .expect("write a schema");
                std::fs::write(
                    root_dir.join("schema.yaml"),
                    "file_format: definition/2\nimports:\n  spans:\n    - a.signal\n",
                )
                .expect("write root schema");
            }
            "metric" => {
                std::fs::write(
                    c1_dir.join("schema.yaml"),
                    "file_format: definition/2\nmetrics:\n  - name: c.item\n    instrument: counter\n    unit: '1'\n    brief: metric\n    stability: stable\n",
                )
                .expect("write c1 schema");
                std::fs::write(
                    c2_dir.join("schema.yaml"),
                    "file_format: definition/2\nmetrics:\n  - name: c.item\n    instrument: counter\n    unit: '1'\n    brief: metric\n    stability: stable\n    annotations:\n      dependency_resolution:\n        exclude: true\n",
                )
                .expect("write c2 schema");
                std::fs::write(
                    a_dir.join("schema.yaml"),
                    "file_format: definition/2\nimports:\n  metrics:\n    - c.item\n",
                )
                .expect("write a schema");
                std::fs::write(
                    root_dir.join("schema.yaml"),
                    "file_format: definition/2\nmetric_refinements:\n  - id: root.refinement\n    ref: c.item\n    brief: refined in root\n",
                )
                .expect("write root schema");
            }
            _ => unreachable!(),
        }

        std::fs::write(
            a_dir.join("manifest.yaml"),
            format!(
                "file_format: definition_manifest/2.0\nschema_url: https://example.com/a/0.1.0\ndependencies:\n  - schema_url: https://example.com/c/1.1.0\n    registry_path: {}\n",
                c1_dir.display()
            ),
        )
        .expect("write a manifest");
        std::fs::write(
            root_dir.join("manifest.yaml"),
            format!(
                "file_format: definition_manifest/2.0\nschema_url: https://example.com/root/0.1.0\ndependencies:\n  - schema_url: https://example.com/a/0.1.0\n    registry_path: {}\n  - schema_url: https://example.com/c/1.2.0\n    registry_path: {}\n",
                a_dir.display(),
                c2_dir.display()
            ),
        )
        .expect("write root manifest");

        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let res = resolve_fixture(root_dir.parent().unwrap().to_str().unwrap(), target);
            match kind {
                "attribute" => match res {
                    Err(Error::AttributeNotFoundInUpgradedSchema {
                        attribute_name,
                        original_url,
                        upgraded_url,
                    }) => {
                        assert_eq!(attribute_name, "c.item");
                        assert_eq!(original_url, "https://example.com/c/1.1.0");
                        assert_eq!(upgraded_url, "https://example.com/c/1.2.0");
                    }
                    other => panic!(
                        "Target {target:?} expected AttributeNotFoundInUpgradedSchema, got {other:?}"
                    ),
                },
                "entity" => match res {
                    Err(Error::EntityNotFoundInUpgradedSchema {
                        entity_name,
                        original_url,
                        upgraded_url,
                    }) => {
                        assert_eq!(entity_name, "c.item");
                        assert_eq!(original_url, "https://example.com/c/1.1.0");
                        assert_eq!(upgraded_url, "https://example.com/c/1.2.0");
                    }
                    other => panic!(
                        "Target {target:?} expected EntityNotFoundInUpgradedSchema, got {other:?}"
                    ),
                },
                "metric" => match res {
                    Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id,
                        original_url,
                        upgraded_url,
                    }) => {
                        assert!(
                            group_id == "c.item" || group_id == "metric.c.item",
                            "unexpected group_id: {group_id}"
                        );
                        assert_eq!(original_url, "https://example.com/c/1.1.0");
                        assert_eq!(upgraded_url, "https://example.com/c/1.2.0");
                    }
                    other => panic!(
                        "Target {target:?} expected GroupNotFoundInUpgradedSchema, got {other:?}"
                    ),
                },
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn test_parity_internal_group_exclusion_taint_fails_on_public_signal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: secret.token
    type: string
    brief: Secret token
    stability: stable
attribute_groups:
  - id: base.secret
    visibility: public
    brief: Secret base
    stability: stable
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: secret.token
  - id: middle.internal
    visibility: internal
    attributes:
      - ref_group: base.secret
spans:
  - type: public.span
    kind: server
    brief: Public span
    stability: stable
    name:
      note: public span name
    attributes:
      - ref_group: middle.internal
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let res = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V2);
    match res {
        Err(Error::ExcludedFromDependencyResolution {
            id,
            r#type,
            used_in,
            provenance,
        }) => {
            assert_eq!(id, "base.secret");
            assert_eq!(r#type, "attribute_group");
            assert!(
                used_in == "public.span" || used_in == "span.public.span",
                "unexpected used_in: {used_in}"
            );
            assert!(
                provenance.is_some(),
                "expected provenance on ExcludedFromDependencyResolution"
            );
        }
        other => panic!("Expected ExcludedFromDependencyResolution, got {other:?}"),
    }
}

#[test]
fn test_v2_compound_refinement_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
spans:
  - type: test.span
    kind: server
    brief: Test span
    stability: stable
    name:
      note: test span
span_refinements:
  - id: test.span.ref
    ref: test.span
    attributes:
      - unref: missing.one
      - unref: missing.two
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let res = resolve_fixture_raw(dir.path().to_str().unwrap(), ResolutionTarget::V2);
    match res {
        WResult::FatalErr(Error::CompoundError(errors)) => {
            assert_eq!(errors.len(), 2);
            for err in &errors {
                assert!(matches!(err, Error::InvalidAttributeUnref { .. }));
            }
        }
        WResult::FatalErr(other) => {
            panic!("Expected FatalErr(CompoundError), got FatalErr({other})")
        }
        WResult::Ok(_) => panic!("Expected FatalErr(CompoundError), got Ok"),
        WResult::OkWithNFEs(_, nfes) => {
            panic!("Expected FatalErr(CompoundError), got OkWithNFEs({nfes:?})")
        }
    }
}

#[test]
fn test_v2_refinement_id_collision_is_fatal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
metrics:
  - name: test.metric
    instrument: counter
    unit: "1"
    brief: Base metric
    stability: stable
metric_refinements:
  - id: metric.test.metric
    ref: test.metric
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let res = resolve_fixture_raw(dir.path().to_str().unwrap(), ResolutionTarget::V2);
    match res {
        WResult::FatalErr(Error::DuplicateGroupId { group_id, .. }) => {
            assert_eq!(group_id, "test.metric");
        }
        WResult::FatalErr(other) => {
            panic!("Expected FatalErr(DuplicateGroupId), got FatalErr({other})")
        }
        WResult::Ok(_) | WResult::OkWithNFEs(..) => {
            panic!("Expected FatalErr for normalized refinement id collision")
        }
    }
}

#[test]
fn test_v2_duplicate_local_definitions_are_fatal() {
    let cases = [
        (
            "duplicate metric",
            r#"file_format: definition/2
metrics:
  - name: dup.metric
    instrument: counter
    unit: "1"
    brief: first
    stability: stable
  - name: dup.metric
    instrument: counter
    unit: "1"
    brief: second
    stability: stable
"#,
        ),
        (
            "duplicate span",
            r#"file_format: definition/2
spans:
  - type: dup.span
    kind: internal
    brief: first
    stability: stable
    name:
      note: first
  - type: dup.span
    kind: internal
    brief: second
    stability: stable
    name:
      note: second
"#,
        ),
        (
            "duplicate event",
            r#"file_format: definition/2
events:
  - name: dup.event
    brief: first
    stability: stable
  - name: dup.event
    brief: second
    stability: stable
"#,
        ),
        (
            "duplicate refinement",
            r#"file_format: definition/2
metrics:
  - name: base.metric
    instrument: counter
    unit: "1"
    brief: base
    stability: stable
metric_refinements:
  - id: dup.ref
    ref: base.metric
  - id: dup.ref
    ref: base.metric
"#,
        ),
    ];

    for (label, yaml) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let reg_dir = dir.path().join("registry");
        std::fs::create_dir_all(&reg_dir).expect("create_dir");
        std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");
        let res = resolve_fixture_raw(dir.path().to_str().unwrap(), ResolutionTarget::V2);
        assert!(
            matches!(res, WResult::FatalErr(_)),
            "Expected FatalErr for {label} in native V2"
        );
    }
}

#[test]
fn test_v2_refinement_id_colliding_with_imported_signal_is_fatal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep_dir = dir.path().join("dep/registry");
    let root_dir = dir.path().join("root/registry");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::create_dir_all(&root_dir).expect("create root dir");

    std::fs::write(
        dep_dir.join("manifest.yaml"),
        "description: dep\nschema_url: https://dep.example.com/schemas/1.0.0\n",
    )
    .expect("write dep manifest");
    std::fs::write(
        dep_dir.join("metrics.yaml"),
        r#"file_format: definition/2
metrics:
  - name: dep.metric
    instrument: counter
    unit: "1"
    brief: dep metric
    stability: stable
"#,
    )
    .expect("write dep metrics");

    let dep_path = dep_dir.to_string_lossy().replace('\\', "/");
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            "description: root\nschema_url: https://root.example.com/schemas/1.0.0\ndependencies:\n  - schema_url: https://dep.example.com/schemas/1.0.0\n    registry_path: {dep_path}\n"
        ),
    )
    .expect("write root manifest");
    std::fs::write(
        root_dir.join("metrics.yaml"),
        r#"file_format: definition/2
imports:
  metrics:
    - dep.metric
metrics:
  - name: local.metric
    instrument: counter
    unit: "1"
    brief: local metric
    stability: stable
metric_refinements:
  - id: dep.metric
    ref: local.metric
"#,
    )
    .expect("write root metrics");

    let res = resolve_fixture_raw(
        root_dir.parent().unwrap().to_str().unwrap(),
        ResolutionTarget::V2,
    );
    match res {
        WResult::FatalErr(Error::DuplicateGroupId { group_id, .. }) => {
            assert_eq!(group_id, "dep.metric");
        }
        WResult::FatalErr(other) => {
            panic!("Expected FatalErr(DuplicateGroupId), got FatalErr({other})")
        }
        WResult::Ok(_) | WResult::OkWithNFEs(..) => {
            panic!("Expected FatalErr when refinement id collides with imported signal")
        }
    }
}

#[test]
fn test_v2_refinement_entity_associations_empty_list_clears_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - key: host.id
    type: string
    brief: Host ID
    stability: stable
entities:
  - type: host
    brief: Host entity
    stability: stable
    identity:
      - ref: host.id
metrics:
  - name: base.metric
    instrument: counter
    unit: "1"
    brief: Base metric
    stability: stable
    entity_associations:
      - host
spans:
  - type: base.span
    kind: internal
    brief: Base span
    stability: stable
    name:
      note: base span
    entity_associations:
      - host
events:
  - name: base.event
    brief: Base event
    stability: stable
    entity_associations:
      - host
metric_refinements:
  - id: metric.inherits
    ref: base.metric
  - id: metric.clears
    ref: base.metric
    entity_associations: []
span_refinements:
  - id: span.inherits
    ref: base.span
  - id: span.clears
    ref: base.span
    entity_associations: []
event_refinements:
  - id: event.inherits
    ref: base.event
  - id: event.clears
    ref: base.event
    entity_associations: []
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let schema = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V2)
        .expect("resolution should succeed");

    let m_inherits = schema
        .refinements
        .metrics
        .iter()
        .find(|r| &*r.id == "inherits")
        .expect("metric.inherits");
    let m_clears = schema
        .refinements
        .metrics
        .iter()
        .find(|r| &*r.id == "clears")
        .expect("metric.clears");
    assert_eq!(m_inherits.metric.entity_associations.len(), 1);
    assert!(m_clears.metric.entity_associations.is_empty());

    let s_inherits = schema
        .refinements
        .spans
        .iter()
        .find(|r| &*r.id == "inherits")
        .expect("span.inherits");
    let s_clears = schema
        .refinements
        .spans
        .iter()
        .find(|r| &*r.id == "clears")
        .expect("span.clears");
    assert_eq!(s_inherits.span.entity_associations.len(), 1);
    assert!(s_clears.span.entity_associations.is_empty());

    let e_inherits = schema
        .refinements
        .events
        .iter()
        .find(|r| &*r.id == "inherits")
        .expect("event.inherits");
    let e_clears = schema
        .refinements
        .events
        .iter()
        .find(|r| &*r.id == "clears")
        .expect("event.clears");
    assert_eq!(e_inherits.event.entity_associations.len(), 1);
    assert!(e_clears.event.entity_associations.is_empty());
}

#[test]
fn test_v2_provenance_on_unresolved_attr_ref_and_group_cycle() {
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let reg_dir = dir.path().join("registry");
        std::fs::create_dir_all(&reg_dir).expect("create_dir");

        let yaml = r#"file_format: definition/2
spans:
  - type: test.span
    kind: server
    brief: Test span
    stability: stable
    name:
      note: test span
    attributes:
      - ref: nonexistent.attr
"#;
        std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

        let res = resolve_fixture_raw(dir.path().to_str().unwrap(), ResolutionTarget::V2);
        match res {
            WResult::FatalErr(Error::UnresolvedAttributeRef {
                attribute_ref,
                provenance,
                ..
            }) => {
                assert_eq!(attribute_ref, "nonexistent.attr");
                assert!(
                    provenance.is_some(),
                    "provenance must be present on UnresolvedAttributeRef"
                );
            }
            WResult::FatalErr(other) => {
                panic!("Expected FatalErr(UnresolvedAttributeRef), got FatalErr({other})")
            }
            WResult::Ok(_) => panic!("Expected FatalErr(UnresolvedAttributeRef), got Ok"),
            WResult::OkWithNFEs(_, nfes) => {
                panic!("Expected FatalErr(UnresolvedAttributeRef), got OkWithNFEs({nfes:?})")
            }
        }
    }

    {
        let dir = tempfile::tempdir().expect("tempdir");
        let reg_dir = dir.path().join("registry");
        std::fs::create_dir_all(&reg_dir).expect("create_dir");

        let yaml = r#"file_format: definition/2
attribute_groups:
  - id: group.alpha
    visibility: public
    brief: Group Alpha
    stability: stable
    attributes:
      - ref_group: group.beta
  - id: group.beta
    visibility: public
    brief: Group Beta
    stability: stable
    attributes:
      - ref_group: group.alpha
"#;
        std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

        let res = resolve_fixture_raw(dir.path().to_str().unwrap(), ResolutionTarget::V2);
        match res {
            WResult::FatalErr(Error::AttributeGroupCycle { cycle, provenance }) => {
                assert!(!cycle.is_empty(), "cycle path must not be empty");
                assert!(
                    provenance.is_some(),
                    "provenance must be present on AttributeGroupCycle"
                );
            }
            WResult::FatalErr(other) => {
                panic!("Expected FatalErr(AttributeGroupCycle), got FatalErr({other})")
            }
            WResult::Ok(_) => panic!("Expected FatalErr(AttributeGroupCycle), got Ok"),
            WResult::OkWithNFEs(_, nfes) => {
                panic!("Expected FatalErr(AttributeGroupCycle), got OkWithNFEs({nfes:?})")
            }
        }
    }
}

#[test]
fn test_parity_v1_dependency_mixed_inline_and_ref_import() {
    let dir = tempfile::tempdir().expect("tempdir");
    let v1_dir = dir.path().join("v1_dep");
    let main_dir = dir.path().join("main").join("registry");

    std::fs::create_dir_all(&v1_dir).expect("create v1_dep");
    std::fs::create_dir_all(&main_dir).expect("create main");

    // V1 dependency registry
    std::fs::write(
        v1_dir.join("registry_manifest.yaml"),
        "schema_url: https://example.com/v1_dep/1.0.0\n",
    )
    .expect("write v1 manifest");
    std::fs::write(
        v1_dir.join("semconv.yaml"),
        r#"groups:
  - id: v1.catalog.group
    type: attribute_group
    brief: Catalog group
    stability: stable
    attributes:
      - id: catalog.attr
        type: string
        brief: Catalog attribute
        stability: stable
        examples: ["cat"]
  - id: v1.metric
    type: metric
    metric_name: v1.metric
    instrument: counter
    unit: "1"
    brief: V1 metric
    stability: stable
    attributes:
      - id: inline.attr
        type: int
        brief: Inline attribute
        stability: stable
      - ref: catalog.attr
"#,
    )
    .expect("write v1 semconv");

    // V2 main registry
    std::fs::write(
        main_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/main/1.0.0\ndependencies:\n  - schema_url: https://example.com/v1_dep/1.0.0\n    registry_path: {}\n",
            v1_dir.display()
        ),
    )
    .expect("write main manifest");
    std::fs::write(
        main_dir.join("schema.yaml"),
        r#"file_format: definition/2
imports:
  metrics:
    - v1.metric
"#,
    )
    .expect("write main schema");

    let legacy = resolve_fixture(
        main_dir.parent().unwrap().to_str().unwrap(),
        ResolutionTarget::V1,
    )
    .expect("V1 resolution succeeded");
    let native = resolve_fixture(
        main_dir.parent().unwrap().to_str().unwrap(),
        ResolutionTarget::V2,
    )
    .expect("V2 resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);
    assert_eq!(native.registry.metrics.len(), 1);
    let m = &native.registry.metrics[0];
    assert_eq!(&*m.name, "v1.metric");
    assert_eq!(
        m.attributes.len(),
        2,
        "Metric should preserve both inline and ref attributes from V1 dependency"
    );

    let keys: Vec<_> = m
        .attributes
        .iter()
        .map(|ar| native.attribute_catalog[ar.base.0 as usize].key.as_str())
        .collect();
    assert!(keys.contains(&"inline.attr"));
    assert!(keys.contains(&"catalog.attr"));
}

#[test]
fn test_transitive_chain_cache_capacity_one_parity_and_schema_cache() {
    use std::num::NonZeroUsize;
    use weaver_resolver::SchemaResolver;

    let dir = tempfile::tempdir().expect("tempdir");
    let base_dir = dir.path().join("base");
    let middle_dir = dir.path().join("middle");
    let root_dir = dir.path().join("root");

    std::fs::create_dir_all(&base_dir).expect("create base");
    std::fs::create_dir_all(&middle_dir).expect("create middle");
    std::fs::create_dir_all(&root_dir).expect("create root");

    let base_url = SchemaUrl::try_from("https://example.com/base/1.0.0").unwrap();
    let middle_url = SchemaUrl::try_from("https://example.com/middle/1.0.0").unwrap();
    let root_url = SchemaUrl::try_from("https://example.com/root/1.0.0").unwrap();

    // Base registry
    std::fs::write(
        base_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/base/1.0.0\n",
    )
    .expect("write base manifest");
    std::fs::write(
        base_dir.join("schema.yaml"),
        r#"file_format: definition/2
attributes:
  - key: base.attr
    type: string
    brief: Base attr
    stability: stable
  - key: base.id
    type: string
    brief: Base id
    stability: stable
entities:
  - type: base.entity
    brief: Base entity
    stability: stable
    identity:
      - ref: base.id
metrics:
  - name: base.metric
    instrument: counter
    unit: "1"
    brief: Base metric
    stability: stable
    attributes:
      - ref: base.attr
"#,
    )
    .expect("write base schema");

    // Middle registry depends on base, re-exports base.attr with an override in a public group
    std::fs::write(
        middle_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/middle/1.0.0\ndependencies:\n  - schema_url: https://example.com/base/1.0.0\n    registry_path: {}\n",
            base_dir.display()
        ),
    )
    .expect("write middle manifest");
    std::fs::write(
        middle_dir.join("schema.yaml"),
        r#"file_format: definition/2
imports:
  metrics:
    - base.metric
  entities:
    - base.entity
attribute_groups:
  - id: middle.group
    visibility: public
    brief: Middle group
    stability: stable
    attributes:
      - ref: base.attr
        brief: Middle overridden brief
spans:
  - type: middle.span
    kind: internal
    brief: Middle span
    stability: stable
    name:
      note: middle span
    entity_associations:
      - base.entity
    attributes:
      - ref: base.attr
"#,
    )
    .expect("write middle schema");

    // Root registry depends on middle, imports middle's group/span and directly references base.attr
    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/root/1.0.0\ndependencies:\n  - schema_url: https://example.com/middle/1.0.0\n    registry_path: {}\n",
            middle_dir.display()
        ),
    )
    .expect("write root manifest");
    std::fs::write(
        root_dir.join("schema.yaml"),
        r#"file_format: definition/2
imports:
  attribute_groups:
    - middle.group
  spans:
    - middle.span
metrics:
  - name: root.metric
    instrument: counter
    unit: "1"
    brief: Root metric
    stability: stable
    attributes:
      - ref: base.attr
"#,
    )
    .expect("write root schema");

    let resolve_with = |target: ResolutionTarget, capacity: usize| -> V2Schema {
        let mut repo_nfes = Vec::new();
        let repo = RegistryRepo::try_new(
            Some(root_url.clone()),
            &VirtualDirectoryPath::LocalFolder {
                path: root_dir.display().to_string(),
            },
            &mut repo_nfes,
        )
        .expect("repo");

        let mut resolver = WeaverResolver::new(
            WeaverResolverConfig::default()
                .with_cache_capacity(NonZeroUsize::new(capacity).unwrap())
                .with_target(target),
        );
        let bundle = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        };

        let mut cache = match resolver.schema_cache_for(&bundle) {
            WResult::Ok(c) | WResult::OkWithNFEs(c, _) => c,
            WResult::FatalErr(e) => panic!("schema_cache_for failed: {e}"),
        };
        assert!(matches!(
            cache.resolve_schema(&base_url),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));
        assert!(matches!(
            cache.resolve_schema(&middle_url),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));

        match bundle {
            WeaverResolvedSchema::V2(v2) => v2,
            WeaverResolvedSchema::V1(v1) => V2Schema::try_from(v1).expect("V1 to V2"),
        }
    };

    let v2_cap1 = resolve_with(ResolutionTarget::V2, 1);
    let v2_cap32 = resolve_with(ResolutionTarget::V2, 32);
    let v1_cap1 = resolve_with(ResolutionTarget::V1, 1);
    let v1_cap32 = resolve_with(ResolutionTarget::V1, 32);

    assert_v2_schema_eq(&v2_cap1, &v2_cap32);
    assert_v2_schema_eq(&v1_cap1, &v1_cap32);
    assert_v2_schema_eq(&v2_cap1, &v1_cap1);

    let root_metric = v2_cap1
        .registry
        .metrics
        .iter()
        .find(|m| &*m.name == "root.metric")
        .expect("root.metric present");
    let attr = &v2_cap1.attribute_catalog[root_metric.attributes[0].base.0 as usize];
    assert_eq!(attr.common.brief, "Base attr");
}

#[test]
fn test_into_v2_converts_v1_and_passes_through_v2() {
    let mut repo_nfes = vec![];
    let v2_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-1-everything/registry".to_owned(),
        },
        &mut repo_nfes,
    )
    .expect("v2 repo");
    let mut resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let v2_bundle = match resolver.load_and_resolve_schema(v2_repo, DefaultSchemaVisitor) {
        WResult::Ok(b) | WResult::OkWithNFEs(b, _) => b,
        WResult::FatalErr(e) => panic!("V2 resolution failed: {e}"),
    };
    assert!(v2_bundle.as_v2().is_some());
    let v2_schema = v2_bundle.into_v2().expect("into_v2 on V2 bundle");
    assert!(!v2_schema.registry.metrics.is_empty());

    let v1_for_v2_target = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-7-spans/registry".to_owned(),
        },
        &mut vec![],
    )
    .expect("v1 repo");
    let mut v2_on_v1_resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    let v2_on_v1_bundle =
        match v2_on_v1_resolver.load_and_resolve_schema(v1_for_v2_target, DefaultSchemaVisitor) {
            WResult::Ok(b) | WResult::OkWithNFEs(b, _) => b,
            WResult::FatalErr(e) => panic!("V2 target on V1 registry failed: {e}"),
        };
    assert!(
        v2_on_v1_bundle.as_v2().is_some(),
        "ResolutionTarget::V2 should always return WeaverResolvedSchema::V2"
    );

    let v1_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-7-spans/registry".to_owned(),
        },
        &mut vec![],
    )
    .expect("v1 repo");
    let mut v1_resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V1));
    let v1_bundle = match v1_resolver.load_and_resolve_schema(v1_repo, DefaultSchemaVisitor) {
        WResult::Ok(b) | WResult::OkWithNFEs(b, _) => b,
        WResult::FatalErr(e) => panic!("V1 resolution failed: {e}"),
    };
    assert!(v1_bundle.as_v1().is_some());
    let converted_v2 = v1_bundle.into_v2().expect("into_v2 on V1 bundle");
    assert!(!converted_v2.registry.spans.is_empty());
}

#[test]
fn test_v2_dependency_nfe_forwarding_and_authoring_lint_scoping() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_v1_dir = dir.path().join("base_v1");
    let base_v2_dir = dir.path().join("base_v2_override");
    let mid_dir = dir.path().join("mid");
    let root_dir = dir.path().join("root");
    for d in [&base_v1_dir, &base_v2_dir, &mid_dir, &root_dir] {
        std::fs::create_dir_all(d).expect("mkdir");
    }

    std::fs::write(
        base_v1_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nname: base\nschema_url: https://example.com/base/1.0.0\n",
    )
    .expect("write");
    std::fs::write(
        base_v1_dir.join("defs.yaml"),
        "file_format: definition/2\nattributes:\n  - key: base.attr\n    type: string\n    brief: Base\n    stability: stable\n",
    )
    .expect("write");

    // Superseded base 1.1.0 omits `file_format` (load NFE) and declares an unmatched import (authoring lint).
    std::fs::write(
        base_v2_dir.join("manifest.yaml"),
        "name: base\nschema_url: https://example.com/base/1.1.0\n",
    )
    .expect("write");
    std::fs::write(
        base_v2_dir.join("defs.yaml"),
        "file_format: definition/2\nimports:\n  metrics:\n    - nonexistent.*\nattributes:\n  - key: base.attr\n    type: string\n    brief: Base upgraded\n    stability: stable\n",
    )
    .expect("write");

    std::fs::write(
        mid_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nname: mid\nschema_url: https://example.com/mid/1.0.0\ndependencies:\n  - name: base\n    schema_url: https://example.com/base/1.1.0\n",
    )
    .expect("write");
    std::fs::write(
        mid_dir.join("defs.yaml"),
        "file_format: definition/2\nimports:\n  metrics:\n    - upstream_unused.*\nspans:\n  - type: mid.span\n    kind: internal\n    name:\n      note: Mid\n    brief: Mid\n    stability: stable\n    requirement_level: recommended\n    attributes:\n      - ref: base.attr\n",
    )
    .expect("write");

    std::fs::write(
        root_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nname: root\nschema_url: https://example.com/root/1.0.0\ndependencies:\n  - name: base\n    schema_url: https://example.com/base/1.0.0\n    registry_path: {}\n  - name: mid\n    schema_url: https://example.com/mid/1.0.0\n    registry_path: {}\n",
            base_v1_dir.display(),
            mid_dir.display()
        ),
    )
    .expect("write");
    std::fs::write(
        root_dir.join("defs.yaml"),
        "file_format: definition/2\nspans:\n  - type: root.span\n    kind: client\n    name:\n      note: Root\n    brief: Root\n    stability: stable\n    requirement_level: recommended\n    attributes:\n      - ref: base.attr\n",
    )
    .expect("write");

    let mut repo_nfes = vec![];
    let root_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: root_dir.display().to_string(),
        },
        &mut repo_nfes,
    )
    .expect("root repo");

    let mut resolver =
        WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
    resolver.add_schema_url_override(
        SchemaUrl::try_from("https://example.com/base/1.1.0").unwrap(),
        VirtualDirectoryPath::LocalFolder {
            path: base_v2_dir.display().to_string(),
        },
    );
    match resolver.load_and_resolve_schema(root_repo, DefaultSchemaVisitor) {
        WResult::OkWithNFEs(_, nfes) => {
            assert!(
                nfes.iter().any(|e| matches!(
                    e,
                    Error::FailToResolveDefinition(
                        weaver_semconv::Error::MissingManifestFileFormat { .. }
                    )
                )),
                "Expected override FailToResolveDefinition NFE to be preserved, got: {nfes:?}"
            );
            assert!(
                nfes.iter()
                    .any(|e| matches!(e, Error::DependencyVersionUpgradedWarning { .. })),
                "Expected DependencyVersionUpgradedWarning, got: {nfes:?}"
            );
            assert!(
                !nfes
                    .iter()
                    .any(|e| matches!(e, Error::UnmatchedImport { .. })),
                "Upstream dependency UnmatchedImport should not leak to consumer: {nfes:?}"
            );
        }
        WResult::Ok(_) => panic!("Expected OkWithNFEs with override & upgrade warnings"),
        WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
    }
}

#[test]
fn test_v2_diagnostic_provenance_formatting() {
    let no_prov = Error::UnresolvedRefGroup {
        used_in: "span.demo".to_owned(),
        ref_group: "missing.group".to_owned(),
        provenance: None,
    };
    let rendered_none = no_prov.to_string();
    assert!(
        !rendered_none.contains("Provenance:"),
        "Should omit Provenance when None, got: {rendered_none}"
    );

    let prov = Box::new(weaver_semconv::provenance::Provenance::new(
        SchemaUrl::try_from("https://example.com/schemas/1.0.0").unwrap(),
        "spans.yaml",
    ));
    let cycle_err = Error::AttributeGroupCycle {
        cycle: "a -> b -> a".to_owned(),
        provenance: Some(prov.clone()),
    };
    assert!(
        cycle_err
            .to_string()
            .contains("Provenance: https://example.com/schemas/1.0.0:spans.yaml"),
        "Unexpected AttributeGroupCycle display: {cycle_err}"
    );

    let excl_err = Error::ExcludedFromDependencyResolution {
        id: "secret.attr".to_owned(),
        r#type: "Attribute".to_owned(),
        used_in: "span.public".to_owned(),
        provenance: Some(prov),
    };
    assert!(
        excl_err
            .to_string()
            .contains("Provenance: https://example.com/schemas/1.0.0:spans.yaml"),
        "Unexpected ExcludedFromDependencyResolution display: {excl_err}"
    );
}

#[test]
fn test_inherited_ref_negative_fixtures_v1_and_v2() {
    for (fixture, expected_attr) in [
        ("data/inherited-ref-test/user", "base.attr"),
        ("data/inherited-ref-test/refiner_user", "base.attr"),
    ] {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let mut repo_nfes = vec![];
            let repo = RegistryRepo::try_new(
                None,
                &VirtualDirectoryPath::LocalFolder {
                    path: fixture.to_owned(),
                },
                &mut repo_nfes,
            )
            .expect("repo");
            let mut resolver =
                WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
            match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
                WResult::FatalErr(Error::UnresolvedAttributeRef { attribute_ref, .. }) => {
                    assert_eq!(attribute_ref, expected_attr);
                }
                WResult::FatalErr(Error::CompoundError(errs)) => {
                    assert!(
                        errs.iter().any(|e| matches!(
                            e,
                            Error::UnresolvedAttributeRef { attribute_ref, .. }
                                if attribute_ref == expected_attr
                        )),
                        "Expected UnresolvedAttributeRef({expected_attr}) for {fixture} under {target:?}, got: {errs:?}"
                    );
                }
                WResult::FatalErr(other) => panic!(
                    "Expected UnresolvedAttributeRef({expected_attr}) for {fixture} under {target:?}, got FatalErr({other:?})"
                ),
                WResult::Ok(_) | WResult::OkWithNFEs(_, _) => panic!(
                    "Expected UnresolvedAttributeRef({expected_attr}) for {fixture} under {target:?}, got Ok"
                ),
            }
        }
    }
}

#[test]
fn test_parity_attribute_group_named_like_signal_does_not_collide() {
    let dir = tempfile::tempdir().expect("tempdir");
    let reg_dir = dir.path().join("registry");
    std::fs::create_dir_all(&reg_dir).expect("create_dir");

    let yaml = r#"file_format: definition/2
attributes:
  - {key: attr.group_metric, type: string, brief: gm, stability: stable}
  - {key: attr.metric_own, type: string, brief: mo, stability: stable}
  - {key: attr.group_span, type: string, brief: gs, stability: stable}
  - {key: attr.span_own, type: string, brief: so, stability: stable}
  - {key: attr.group_event, type: string, brief: ge, stability: stable}
  - {key: attr.event_own, type: string, brief: eo, stability: stable}
attribute_groups:
  - id: metric.foo
    visibility: public
    brief: Group named like metric
    stability: stable
    attributes:
      - ref: attr.group_metric
  - id: span.bar
    visibility: public
    brief: Group named like span
    stability: stable
    attributes:
      - ref: attr.group_span
  - id: event.baz
    visibility: public
    brief: Group named like event
    stability: stable
    attributes:
      - ref: attr.group_event
metrics:
  - name: foo
    instrument: counter
    unit: "1"
    brief: Metric foo
    stability: stable
    requirement_level: recommended
    attributes:
      - ref: attr.metric_own
spans:
  - type: bar
    kind: internal
    brief: Span bar
    stability: stable
    requirement_level: recommended
    name:
      note: bar span
    attributes:
      - ref: attr.span_own
events:
  - name: baz
    brief: Event baz
    stability: stable
    requirement_level: recommended
    attributes:
      - ref: attr.event_own
"#;
    std::fs::write(reg_dir.join("schema.yaml"), yaml).expect("write schema");

    let legacy = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V1)
        .expect("V1 resolution succeeded");
    let native = resolve_fixture(dir.path().to_str().unwrap(), ResolutionTarget::V2)
        .expect("V2 resolution succeeded");

    assert_v2_schema_eq(&native, &legacy);

    let metric_attr_idx = native.registry.metrics[0].attributes[0].base.0 as usize;
    assert_eq!(
        native.attribute_catalog[metric_attr_idx].key,
        "attr.metric_own"
    );
    let span_attr_idx = native.registry.spans[0].attributes[0].base.0 as usize;
    assert_eq!(native.attribute_catalog[span_attr_idx].key, "attr.span_own");
    let event_attr_idx = native.registry.events[0].attributes[0].base.0 as usize;
    assert_eq!(
        native.attribute_catalog[event_attr_idx].key,
        "attr.event_own"
    );
}

#[test]
fn test_v2_ref_group_resolves_public_group_from_packaged_and_source_dependencies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let producer_dir = dir.path().join("producer");
    let published_dir = dir.path().join("published");
    let consumer_dir = dir.path().join("consumer");
    let consumer_no_import_dir = dir.path().join("consumer_no_import");
    let consumer_private_dir = dir.path().join("consumer_private");
    for d in [
        &producer_dir,
        &published_dir,
        &consumer_dir,
        &consumer_no_import_dir,
        &consumer_private_dir,
    ] {
        std::fs::create_dir_all(d).expect("mkdir");
    }

    // Producer registry with a public attribute group, an internal one, and an excluded public one.
    std::fs::write(
        producer_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/producer/1.0.0\n",
    )
    .expect("write producer manifest");
    std::fs::write(
        producer_dir.join("registry.yaml"),
        r#"file_format: definition/2
attributes:
  - key: example.name
    type: string
    brief: Example name.
    stability: development
  - key: example.extra
    type: int
    brief: Example extra.
    stability: development
attribute_groups:
  - id: example.common
    visibility: public
    brief: Common attributes.
    stability: development
    attributes:
      - ref: example.name
        requirement_level: required
  - id: example.internal
    visibility: internal
    attributes:
      - ref: example.extra
  - id: example.excluded
    visibility: public
    brief: Excluded attributes.
    stability: development
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: example.extra
spans:
  - type: base.span
    kind: internal
    name:
      note: Base span.
    brief: Base span.
    stability: development
    requirement_level: recommended
"#,
    )
    .expect("write producer registry");

    // Resolve producer and write a packaged publication artifact (`manifest/2.0` + `resolved.yaml`).
    let producer_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: producer_dir.display().to_string(),
        },
        &mut vec![],
    )
    .expect("producer repo");
    let producer_config = WeaverResolverConfig::default().with_target(ResolutionTarget::V2);
    let producer_resolved = match WeaverResolver::new(producer_config)
        .load_and_resolve_schema(producer_repo, DefaultSchemaVisitor)
    {
        WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s.into_v2().expect("v2 schema"),
        WResult::FatalErr(e) => panic!("producer resolve failed: {e:?}"),
    };
    std::fs::write(
        published_dir.join("manifest.yaml"),
        r#"file_format: manifest/2.0
schema_url: https://example.com/producer/1.0.0
description: Published producer registry.
stability: development
resolved_registry_uri: resolved.yaml
"#,
    )
    .expect("write published manifest");
    std::fs::write(
        published_dir.join("resolved.yaml"),
        serde_saphyr::to_string(&producer_resolved).expect("serialize resolved.yaml"),
    )
    .expect("write published resolved.yaml");

    let published_path = published_dir.to_string_lossy().replace('\\', "/");

    // Consumer 1: exact issue #1843 repro (imports + ref_group on span, plus inline override & refinement).
    std::fs::write(
        consumer_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/consumer/1.0.0\ndependencies:\n  - schema_url: https://example.com/producer/1.0.0\n    registry_path: {published_path}\n"
        ),
    )
    .expect("write consumer manifest");
    std::fs::write(
        consumer_dir.join("registry.yaml"),
        r#"file_format: definition/2
imports:
  attribute_groups: [example.common]
spans:
  - type: example.operation
    kind: internal
    name:
      note: Use example.operation.
    brief: Example operation.
    stability: development
    requirement_level: recommended
    attributes:
      - ref_group: example.common
      - ref: example.name
        brief: Overridden brief in consumer.
span_refinements:
  - id: refined.operation
    ref: base.span
    brief: Refined base span using dependency ref_group.
    stability: development
    attributes:
      - ref_group: example.common
"#,
    )
    .expect("write consumer registry");

    let consumer_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: consumer_dir.display().to_string(),
        },
        &mut vec![],
    )
    .expect("consumer repo");
    let config = WeaverResolverConfig::default().with_target(ResolutionTarget::V2);
    let consumer_v2 = match WeaverResolver::new(config)
        .load_and_resolve_schema(consumer_repo, DefaultSchemaVisitor)
    {
        WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s.into_v2().expect("v2 schema"),
        WResult::FatalErr(e) => panic!("consumer resolve failed: {e:?}"),
    };

    assert_eq!(consumer_v2.registry.attribute_groups.len(), 1);
    assert_eq!(
        &*consumer_v2.registry.attribute_groups[0].id,
        "example.common"
    );
    assert_eq!(consumer_v2.registry.spans.len(), 1);
    let span = &consumer_v2.registry.spans[0];
    assert_eq!(span.attributes.len(), 1);
    let span_attr = &consumer_v2.attribute_catalog[span.attributes[0].base.0 as usize];
    assert_eq!(span_attr.key, "example.name");
    assert_eq!(span_attr.common.brief, "Overridden brief in consumer.");
    assert!(
        span_attr.provenance.source.is_some(),
        "attribute from dependency ref_group must preserve dependency provenance"
    );
    assert_eq!(
        span.attributes[0].requirement_level,
        weaver_semconv::v2::attribute::RequirementLevel::Basic(
            weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required
        )
    );

    // Refinement also resolved `ref_group: example.common` from the packaged dependency.
    let refined_span = consumer_v2
        .refinements
        .spans
        .iter()
        .find(|r| &*r.id == "refined.operation")
        .expect("refined.operation refinement present");
    assert_eq!(refined_span.span.attributes.len(), 1);
    let ref_attr = &consumer_v2.attribute_catalog[refined_span.span.attributes[0].base.0 as usize];
    assert_eq!(ref_attr.key, "example.name");
    assert!(ref_attr.provenance.source.is_some());

    // Consumer 2: `ref_group` without `imports:` still works and does not re-export the group.
    std::fs::write(
        consumer_no_import_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/consumer2/1.0.0\ndependencies:\n  - schema_url: https://example.com/producer/1.0.0\n    registry_path: {published_path}\n"
        ),
    )
    .expect("write consumer2 manifest");
    std::fs::write(
        consumer_no_import_dir.join("registry.yaml"),
        r#"file_format: definition/2
spans:
  - type: example.operation
    kind: internal
    name:
      note: Use example.operation.
    brief: Example operation.
    stability: development
    requirement_level: recommended
    attributes:
      - ref_group: example.common
"#,
    )
    .expect("write consumer2 registry");

    let consumer2_repo = RegistryRepo::try_new(
        None,
        &VirtualDirectoryPath::LocalFolder {
            path: consumer_no_import_dir.display().to_string(),
        },
        &mut vec![],
    )
    .expect("consumer2 repo");
    let config2 = WeaverResolverConfig::default().with_target(ResolutionTarget::V2);
    let consumer2_v2 = match WeaverResolver::new(config2)
        .load_and_resolve_schema(consumer2_repo, DefaultSchemaVisitor)
    {
        WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s.into_v2().expect("v2 schema"),
        WResult::FatalErr(e) => panic!("consumer2 resolve failed: {e:?}"),
    };
    assert!(
        consumer2_v2.registry.attribute_groups.is_empty(),
        "ref_group alone should not re-export the dependency group"
    );
    assert_eq!(consumer2_v2.registry.spans[0].attributes.len(), 1);

    // Consumer 3: internal or excluded dependency attribute groups remain invisible to `ref_group`.
    let producer_path = producer_dir.to_string_lossy().replace('\\', "/");
    std::fs::write(
        consumer_private_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/consumer3/1.0.0\ndependencies:\n  - schema_url: https://example.com/producer/1.0.0\n    registry_path: {producer_path}\n"
        ),
    )
    .expect("write consumer3 manifest");
    for private_id in ["example.internal", "example.excluded"] {
        std::fs::write(
            consumer_private_dir.join("registry.yaml"),
            format!(
                r#"file_format: definition/2
spans:
  - type: example.operation
    kind: internal
    name:
      note: Use example.operation.
    brief: Example operation.
    stability: development
    requirement_level: recommended
    attributes:
      - ref_group: {private_id}
"#
            ),
        )
        .expect("write consumer3 registry");

        let repo3 = RegistryRepo::try_new(
            None,
            &VirtualDirectoryPath::LocalFolder {
                path: consumer_private_dir.display().to_string(),
            },
            &mut vec![],
        )
        .expect("consumer3 repo");
        let config3 = WeaverResolverConfig::default().with_target(ResolutionTarget::V2);
        match WeaverResolver::new(config3).load_and_resolve_schema(repo3, DefaultSchemaVisitor) {
            WResult::FatalErr(Error::UnresolvedRefGroup { ref ref_group, .. })
                if ref_group == private_id => {}
            WResult::FatalErr(e) => {
                panic!("expected UnresolvedRefGroup for {private_id}, got FatalErr({e:?})")
            }
            _ => panic!("expected UnresolvedRefGroup for {private_id}, got Ok"),
        }
    }
}
