// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `unref` attribute removal across V1 and V2 resolution targets.

use std::cell::Cell;
use weaver_resolved_schema::v2::catalog::AttributeCatalog;

use super::*;

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

thread_local! {
    static CURRENT_TARGET: Cell<ResolutionTarget> = const { Cell::new(ResolutionTarget::V1) };
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttrInfo {
    name: String,
    brief: String,
    sampling_relevant: Option<bool>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum SchemaWrapper {
    V1(ResolvedTelemetrySchema),
    V2(V2Schema),
}

impl SchemaWrapper {
    fn attrs(&self, id: &str) -> Vec<AttrInfo> {
        match self {
            Self::V1(s) => {
                let group = s
                    .group(id)
                    .unwrap_or_else(|| panic!("group {id} not found in V1"));
                group
                    .attributes
                    .iter()
                    .map(|r| {
                        let a = s.catalog.attribute(r).unwrap();
                        AttrInfo {
                            name: a.name.clone(),
                            brief: a.brief.clone(),
                            sampling_relevant: a.sampling_relevant,
                        }
                    })
                    .collect()
            }
            Self::V2(v2) => {
                if let Some(rest) = id.strip_prefix("span.") {
                    if let Some(sp) = v2.registry.spans.iter().find(|s| &*s.r#type == rest) {
                        return sp
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: aref.sampling_relevant,
                                }
                            })
                            .collect();
                    }
                    if let Some(sp_ref) = v2
                        .refinements
                        .spans
                        .iter()
                        .find(|r| &*r.id == rest || &*r.id == id)
                    {
                        return sp_ref
                            .span
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: aref.sampling_relevant,
                                }
                            })
                            .collect();
                    }
                }
                if let Some(rest) = id.strip_prefix("metric.") {
                    if let Some(m) = v2.registry.metrics.iter().find(|m| &*m.name == rest) {
                        return m
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: None,
                                }
                            })
                            .collect();
                    }
                    if let Some(m_ref) = v2
                        .refinements
                        .metrics
                        .iter()
                        .find(|r| &*r.id == rest || &*r.id == id)
                    {
                        return m_ref
                            .metric
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: None,
                                }
                            })
                            .collect();
                    }
                }
                if let Some(rest) = id.strip_prefix("event.") {
                    if let Some(e) = v2.registry.events.iter().find(|e| &*e.name == rest) {
                        return e
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: None,
                                }
                            })
                            .collect();
                    }
                    if let Some(e_ref) = v2
                        .refinements
                        .events
                        .iter()
                        .find(|r| &*r.id == rest || &*r.id == id)
                    {
                        return e_ref
                            .event
                            .attributes
                            .iter()
                            .map(|aref| {
                                let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                                AttrInfo {
                                    name: a.key.clone(),
                                    brief: a.common.brief.clone(),
                                    sampling_relevant: None,
                                }
                            })
                            .collect();
                    }
                }
                if let Some(rest) = id.strip_prefix("entity.") {
                    if let Some(en) = v2.registry.entities.iter().find(|en| &*en.r#type == rest) {
                        let mut res = Vec::new();
                        for aref in &en.identity {
                            let a = v2.attribute_catalog.attribute(aref).unwrap();
                            res.push(AttrInfo {
                                name: a.key.clone(),
                                brief: a.common.brief.clone(),
                                sampling_relevant: None,
                            });
                        }
                        for aref in &en.description {
                            let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                            res.push(AttrInfo {
                                name: a.key.clone(),
                                brief: a.common.brief.clone(),
                                sampling_relevant: None,
                            });
                        }
                        return res;
                    }
                    if let Some(en_ref) = v2
                        .refinements
                        .entities
                        .iter()
                        .find(|r| &*r.id == rest || &*r.id == id)
                    {
                        let mut res = Vec::new();
                        for aref in &en_ref.entity.identity {
                            let a = v2.attribute_catalog.attribute(aref).unwrap();
                            res.push(AttrInfo {
                                name: a.key.clone(),
                                brief: a.common.brief.clone(),
                                sampling_relevant: None,
                            });
                        }
                        for aref in &en_ref.entity.description {
                            let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                            res.push(AttrInfo {
                                name: a.key.clone(),
                                brief: a.common.brief.clone(),
                                sampling_relevant: None,
                            });
                        }
                        return res;
                    }
                }
                if let Some(sp_ref) = v2.refinements.spans.iter().find(|r| &*r.id == id) {
                    return sp_ref
                        .span
                        .attributes
                        .iter()
                        .map(|aref| {
                            let a = v2.attribute_catalog.attribute(&aref.base).unwrap();
                            AttrInfo {
                                name: a.key.clone(),
                                brief: a.common.brief.clone(),
                                sampling_relevant: aref.sampling_relevant,
                            }
                        })
                        .collect();
                }
                panic!("Signal or refinement {id} not found in V2 schema");
            }
        }
    }

    fn to_v2(&self) -> V2Schema {
        match self {
            Self::V1(s) => V2Schema::try_from(s.clone()).unwrap(),
            Self::V2(s) => s.clone(),
        }
    }
}

