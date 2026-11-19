// SPDX-License-Identifier: Apache-2.0

//! Unit tests for refinement entity association inheritance and dependency provenance.

use std::sync::Arc;
use weaver_resolved_schema::v2::entity::{EntityAssociation, EntityRef};
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;

use crate::graph::loader::LoadedSemconvRegistry;
use crate::{ResolutionTarget, WeaverResolvedSchema, WeaverResolver, WeaverResolverConfig};

fn resolve_v2(loaded: LoadedSemconvRegistry, target: ResolutionTarget) -> V2Schema {
    let resolved = WeaverResolver::new(WeaverResolverConfig {
        target,
        ..Default::default()
    })
    .resolve_loaded(loaded)
    .into_result_failing_non_fatal()
    .expect("resolve loaded");
    match Arc::unwrap_or_clone(resolved) {
        WeaverResolvedSchema::V1(v1) => v1.try_into().expect("try_into v2"),
        WeaverResolvedSchema::V2(v2) => v2,
    }
}

fn create_registry_with_url(url: &SchemaUrl, spec_str: &str) -> LoadedSemconvRegistry {
    let mut loaded =
        LoadedSemconvRegistry::create_from_string(spec_str).expect("create loaded from string");
    let LoadedSemconvRegistry::Unresolved { repo, specs, .. } = &mut loaded else {
        unreachable!()
    };
    *repo = RegistryRepo::try_new(Some(url.clone()), &"data".try_into().unwrap(), &mut vec![])
        .expect("create repo");
    for spec in specs {
        spec.provenance.schema_url = url.clone();
    }
    loaded
}

#[test]
fn test_local_refinements_inherit_parent_entity_associations() {
    let yaml = r#"
file_format: definition/2
entities:
  - type: example.entity
    brief: Example entity.
    stability: development
    requirement_level: recommended
    identity:
      - ref: example.id
attributes:
  - key: example.id
    type: int
    brief: Entity ID.
    stability: development
spans:
  - type: example.span
    brief: Example span.
    stability: development
    requirement_level: recommended
    kind: internal
    name:
      note: Span name.
    entity_associations:
      - example.entity
metrics:
  - name: example.metric
    brief: Example metric.
    stability: development
    requirement_level: recommended
    instrument: counter
    unit: '1'
    entity_associations:
      - example.entity
events:
  - name: example.event
    brief: Example event.
    stability: development
    requirement_level: recommended
    entity_associations:
      - example.entity
span_refinements:
  - id: example.span.refined
    ref: example.span
metric_refinements:
  - id: example.metric.refined
    ref: example.metric
event_refinements:
  - id: example.event.refined
    ref: example.event
"#;
    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let loaded = LoadedSemconvRegistry::create_from_string(yaml).unwrap();
        let v2 = resolve_v2(loaded, target);

        let span_ref = v2
            .refinements
            .spans
            .iter()
            .find(|s| &*s.id == "example.span.refined")
            .expect("span refinement");
        assert_eq!(span_ref.span.entity_associations.len(), 1);
        match &span_ref.span.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                assert_eq!(&**r#type, "example.entity");
                assert!(
                    provenance.source.is_none(),
                    "Local entity association must not have dependency source"
                );
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }

        let metric_ref = v2
            .refinements
            .metrics
            .iter()
            .find(|m| &*m.id == "example.metric.refined")
            .expect("metric refinement");
        assert_eq!(metric_ref.metric.entity_associations.len(), 1);
        match &metric_ref.metric.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                assert_eq!(&**r#type, "example.entity");
                assert!(provenance.source.is_none());
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }

        let event_ref = v2
            .refinements
            .events
            .iter()
            .find(|e| &*e.id == "example.event.refined")
            .expect("event refinement");
        assert_eq!(event_ref.event.entity_associations.len(), 1);
        match &event_ref.event.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                assert_eq!(&**r#type, "example.entity");
                assert!(provenance.source.is_none());
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }
    }
}

