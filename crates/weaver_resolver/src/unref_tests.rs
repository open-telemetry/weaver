// SPDX-License-Identifier: Apache-2.0

use super::*;
use weaver_resolved_schema::v1::attribute::Attribute;

const BASE: &str = r#"
file_format: definition/2
attributes:
  - {key: test.keep, type: string, brief: Keep, stability: stable}
  - {key: test.remove, type: string, brief: Remove, stability: stable}
  - {key: test.unused, type: string, brief: Unused, stability: stable}
  - {key: test.id, type: string, brief: Identity, stability: stable}
attribute_groups:
  - id: shared
    visibility: internal
    attributes:
      - ref: test.keep
        brief: Shared brief
        requirement_level: opt_in
      - ref: test.remove
        requirement_level: recommended
  - id: nested
    visibility: internal
    attributes:
      - ref_group: shared
spans:
  - type: span_base
    requirement_level: recommended
    kind: client
    name: {note: Base name}
    brief: Base span
    stability: stable
    attributes:
      - ref_group: nested
      - ref: test.keep
        sampling_relevant: true
        note: Keep this note
metrics:
  - name: metric_base
    requirement_level: recommended
    instrument: counter
    unit: '1'
    brief: Base metric
    stability: stable
    attributes:
      - ref_group: shared
events:
  - name: event_base
    requirement_level: recommended
    brief: Base event
    stability: stable
    attributes:
      - ref_group: shared
entities:
  - type: entity_base
    requirement_level: recommended
    brief: Base entity
    stability: stable
    identity:
      - ref: test.id
    description:
      - ref: test.keep
      - ref: test.remove
        requirement_level: recommended
"#;

fn resolve(yaml: &str) -> Result<ResolvedTelemetrySchema, Error> {
    let loaded = LoadedSemconvRegistry::create_from_string(yaml)?;
    resolve_loaded(loaded)
}

fn resolve_loaded(loaded: LoadedSemconvRegistry) -> Result<ResolvedTelemetrySchema, Error> {
    let resolved = WeaverResolver::new(WeaverResolverConfig::default())
        .resolve_loaded(loaded)
        .into_result_failing_non_fatal()?;
    Arc::unwrap_or_clone(resolved).into_v1()
}

fn refinement(kind: &str, entries: &str) -> String {
    let field = if kind == "entity" {
        "description"
    } else {
        "attributes"
    };
    format!(
        "\n{kind}_refinements:\n  - id: {kind}.refined\n    ref: {kind}_base\n    {field}:\n{entries}\n"
    )
}

fn attrs<'a>(schema: &'a ResolvedTelemetrySchema, id: &str) -> Vec<&'a Attribute> {
    schema
        .group(id)
        .unwrap()
        .attributes
        .iter()
        .map(|r| schema.catalog.attribute(r).unwrap())
        .collect()
}

fn unref_error(error: &Error) -> (&str, &str, &str) {
    match error {
        Error::InvalidAttributeUnref {
            refinement_id,
            attribute_key,
            reason,
            provenance,
        } => {
            assert!(provenance.is_some());
            (refinement_id, attribute_key, reason)
        }
        Error::CompoundError(errors) if errors.len() == 1 => unref_error(&errors[0]),
        other => panic!("Expected one InvalidAttributeUnref, got {other:?}"),
    }
}

#[test]
fn unref_same_registry_all_signals_preserves_base_and_retained_attributes() {
    let baseline = resolve(BASE).unwrap();
    for kind in ["span", "metric", "event", "entity"] {
        let yaml = format!("{BASE}{}", refinement(kind, "      - unref: test.remove"));
        let resolved = resolve(&yaml).unwrap();
        let original = attrs(&baseline, &format!("{kind}.{kind}_base"));
        assert_eq!(attrs(&resolved, &format!("{kind}.{kind}_base")), original);
        let expected: Vec<_> = original
            .into_iter()
            .filter(|a| a.name != "test.remove")
            .collect();
        assert_eq!(attrs(&resolved, &format!("{kind}.refined")), expected);
        let group = resolved.group(&format!("{kind}.refined")).unwrap();
        assert!(!serde_json::to_string(group)
            .unwrap()
            .contains("test.remove"));
        assert!(!serde_json::to_string(group).unwrap().contains("unref"));
    }
}