fn resolve(yaml: &str) -> Result<SchemaWrapper, Error> {
    let loaded = LoadedSemconvRegistry::create_from_string(yaml)?;
    resolve_loaded(loaded)
}

fn resolve_loaded(loaded: LoadedSemconvRegistry) -> Result<SchemaWrapper, Error> {
    let target = CURRENT_TARGET.with(|c| c.get());
    let resolved = WeaverResolver::new(WeaverResolverConfig {
        target,
        ..Default::default()
    })
    .resolve_loaded(loaded)
    .into_result_failing_non_fatal()?;
    match Arc::unwrap_or_clone(resolved) {
        WeaverResolvedSchema::V1(v1) => Ok(SchemaWrapper::V1(v1)),
        WeaverResolvedSchema::V2(v2) => Ok(SchemaWrapper::V2(v2)),
    }
}

fn for_both_targets<F: Fn()>(f: F) {
    for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
        CURRENT_TARGET.with(|c| c.set(target));
        f();
    }
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

fn attrs(schema: &SchemaWrapper, id: &str) -> Vec<AttrInfo> {
    schema.attrs(id)
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
    for_both_targets(|| {
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
            match &resolved {
                SchemaWrapper::V1(s) => {
                    let group = s.group(&format!("{kind}.refined")).unwrap();
                    assert!(!serde_json::to_string(group)
                        .unwrap()
                        .contains("test.remove"));
                    assert!(!serde_json::to_string(group).unwrap().contains("unref"));
                }
                SchemaWrapper::V2(v2) => match kind {
                    "span" => {
                        let r = v2
                            .refinements
                            .spans
                            .iter()
                            .find(|r| &*r.id == "refined")
                            .unwrap();
                        assert!(!serde_json::to_string(r).unwrap().contains("unref"));
                    }
                    "metric" => {
                        let r = v2
                            .refinements
                            .metrics
                            .iter()
                            .find(|r| &*r.id == "refined")
                            .unwrap();
                        assert!(!serde_json::to_string(r).unwrap().contains("unref"));
                    }
                    "event" => {
                        let r = v2
                            .refinements
                            .events
                            .iter()
                            .find(|r| &*r.id == "refined")
                            .unwrap();
                        assert!(!serde_json::to_string(r).unwrap().contains("unref"));
                    }
                    "entity" => {
                        let r = v2
                            .refinements
                            .entities
                            .iter()
                            .find(|r| &*r.id == "refined")
                            .unwrap();
                        assert!(!serde_json::to_string(r).unwrap().contains("unref"));
                    }
                    _ => unreachable!(),
                },
            }
        }
    });
}

#[test]
fn unref_missing_or_registered_but_not_inherited_is_an_error() {
    for_both_targets(|| {
        for kind in ["span", "metric", "event", "entity"] {
            for key in ["test.unknown", "test.unused"] {
                let yaml = format!(
                    "{BASE}{}",
                    refinement(kind, &format!("      - unref: {key}"))
                );
                let error = resolve(&yaml).unwrap_err();
                let (id, attr, reason) = unref_error(&error);
                assert!(id == format!("{kind}.refined") || id == "refined");
                assert_eq!(attr, key);
                assert!(reason.contains("not inherited"), "{reason}");
            }
        }
    });
}