#[test]
fn test_refinements_with_associations_replace_parent() {
    let yaml = r#"
file_format: definition/2
entities:
  - type: example.entity_a
    brief: Entity A.
    stability: development
    requirement_level: recommended
    identity:
      - ref: example.id
  - type: example.entity_b
    brief: Entity B.
    stability: development
    requirement_level: recommended
    identity:
      - ref: example.id
attributes:
  - key: example.id
    type: int
    brief: Entity ID.
    stability: development
spans:
  - type: example.span
    brief: Example span.
    stability: development
    requirement_level: recommended
    kind: internal
    name:
      note: Span name.
    entity_associations:
      - example.entity_a
metrics:
  - name: example.metric
    brief: Example metric.
    stability: development
    requirement_level: recommended
    instrument: counter
    unit: '1'
    entity_associations:
      - example.entity_a
events:
  - name: example.event
    brief: Example event.
    stability: development
    requirement_level: recommended
    entity_associations:
      - example.entity_a
span_refinements:
  - id: example.span.refined
    ref: example.span
    entity_associations:
      - example.entity_b
metric_refinements:
  - id: example.metric.refined
    ref: example.metric
    entity_associations:
      - example.entity_b
event_refinements:
  - id: example.event.refined
    ref: example.event
    entity_associations:
      - example.entity_b
"#;
    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let loaded = LoadedSemconvRegistry::create_from_string(yaml).unwrap();
        let v2 = resolve_v2(loaded, target);

        let span_ref = v2
            .refinements
            .spans
            .iter()
            .find(|s| &*s.id == "example.span.refined")
            .expect("span refinement");
        assert_eq!(span_ref.span.entity_associations.len(), 1);
        match &span_ref.span.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, .. }) => {
                assert_eq!(&**r#type, "example.entity_b");
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }

        let metric_ref = v2
            .refinements
            .metrics
            .iter()
            .find(|m| &*m.id == "example.metric.refined")
            .expect("metric refinement");
        assert_eq!(metric_ref.metric.entity_associations.len(), 1);
        match &metric_ref.metric.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, .. }) => {
                assert_eq!(&**r#type, "example.entity_b");
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }

        let event_ref = v2
            .refinements
            .events
            .iter()
            .find(|e| &*e.id == "example.event.refined")
            .expect("event refinement");
        assert_eq!(event_ref.event.entity_associations.len(), 1);
        match &event_ref.event.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, .. }) => {
                assert_eq!(&**r#type, "example.entity_b");
            }
            other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
        }
    }
}

#[test]
fn test_dependency_refinement_inherited_association_origins() {
    let dep_url = SchemaUrl::try_from("https://example.com/dep/1.0.0").unwrap();
    let dep_yaml = r#"
file_format: definition/2
entities:
  - type: common.entity
    brief: Entity in dependency.
    stability: development
    requirement_level: recommended
    identity:
      - ref: dep.id
attributes:
  - key: dep.id
    type: int
    brief: Dependency ID.
    stability: development
spans:
  - type: dep.span
    brief: Dependency span.
    stability: development
    requirement_level: recommended
    kind: internal
    name:
      note: Dep span name.
    entity_associations:
      - common.entity
metrics:
  - name: dep.metric
    brief: Dependency metric.
    stability: development
    requirement_level: recommended
    instrument: counter
    unit: '1'
    entity_associations:
      - common.entity
events:
  - name: dep.event
    brief: Dependency event.
    stability: development
    requirement_level: recommended
    entity_associations:
      - common.entity
"#;

    let consumer_url = SchemaUrl::try_from("https://example.com/consumer/1.0.0").unwrap();
    let consumer_yaml = r#"
file_format: definition/2
entities:
  - type: common.entity
    brief: Unrelated entity defined locally in consumer.
    stability: development
    requirement_level: recommended
    identity:
      - ref: consumer.id
attributes:
  - key: consumer.id
    type: int
    brief: Consumer ID.
    stability: development
span_refinements:
  - id: consumer.span.refined
    ref: dep.span
metric_refinements:
  - id: consumer.metric.refined
    ref: dep.metric
event_refinements:
  - id: consumer.event.refined
    ref: dep.event
"#;

    let dep_loaded = create_registry_with_url(&dep_url, dep_yaml);
    let resolved_dep_v1 = Arc::unwrap_or_clone(
        WeaverResolver::new(WeaverResolverConfig::default())
            .resolve_loaded(dep_loaded)
            .into_result_failing_non_fatal()
            .expect("resolve dependency"),
    )
    .into_v1()
    .unwrap();

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        for is_v2 in [true, false] {
            let dependency = if is_v2 {
                let dep_v2: V2Schema = resolved_dep_v1.clone().try_into().unwrap();
                LoadedSemconvRegistry::ResolvedV2 {
                    schema: dep_v2,
                    direct_dependencies: vec![],
                }
            } else {
                LoadedSemconvRegistry::Resolved {
                    schema: resolved_dep_v1.clone(),
                    direct_dependencies: vec![],
                }
            };

            let mut consumer_loaded = create_registry_with_url(&consumer_url, consumer_yaml);
            let LoadedSemconvRegistry::Unresolved { dependencies, .. } = &mut consumer_loaded
            else {
                unreachable!()
            };
            dependencies.push(dependency);

            let v2 = resolve_v2(consumer_loaded, target);

            assert_eq!(v2.dependencies.len(), 1);
            assert!(v2.dependencies.contains(&dep_url));

            for (refinement_id, is_span, is_metric) in [
                ("consumer.span.refined", true, false),
                ("consumer.metric.refined", false, true),
                ("consumer.event.refined", false, false),
            ] {
                let assocs = if is_span {
                    v2.refinements
                        .spans
                        .iter()
                        .find(|s| &*s.id == refinement_id)
                        .unwrap()
                        .span
                        .entity_associations
                        .clone()
                } else if is_metric {
                    v2.refinements
                        .metrics
                        .iter()
                        .find(|m| &*m.id == refinement_id)
                        .unwrap()
                        .metric
                        .entity_associations
                        .clone()
                } else {
                    v2.refinements
                        .events
                        .iter()
                        .find(|e| &*e.id == refinement_id)
                        .unwrap()
                        .event
                        .entity_associations
                        .clone()
                };

                assert_eq!(
                    assocs.len(),
                    1,
                    "Failed for {refinement_id} (is_v2={is_v2}, target={target:?})"
                );
                match &assocs[0] {
                    EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                        assert_eq!(&**r#type, "common.entity");
                        assert_eq!(
                            provenance.source,
                            Some(DependencyRef(0)),
                            "Inherited association for {refinement_id} must resolve to dependency entity (DependencyRef(0)), not local entity (is_v2={is_v2}, target={target:?})"
                        );
                    }
                    other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
                }
            }
        }
    }
}

