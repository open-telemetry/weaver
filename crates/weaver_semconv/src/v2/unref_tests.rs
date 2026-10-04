// SPDX-License-Identifier: Apache-2.0

use super::*;

fn accepts(yaml: &str) -> bool {
    serde_yaml::from_str::<SemConvSpecV2>(yaml).is_ok()
}

#[test]
fn unref_rust_constructors_preserve_flat_yaml() {
    use attribute::{AttributeRefinement, GroupRef};
    use entity::EntityAttributeRefinement;
    use span::{SpanAttributeRef, SpanAttributeRefinement, SpanGroupRef};

    let attr: AttributeRef = serde_yaml::from_str("ref: test.attr").unwrap();
    let ordinary: AttributeRefinement = AttributeRefinement::reference(attr.clone());
    let group: AttributeRefinement = AttributeRefinement::reference(GroupRef {
        ref_group: "shared".into(),
    });
    let span = SpanAttributeRefinement::reference(SpanAttributeRef {
        base: attr.clone(),
        sampling_relevant: Some(true),
    });
    let span_group = SpanAttributeRefinement::reference(SpanGroupRef {
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
            serde_json::to_value(SpanAttributeRefinement::unref("test.attr")).unwrap(),
            serde_json::json!({"unref": "test.attr"}),
        ),
    ] {
        assert_eq!(actual, expected);
    }
}

#[test]
fn unref_schema_reuses_reference_definitions() {
    let schema = serde_json::to_value(SemConvSpecV2::output_schema()).unwrap();
    for (refinement, field, reference) in [
        ("MetricRefinement", "attributes", "AttributeOrGroupRef"),
        ("EventRefinement", "attributes", "AttributeOrGroupRef"),
        ("SpanRefinement", "attributes", "SpanAttributeOrGroupRef"),
        ("EntityRefinement", "description", "AttributeRef"),
    ] {
        let wrapper = schema["$defs"][refinement]["properties"][field]["items"]["$ref"]
            .as_str()
            .unwrap();
        let alternatives = schema.pointer(&wrapper[1..]).unwrap()["anyOf"]
            .as_array()
            .unwrap();
        assert_eq!(alternatives.len(), 2);
        assert_eq!(alternatives[0]["$ref"], format!("#/$defs/{reference}"));
        assert_eq!(alternatives[1]["$ref"], "#/$defs/AttributeUnref");
    }
}

#[test]
fn unref_checked_in_schema_matches_generated_schema() {
    let checked_in: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schemas/semconv.schema.v2.json")).unwrap();
    assert_eq!(
        checked_in,
        serde_json::to_value(SemConvSpecV2::output_schema()).unwrap()
    );
}

#[test]
fn unref_round_trips_for_all_refinement_types() {
    for kind in ["span", "metric", "event", "entity"] {
        let field = if kind == "entity" {
            "description"
        } else {
            "attributes"
        };
        let yaml = format!("{kind}_refinements:\n  - id: refined\n    ref: base\n    {field}:\n      - unref: test.attr\n");
        let spec: SemConvSpecV2 = serde_yaml::from_str(&yaml).unwrap();
        let value = serde_json::to_value(&spec).unwrap();
        assert_eq!(
            value[format!("{kind}_refinements")][0][field][0],
            serde_json::json!({"unref": "test.attr"})
        );
        assert_eq!(
            serde_yaml::from_str::<serde_yaml::Value>(&serde_yaml::to_string(&spec).unwrap())
                .unwrap(),
            serde_yaml::from_str::<serde_yaml::Value>(&yaml).unwrap()
        );
        let schema = serde_json::to_value(SemConvSpecV2::output_schema()).unwrap();
        assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&value));
        let converted = crate::semconv::Versioned::V2(spec).into_v1("unref.yaml");
        assert_eq!(converted.groups[0].attribute_unrefs, ["test.attr"]);
        assert!(!serde_json::to_string(&converted).unwrap().contains("unref"));
    }
}

#[test]
fn unref_is_rejected_on_base_signals_and_attribute_groups() {
    for yaml in [
        "spans:\n  - type: base\n    kind: client\n    name: {note: Name}\n    brief: Base\n    stability: stable\n    attributes: [{unref: test.attr}]",
        "metrics:\n  - name: base\n    instrument: counter\n    unit: '1'\n    brief: Base\n    stability: stable\n    attributes: [{unref: test.attr}]",
        "events:\n  - name: base\n    brief: Base\n    stability: stable\n    attributes: [{unref: test.attr}]",
        "entities:\n  - type: base\n    brief: Base\n    stability: stable\n    identity: []\n    description: [{unref: test.attr}]",
        "attribute_groups:\n  - id: base\n    visibility: internal\n    attributes: [{ref: test.attr}]\n  - id: child\n    visibility: internal\n    attributes: [{ref_group: base}, {unref: test.attr}]",
        "attribute_groups:\n  - id: base\n    visibility: public\n    brief: Base\n    stability: stable\n    attributes: [{unref: test.attr}]",
        "entity_refinements:\n  - id: refined\n    ref: base\n    identity: [{unref: test.attr}]",
    ] {
        assert!(!accepts(yaml), "Unexpectedly accepted {yaml}");
        let schema = serde_json::to_value(SemConvSpecV2::output_schema()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let value: serde_json::Value = serde_yaml::from_str(yaml).unwrap();
        assert!(!validator.is_valid(&value), "Schema unexpectedly accepted {yaml}");
    }
}

#[test]
fn unref_is_rejected_on_v1_group_extending_another_group() {
    let yaml = "groups:\n  - id: base\n    type: attribute_group\n    brief: Base\n    attributes: [{id: test.attr, type: string, brief: Attribute}]\n  - id: child\n    type: attribute_group\n    brief: Child\n    extends: base\n    attributes: [{unref: test.attr}]";
    assert!(serde_yaml::from_str::<crate::v1::semconv::SemConvSpecV1>(yaml).is_err());
}

#[test]
fn unref_rejects_mixed_entries_and_override_fields() {
    for kind in ["span", "metric", "event", "entity"] {
        let field = if kind == "entity" {
            "description"
        } else {
            "attributes"
        };
        for entry in [
            "{unref: test.attr, ref: test.attr}",
            "{unref: test.attr, ref_group: shared}",
            "{unref: test.attr, requirement_level: required}",
            "{unref: test.attr, note: Note}",
            "{unref: test.attr, sampling_relevant: true}",
        ] {
            let yaml = format!(
                "{kind}_refinements:\n  - id: refined\n    ref: base\n    {field}: [{entry}]"
            );
            assert!(!accepts(&yaml), "Unexpectedly accepted {yaml}");
        }
    }
}
