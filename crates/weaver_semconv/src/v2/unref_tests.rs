// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{schema_url::SchemaUrl, semconv::SemConvSpecWithProvenance};
use std::{fs, path::Path};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data/unref")
        .join(name);
    fs::read_to_string(path).unwrap()
}

#[test]
fn unref_rust_constructors_preserve_flat_yaml() {
    use attribute::{GroupRef, RefinementAttributeOrGroupRef};
    use entity::EntityAttributeRefinement;
    use span::{SpanAttributeRef, SpanGroupRef, SpanRefinementAttributeOrGroupRef};

    let attr: AttributeRef = serde_yaml::from_str("ref: test.attr").unwrap();
    let ordinary: RefinementAttributeOrGroupRef =
        RefinementAttributeOrGroupRef::reference(attr.clone());
    let group: RefinementAttributeOrGroupRef = RefinementAttributeOrGroupRef::reference(GroupRef {
        ref_group: "shared".into(),
    });
    let span = SpanRefinementAttributeOrGroupRef::reference(SpanAttributeRef {
        base: attr.clone(),
        sampling_relevant: Some(true),
    });
    let span_group = SpanRefinementAttributeOrGroupRef::reference(SpanGroupRef {
        ref_group: "shared".to_owned(),
    });
    let entity = EntityAttributeRefinement::reference(attr);
    for (actual, expected) in [
        (
            serde_json::to_value(ordinary).unwrap(),
            serde_json::json!({"ref": "test.attr"}),
        ),
        (
            serde_json::to_value(group).unwrap(),
            serde_json::json!({"ref_group": "shared"}),
        ),
        (
            serde_json::to_value(span).unwrap(),
            serde_json::json!({"ref": "test.attr", "sampling_relevant": true}),
        ),
        (
            serde_json::to_value(span_group).unwrap(),
            serde_json::json!({"ref_group": "shared"}),
        ),
        (
            serde_json::to_value(entity).unwrap(),
            serde_json::json!({"ref": "test.attr"}),
        ),
        (
            serde_json::to_value(SpanRefinementAttributeOrGroupRef::unref("test.attr")).unwrap(),
            serde_json::json!({"unref": "test.attr"}),
        ),
    ] {
        assert_eq!(actual, expected);
    }
}

#[test]
fn unref_schema_reuses_reference_definitions() {
    let schema = serde_json::to_value(SemConvSpecV2::output_schema()).unwrap();
    for (refinement, field, references) in [
        (
            "MetricRefinement",
            "attributes",
            vec!["AttributeRef", "GroupRef", "AttributeUnref"],
        ),
        (
            "EventRefinement",
            "attributes",
            vec!["AttributeRef", "GroupRef", "AttributeUnref"],
        ),
        (
            "SpanRefinement",
            "attributes",
            vec!["SpanAttributeRef", "SpanGroupRef", "AttributeUnref"],
        ),
        (
            "EntityRefinement",
            "description",
            vec!["AttributeRef", "AttributeUnref"],
        ),
    ] {
        let alternatives = schema["$defs"][refinement]["properties"][field]["items"]["anyOf"]
            .as_array()
            .unwrap();
        let actual: Vec<_> = alternatives
            .iter()
            .map(|item| item["$ref"].as_str().unwrap())
            .collect();
        let expected: Vec<_> = references
            .iter()
            .map(|name| format!("#/$defs/{name}"))
            .collect();
        assert_eq!(actual, expected, "{refinement}");
    }
    for name in [
        "RefinementAttributeOrGroupRef",
        "SpanRefinementAttributeOrGroupRef",
        "EntityAttributeRefinement",
    ] {
        assert!(schema["$defs"].get(name).is_none());
    }
}

#[test]
fn unref_round_trips_for_all_refinement_types() {
    let yaml = fixture("refinements.yaml");
    let spec: SemConvSpecV2 = serde_yaml::from_str(&yaml).unwrap();
    let value = serde_json::to_value(&spec).unwrap();
    for (kind, field) in [
        ("span", "attributes"),
        ("metric", "attributes"),
        ("event", "attributes"),
        ("entity", "description"),
    ] {
        assert_eq!(
            value[format!("{kind}_refinements")][0][field][0],
            serde_json::json!({"unref": "test.attr"})
        );
    }
    assert_eq!(
        serde_yaml::from_str::<serde_yaml::Value>(&serde_yaml::to_string(&spec).unwrap()).unwrap(),
        serde_yaml::from_str::<serde_yaml::Value>(&yaml).unwrap()
    );
    let schema = serde_json::to_value(SemConvSpecV2::output_schema()).unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&value));
    let converted = crate::semconv::Versioned::V2(spec).into_v1("unref.yaml");
    assert_eq!(converted.groups.len(), 4);
    for group in &converted.groups {
        assert_eq!(group.attribute_unrefs, ["test.attr"]);
    }
    assert!(!serde_json::to_string(&converted).unwrap().contains("unref"));
}

#[test]
fn unref_is_rejected_on_base_signals_and_attribute_groups() {
    for file in [
        "base-span.yaml",
        "base-metric.yaml",
        "base-event.yaml",
        "base-entity.yaml",
        "internal-group.yaml",
        "public-group.yaml",
        "entity-identity.yaml",
        "invalid-v1-unref.yaml",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/unref")
            .join(file);
        let error = SemConvSpecWithProvenance::from_file(SchemaUrl::new_unknown(), path)
            .into_result_failing_non_fatal()
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Object contains unexpected properties: unref."),
            "{file}: {error}"
        );
    }
}

#[test]
fn unref_rejects_mixed_entries_and_override_fields() {
    for kind in ["span", "metric", "event", "entity"] {
        for field in [
            "ref",
            "ref-group",
            "requirement-level",
            "note",
            "sampling-relevant",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data/unref/invalid")
                .join(format!("{kind}-unref-with-{field}.yaml"));
            let error = SemConvSpecWithProvenance::from_file(SchemaUrl::new_unknown(), &path)
                .into_result_failing_non_fatal()
                .unwrap_err();
            let message = error.to_string();
            assert!(
                matches!(
                    error,
                    Error::InvalidSemConvSpec(_) | Error::CompoundError(_)
                ),
                "{}: {message}",
                path.display()
            );
            assert!(
                message.contains("Object contains unexpected properties:")
                    && message.contains("unref"),
                "{}: {message}",
                path.display()
            );
        }
    }
}