#[test]
fn test_dependency_refinement_chain_carries_origins_through_group_index() {
    let dep_url = SchemaUrl::try_from("https://example.com/dep/1.0.0").unwrap();
    let dep_yaml = r#"
file_format: definition/2
entities:
  - type: common.entity
    brief: Entity in dependency.
    stability: development
    requirement_level: recommended
    identity:
      - ref: dep.id
attributes:
  - key: dep.id
    type: int
    brief: Dependency ID.
    stability: development
spans:
  - type: dep.span
    brief: Dependency span.
    stability: development
    requirement_level: recommended
    kind: internal
    name:
      note: Dep span name.
    entity_associations:
      - common.entity
"#;

    let consumer_url = SchemaUrl::try_from("https://example.com/consumer/1.0.0").unwrap();
    let consumer_yaml = r#"
file_format: definition/2
entities:
  - type: common.entity
    brief: Unrelated entity defined locally in consumer.
    stability: development
    requirement_level: recommended
    identity:
      - ref: consumer.id
attributes:
  - key: consumer.id
    type: int
    brief: Consumer ID.
    stability: development
span_refinements:
  - id: span.consumer.span.step1
    ref: dep.span
  - id: span.consumer.span.step2
    ref: consumer.span.step1
"#;

    let dep_loaded = create_registry_with_url(&dep_url, dep_yaml);
    let resolved_dep_v1 = Arc::unwrap_or_clone(
        WeaverResolver::new(WeaverResolverConfig::default())
            .resolve_loaded(dep_loaded)
            .into_result_failing_non_fatal()
            .expect("resolve dependency"),
    )
    .into_v1()
    .unwrap();

    let dependency = LoadedSemconvRegistry::ResolvedV2 {
        schema: resolved_dep_v1.try_into().unwrap(),
        direct_dependencies: vec![],
    };

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let mut consumer_loaded = create_registry_with_url(&consumer_url, consumer_yaml);
        let LoadedSemconvRegistry::Unresolved { dependencies, .. } = &mut consumer_loaded else {
            unreachable!()
        };
        dependencies.push(dependency.clone());

        let v2 = resolve_v2(consumer_loaded, target);

        for id in ["consumer.span.step1", "consumer.span.step2"] {
            let span_ref = v2
                .refinements
                .spans
                .iter()
                .find(|s| &*s.id == id)
                .unwrap_or_else(|| {
                    panic!(
                        "refinement {id} not found in {:#?} (target={target:?})",
                        v2.refinements.spans
                    )
                });
            assert_eq!(
                span_ref.span.entity_associations.len(),
                1,
                "Failed for {id} (target={target:?})"
            );
            match &span_ref.span.entity_associations[0] {
                EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                    assert_eq!(&**r#type, "common.entity");
                    assert_eq!(
                        provenance.source,
                        Some(DependencyRef(0)),
                        "Chain refinement {id} must retain DependencyRef(0) (target={target:?})"
                    );
                }
                other => panic!("Expected EntityAssociation::Ref, got {other:?}"),
            }
        }
    }
}