#[test]
fn unref_missing_or_registered_but_not_inherited_is_an_error() {
    for kind in ["span", "metric", "event", "entity"] {
        for key in ["test.unknown", "test.unused"] {
            let yaml = format!(
                "{BASE}{}",
                refinement(kind, &format!("      - unref: {key}"))
            );
            let error = resolve(&yaml).unwrap_err();
            let (id, attr, reason) = unref_error(&error);
            assert_eq!(id, format!("{kind}.refined"));
            assert_eq!(attr, key);
            assert!(reason.contains("not inherited"), "{reason}");
        }
    }
}

#[test]
fn unref_conflicts_with_explicit_ref_in_either_order() {
    for kind in ["span", "metric", "event", "entity"] {
        for key in ["test.remove", "test.unused"] {
            for reverse in [false, true] {
                let mut entries = [
                    format!("      - ref: {key}"),
                    format!("      - unref: {key}"),
                ];
                if reverse {
                    entries.reverse();
                }
                let yaml = format!("{BASE}{}", refinement(kind, &entries.join("\n")));
                let error = resolve(&yaml).unwrap_err();
                assert!(unref_error(&error).2.contains("explicitly referenced"));
            }
        }
    }
}

#[test]
fn unref_conflicts_with_nested_group_in_either_order() {
    for kind in ["span", "metric", "event"] {
        for entries in [
            "      - ref_group: nested\n      - unref: test.remove",
            "      - unref: test.remove\n      - ref_group: nested",
        ] {
            let error = resolve(&format!("{BASE}{}", refinement(kind, entries))).unwrap_err();
            assert!(unref_error(&error).2.contains("ref_group `nested`"));
        }
    }
}

#[test]
fn unref_duplicate_is_an_error() {
    let entries = "      - unref: test.remove\n      - unref: test.remove";
    let error = resolve(&format!("{BASE}{}", refinement("span", entries))).unwrap_err();
    assert_eq!(unref_error(&error).2, "duplicate unref entry");
}

#[test]
fn unref_identity_through_description_is_an_error() {
    let error = resolve(&format!(
        "{BASE}{}",
        refinement("entity", "      - unref: test.id")
    ))
    .unwrap_err();
    assert!(unref_error(&error).2.contains("must preserve identity"));
}

#[test]
fn unref_required_attribute_is_an_error() {
    let base = BASE.replace(
        "        requirement_level: recommended",
        "        requirement_level: required",
    );
    for kind in ["span", "metric", "event", "entity"] {
        let yaml = format!("{base}{}", refinement(kind, "      - unref: test.remove"));
        let error = resolve(&yaml).unwrap_err();
        let (_, attr, reason) = unref_error(&error);
        assert_eq!(attr, "test.remove");
        assert!(reason.contains("is required by"), "{reason}");
    }
}

#[test]
fn unref_all_attributes_leaves_an_empty_span_refinement() {
    let entries = "      - unref: test.remove\n      - unref: test.keep";
    let resolved = resolve(&format!("{BASE}{}", refinement("span", entries))).unwrap();
    assert!(attrs(&resolved, "span.refined").is_empty());
    assert_eq!(attrs(&resolved, "span.span_base").len(), 2);
}

#[test]
fn unref_does_not_affect_sibling_or_local_overrides() {
    let yaml = format!("{BASE}{}\n  - id: span.sibling\n    ref: span_base\n", refinement("span",
        "      - unref: test.remove\n      - ref: test.keep\n        brief: Override\n        sampling_relevant: false\n      - ref: test.unused"));
    let resolved = resolve(&yaml).unwrap();
    let refined = attrs(&resolved, "span.refined");
    assert_eq!(
        refined.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["test.keep", "test.unused"]
    );
    assert_eq!(refined[0].brief, "Override");
    assert_eq!(refined[0].sampling_relevant, Some(false));
    assert_eq!(
        attrs(&resolved, "span.sibling"),
        attrs(&resolved, "span.span_base")
    );
}