#[test]
fn unref_conflicts_with_explicit_ref_in_either_order() {
    for_both_targets(|| {
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
    });
}

#[test]
fn unref_conflicts_with_nested_group_in_either_order() {
    for_both_targets(|| {
        for kind in ["span", "metric", "event"] {
            for entries in [
                "      - ref_group: nested\n      - unref: test.remove",
                "      - unref: test.remove\n      - ref_group: nested",
            ] {
                let error = resolve(&format!("{BASE}{}", refinement(kind, entries))).unwrap_err();
                assert!(unref_error(&error).2.contains("ref_group `nested`"));
            }
        }
    });
}

#[test]
fn unref_duplicate_is_an_error() {
    for_both_targets(|| {
        let entries = "      - unref: test.remove\n      - unref: test.remove";
        let error = resolve(&format!("{BASE}{}", refinement("span", entries))).unwrap_err();
        assert_eq!(unref_error(&error).2, "duplicate unref entry");
    });
}

#[test]
fn unref_identity_through_description_is_an_error() {
    for_both_targets(|| {
        let error = resolve(&format!(
            "{BASE}{}",
            refinement("entity", "      - unref: test.id")
        ))
        .unwrap_err();
        assert!(unref_error(&error).2.contains("must preserve identity"));
    });
}

#[test]
fn unref_required_attribute_is_an_error() {
    for_both_targets(|| {
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
    });
}

#[test]
fn unref_all_attributes_leaves_an_empty_span_refinement() {
    for_both_targets(|| {
        let entries = "      - unref: test.remove\n      - unref: test.keep";
        let resolved = resolve(&format!("{BASE}{}", refinement("span", entries))).unwrap();
        assert!(attrs(&resolved, "span.refined").is_empty());
        assert_eq!(attrs(&resolved, "span.span_base").len(), 2);
    });
}

#[test]
fn unref_does_not_affect_sibling_or_local_overrides() {
    for_both_targets(|| {
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
    });
}

#[test]
fn unref_is_absent_from_resolved_v2_attributes() {
    for_both_targets(|| {
        use weaver_resolved_schema::v2::catalog::AttributeCatalog;
        let mut yaml = BASE.to_owned();
        for kind in ["span", "metric", "event", "entity"] {
            yaml.push_str(&refinement(kind, "      - unref: test.remove"));
        }
        let v2 = resolve(&yaml).unwrap().to_v2();
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
    });
}

#[test]
fn unref_parent_in_another_registry_v1_and_v2() {
    for_both_targets(|| {
        let url = SchemaUrl::try_from("https://example.com/parent/1.0.0").unwrap();
        let mut parent = LoadedSemconvRegistry::create_from_string(BASE).unwrap();
        let LoadedSemconvRegistry::Unresolved { repo, specs, .. } = &mut parent else {
            unreachable!()
        };
        *repo = RegistryRepo::try_new(Some(url.clone()), &"data".try_into().unwrap(), &mut vec![])
            .unwrap();
        for spec in specs {
            spec.provenance.schema_url = url.clone();
        }
        let resolved_parent = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V1,
            ..Default::default()
        })
        .resolve_loaded(parent)
        .into_result_failing_non_fatal()
        .unwrap();
        let parent_v1 = match Arc::unwrap_or_clone(resolved_parent) {
            WeaverResolvedSchema::V1(s) => s,
            WeaverResolvedSchema::V2(_) => unreachable!(),
        };
        for published_v2 in [false, true] {
            let dependency = if published_v2 {
                LoadedSemconvRegistry::ResolvedV2 {
                    schema: V2Schema::try_from(parent_v1.clone()).unwrap(),
                    direct_dependencies: vec![],
                }
            } else {
                LoadedSemconvRegistry::Resolved {
                    schema: parent_v1.clone(),
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
                    schema: V2Schema::try_from(parent_v1.clone()).unwrap(),
                    direct_dependencies: vec![],
                }
            } else {
                LoadedSemconvRegistry::Resolved {
                    schema: parent_v1.clone(),
                    direct_dependencies: vec![],
                }
            });
            let error = resolve_loaded(loaded).unwrap_err();
            assert!(unref_error(&error).2.contains("must preserve identity"));
        }
    });
}