#[test]
fn test_v2_refinement_inherits_association_with_dependency_upgrade_and_drop_detection() {
    use crate::error::Error;
    use crate::DefaultSchemaVisitor;
    use weaver_common::vdir::VirtualDirectoryPath;

    let dir = tempfile::tempdir().expect("tempdir");

    let c_1_1_dir = dir.path().join("c_1_1");
    let c_1_2_dir = dir.path().join("c_1_2");
    let c_1_2_dropped_dir = dir.path().join("c_1_2_dropped");
    let b_dir = dir.path().join("b");
    let a_dir = dir.path().join("a");

    std::fs::create_dir_all(&c_1_1_dir).unwrap();
    std::fs::create_dir_all(&c_1_2_dir).unwrap();
    std::fs::create_dir_all(&c_1_2_dropped_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    std::fs::create_dir_all(&a_dir).unwrap();

    let url_c_1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();
    let url_c_1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();
    let url_b = SchemaUrl::try_from("https://example.com/b/1.0.0").unwrap();
    let url_a = SchemaUrl::try_from("https://example.com/a/1.0.0").unwrap();

    std::fs::write(
        c_1_1_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.1.0\n",
    )
    .unwrap();
    std::fs::write(
        c_1_1_dir.join("c.yaml"),
        r#"file_format: definition/2
entities:
  - type: k8s.cluster
    brief: Kubernetes cluster.
    stability: development
    requirement_level: recommended
    identity:
      - ref: k8s.cluster.id
attributes:
  - key: k8s.cluster.id
    type: string
    brief: Cluster ID.
    stability: development
"#,
    )
    .unwrap();

    std::fs::write(
        c_1_2_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.2.0\n",
    )
    .unwrap();
    std::fs::write(
        c_1_2_dir.join("c.yaml"),
        r#"file_format: definition/2
entities:
  - type: k8s.cluster
    brief: Kubernetes cluster (upgraded).
    stability: development
    requirement_level: recommended
    identity:
      - ref: k8s.cluster.id
attributes:
  - key: k8s.cluster.id
    type: string
    brief: Cluster ID (upgraded).
    stability: development
"#,
    )
    .unwrap();

    std::fs::write(
        c_1_2_dropped_dir.join("manifest.yaml"),
        "file_format: definition_manifest/2.0\nschema_url: https://example.com/c/1.2.0\n",
    )
    .unwrap();
    std::fs::write(
        c_1_2_dropped_dir.join("c.yaml"),
        r#"file_format: definition/2
attributes:
  - key: other.attr
    type: string
    brief: Other attribute.
    stability: development
"#,
    )
    .unwrap();

    std::fs::write(
        b_dir.join("manifest.yaml"),
        format!(
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/b/1.0.0\ndependencies:\n  - schema_url: https://example.com/c/1.1.0\n    registry_path: {}\n",
            c_1_1_dir.display()
        ),
    )
    .unwrap();
    std::fs::write(
        b_dir.join("b.yaml"),
        r#"file_format: definition/2
spans:
  - type: b.span
    brief: Span in dependency B.
    stability: development
    requirement_level: recommended
    kind: internal
    name:
      note: Span name.
    entity_associations:
      - k8s.cluster
"#,
    )
    .unwrap();

    let write_manifest_a = |c_dir: &std::path::Path| {
        std::fs::write(
            a_dir.join("manifest.yaml"),
            format!(
                "file_format: definition_manifest/2.0\nschema_url: https://example.com/a/1.0.0\ndependencies:\n  - schema_url: https://example.com/b/1.0.0\n    registry_path: {}\n  - schema_url: https://example.com/c/1.2.0\n    registry_path: {}\n",
                b_dir.display(),
                c_dir.display()
            ),
        )
        .unwrap();
    };
    write_manifest_a(&c_1_2_dir);

    std::fs::write(
        a_dir.join("a.yaml"),
        r#"file_format: definition/2
span_refinements:
  - id: a.span.refined
    ref: b.span
"#,
    )
    .unwrap();

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        let mut config = WeaverResolverConfig {
            target,
            ..Default::default()
        };
        _ = config.schema_url_overrides.insert(
            url_c_1_1.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: c_1_1_dir.to_str().unwrap().to_owned(),
            },
        );
        _ = config.schema_url_overrides.insert(
            url_c_1_2.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: c_1_2_dir.to_str().unwrap().to_owned(),
            },
        );
        _ = config.schema_url_overrides.insert(
            url_b.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: b_dir.to_str().unwrap().to_owned(),
            },
        );

        let mut resolver = WeaverResolver::new(config);
        let repo_a = RegistryRepo::try_new(
            Some(url_a.clone()),
            &VirtualDirectoryPath::LocalFolder {
                path: a_dir.to_str().unwrap().to_owned(),
            },
            &mut vec![],
        )
        .expect("create repo a");

        let resolved = match resolver.load_and_resolve_schema(repo_a, DefaultSchemaVisitor) {
            crate::WResult::Ok(r) | crate::WResult::OkWithNFEs(r, _) => r,
            crate::WResult::FatalErr(e) => {
                panic!("resolution of a should succeed (target={target:?}): {e:?}")
            }
        };

        let v2: V2Schema = match resolved {
            WeaverResolvedSchema::V1(v1) => v1.try_into().expect("into_v2"),
            WeaverResolvedSchema::V2(v2) => v2,
        };

        assert!(v2.dependencies.contains(&url_c_1_2));
        assert!(!v2.dependencies.contains(&url_c_1_1));

        let c_1_2_dep_idx = v2
            .dependencies
            .iter()
            .position(|url| url == &url_c_1_2)
            .expect("c/1.2.0 in dependencies");

        let span_ref = v2
            .refinements
            .spans
            .iter()
            .find(|s| &*s.id == "a.span.refined")
            .expect("refinement a.span.refined");
        assert_eq!(span_ref.span.entity_associations.len(), 1);

        match &span_ref.span.entity_associations[0] {
            EntityAssociation::Ref(EntityRef { r#type, provenance }) => {
                assert_eq!(&r#type[..], "k8s.cluster");
                assert_eq!(
                    provenance.source,
                    Some(DependencyRef(c_1_2_dep_idx as u32)),
                    "origin must point to c/1.2.0 (target={target:?})"
                );
            }
            other => panic!("expected EntityAssociation::Ref, got {other:?}"),
        }
    }

    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        write_manifest_a(&c_1_2_dropped_dir);

        let mut config = WeaverResolverConfig {
            target,
            ..Default::default()
        };
        _ = config.schema_url_overrides.insert(
            url_c_1_1.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: c_1_1_dir.to_str().unwrap().to_owned(),
            },
        );
        _ = config.schema_url_overrides.insert(
            url_c_1_2.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: c_1_2_dropped_dir.to_str().unwrap().to_owned(),
            },
        );
        _ = config.schema_url_overrides.insert(
            url_b.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: b_dir.to_str().unwrap().to_owned(),
            },
        );

        let mut resolver = WeaverResolver::new(config);
        let repo_a = RegistryRepo::try_new(
            Some(url_a.clone()),
            &VirtualDirectoryPath::LocalFolder {
                path: a_dir.to_str().unwrap().to_owned(),
            },
            &mut vec![],
        )
        .expect("create repo a");

        match resolver.load_and_resolve_schema(repo_a, DefaultSchemaVisitor) {
            crate::WResult::FatalErr(Error::EntityNotFoundInUpgradedSchema {
                entity_name,
                original_url,
                upgraded_url,
            }) => {
                assert_eq!(entity_name, "k8s.cluster");
                assert_eq!(original_url, url_c_1_1.to_string());
                assert_eq!(upgraded_url, url_c_1_2.to_string());
            }
            crate::WResult::FatalErr(e) => {
                panic!("expected EntityNotFoundInUpgradedSchema (target={target:?}), got {e:?}")
            }
            _ => panic!("expected FatalErr on target {target:?}, got success"),
        }
    }
}