#[test]
fn unref_is_absent_from_resolved_v2_attributes() {
    use weaver_resolved_schema::v2::catalog::AttributeCatalog;
    let mut yaml = BASE.to_owned();
    for kind in ["span", "metric", "event", "entity"] {
        yaml.push_str(&refinement(kind, "      - unref: test.remove"));
    }
    let v2 = V2Schema::try_from(resolve(&yaml).unwrap()).unwrap();
    let span = v2
        .refinements
        .spans
        .iter()
        .find(|r| &*r.id == "refined")
        .unwrap();
    assert_eq!(span.span.attributes.len(), 1);
    assert_eq!(
        v2.attribute_catalog
            .attribute_key(&span.span.attributes[0].base),
        Some("test.keep")
    );
    let event = &v2
        .refinements
        .events
        .iter()
        .find(|r| &*r.id == "refined")
        .unwrap()
        .event;
    let metric = &v2
        .refinements
        .metrics
        .iter()
        .find(|r| &*r.id == "refined")
        .unwrap()
        .metric;
    let entity = &v2
        .refinements
        .entities
        .iter()
        .find(|r| &*r.id == "refined")
        .unwrap()
        .entity;
    for refs in [
        event.attributes.iter().map(|a| a.base).collect::<Vec<_>>(),
        metric.attributes.iter().map(|a| a.base).collect(),
        entity.description.iter().map(|a| a.base).collect(),
    ] {
        assert_eq!(
            refs.iter()
                .map(|r| v2.attribute_catalog.attribute_key(r).unwrap())
                .collect::<Vec<_>>(),
            ["test.keep"]
        );
    }
    assert_eq!(entity.identity.len(), 1);
    assert_eq!(
        v2.attribute_catalog.attribute_key(&entity.identity[0]),
        Some("test.id")
    );
    assert!(!serde_json::to_string(&v2).unwrap().contains("unref"));
}

#[test]
fn unref_parent_in_another_registry_v1_and_v2() {
    let url = SchemaUrl::try_from("https://example.com/parent/1.0.0").unwrap();
    let mut parent = LoadedSemconvRegistry::create_from_string(BASE).unwrap();
    let LoadedSemconvRegistry::Unresolved { repo, specs, .. } = &mut parent else {
        unreachable!()
    };
    *repo =
        RegistryRepo::try_new(Some(url.clone()), &"data".try_into().unwrap(), &mut vec![]).unwrap();
    for spec in specs {
        spec.provenance.schema_url = url.clone();
    }
    let parent = resolve_loaded(parent).unwrap();
    for published_v2 in [false, true] {
        let dependency = if published_v2 {
            LoadedSemconvRegistry::ResolvedV2 {
                schema: V2Schema::try_from(parent.clone()).unwrap(),
                direct_dependencies: vec![],
            }
        } else {
            LoadedSemconvRegistry::Resolved {
                schema: parent.clone(),
                direct_dependencies: vec![],
            }
        };
        let mut yaml = "file_format: definition/2\n".to_owned();
        for kind in ["span", "metric", "event", "entity"] {
            yaml.push_str(&refinement(kind, "      - unref: test.remove"));
        }
        let mut loaded = LoadedSemconvRegistry::create_from_string(&yaml).unwrap();
        let LoadedSemconvRegistry::Unresolved { dependencies, .. } = &mut loaded else {
            unreachable!()
        };
        dependencies.push(dependency);
        let resolved = resolve_loaded(loaded).unwrap();
        for kind in ["span", "metric", "event", "entity"] {
            let refined = attrs(&resolved, &format!("{kind}.refined"));
            assert!(refined.iter().all(|a| a.name != "test.remove"));
            assert_eq!(refined.len(), if kind == "entity" { 2 } else { 1 });
        }
        let yaml = format!(
            "file_format: definition/2\n{}",
            refinement("entity", "      - unref: test.id")
        );
        let mut loaded = LoadedSemconvRegistry::create_from_string(&yaml).unwrap();
        let LoadedSemconvRegistry::Unresolved { dependencies, .. } = &mut loaded else {
            unreachable!()
        };
        dependencies.push(if published_v2 {
            LoadedSemconvRegistry::ResolvedV2 {
                schema: V2Schema::try_from(parent.clone()).unwrap(),
                direct_dependencies: vec![],
            }
        } else {
            LoadedSemconvRegistry::Resolved {
                schema: parent.clone(),
                direct_dependencies: vec![],
            }
        });
        let error = resolve_loaded(loaded).unwrap_err();
        assert!(unref_error(&error).2.contains("must preserve identity"));
    }
}
