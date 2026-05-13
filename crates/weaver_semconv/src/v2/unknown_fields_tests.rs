// SPDX-License-Identifier: Apache-2.0

//! Forward-compat tolerance tests for `file_format: definition/2`.
//!
//! `definition/2` has no minor, so unknowns are log-only. Tests rerun
//! `collect_paths` (same algorithm as the loader) to verify what would be reported.
//! Two extra tests pin loader plumbing for explicit known/newer minors.

use std::io::Write;

use weaver_common::result::WResult;

use crate::semconv::SemConvSpecWithProvenance;
use crate::v2::SemConvSpecV2;
use crate::Error;

fn load(yaml: &str) -> WResult<SemConvSpecWithProvenance, Error> {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(yaml.as_bytes()).expect("write temp file");
    SemConvSpecWithProvenance::from_file(crate::schema_url::SchemaUrl::new_unknown(), tmp.path())
}

/// Runs `collect_paths` on the fragment (mirroring `clean_yaml_mapping`) and asserts
/// some unknown path contains `marker`.
fn assert_warns_about(yaml: &str, marker: &str) {
    let raw: serde_yaml::Value = serde_yaml::from_str(yaml).expect("test yaml should parse");
    let cleaned = strip_format_keys(raw);
    let typed: SemConvSpecV2 = serde_yaml::from_value(cleaned.clone())
        .expect("test yaml should deserialize as SemConvSpecV2");
    let unknowns = crate::unexpected_fields::collect_paths(&cleaned, &typed);
    assert!(
        unknowns.iter().any(|p| p.contains(marker)),
        "expected an unknown-field path containing {marker:?}, got: {unknowns:?}"
    );
}

fn strip_format_keys(value: serde_yaml::Value) -> serde_yaml::Value {
    match value {
        serde_yaml::Value::Mapping(mut m) => {
            let _ = m.remove(serde_yaml::Value::String("file_format".to_owned()));
            let _ = m.remove(serde_yaml::Value::String("version".to_owned()));
            serde_yaml::Value::Mapping(m)
        }
        other => other,
    }
}

// ---- top-level SemConvSpecV2 ----

#[test]
fn definition_v2_warns_unknown_at_top_level() {
    let yaml = r#"
file_format: definition/2
typo_top_level: bad
"#;
    assert_warns_about(yaml, "typo_top_level");
}

// ---- AttributeDef ----

#[test]
fn definition_v2_warns_unknown_inside_attribute_def() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    typo_in_attr_def: bad
"#;
    assert_warns_about(yaml, "typo_in_attr_def");
}

// ---- AttributeRef ----

#[test]
fn definition_v2_warns_unknown_inside_attribute_ref() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    attributes:
      - ref: some.attr
        typo_in_attr_ref: bad
"#;
    assert_warns_about(yaml, "typo_in_attr_ref");
}

// ---- GroupRef ----

#[test]
fn definition_v2_warns_unknown_inside_group_ref() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    attributes:
      - ref_group: some.group
        typo_in_group_ref: bad
"#;
    assert_warns_about(yaml, "typo_in_group_ref");
}

// ---- Entity ----

#[test]
fn definition_v2_warns_unknown_inside_entity() {
    let yaml = r#"
file_format: definition/2
entities:
  - type: my.entity
    identity:
      - ref: some.attr
    brief: t
    stability: stable
    typo_in_entity: bad
"#;
    assert_warns_about(yaml, "typo_in_entity");
}

// ---- EntityRefinement ----

#[test]
fn definition_v2_warns_unknown_inside_entity_refinement() {
    let yaml = r#"
file_format: definition/2
entity_refinements:
  - id: my.entity.refined
    ref: my.entity
    typo_in_entity_refinement: bad
"#;
    assert_warns_about(yaml, "typo_in_entity_refinement");
}

// ---- Event ----

#[test]
fn definition_v2_warns_unknown_inside_event() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    typo_in_event: bad
"#;
    assert_warns_about(yaml, "typo_in_event");
}

// ---- EventRefinement ----

#[test]
fn definition_v2_warns_unknown_inside_event_refinement() {
    let yaml = r#"
file_format: definition/2
event_refinements:
  - id: my.event.refined
    ref: my.event
    typo_in_event_refinement: bad
"#;
    assert_warns_about(yaml, "typo_in_event_refinement");
}

// ---- Metric ----

#[test]
fn definition_v2_warns_unknown_inside_metric() {
    let yaml = r#"
file_format: definition/2
metrics:
  - name: my.metric
    instrument: counter
    unit: s
    brief: t
    stability: stable
    typo_in_metric: bad
"#;
    assert_warns_about(yaml, "typo_in_metric");
}

// ---- MetricRefinement ----

#[test]
fn definition_v2_warns_unknown_inside_metric_refinement() {
    let yaml = r#"
file_format: definition/2
metric_refinements:
  - id: my.metric.refined
    ref: my.metric
    typo_in_metric_refinement: bad
"#;
    assert_warns_about(yaml, "typo_in_metric_refinement");
}

// ---- Span ----

#[test]
fn definition_v2_warns_unknown_inside_span() {
    let yaml = r#"
file_format: definition/2
spans:
  - type: my.span
    kind: client
    name:
      note: how to name
    brief: t
    stability: stable
    typo_in_span: bad
"#;
    assert_warns_about(yaml, "typo_in_span");
}

// ---- SpanRefinement ----

#[test]
fn definition_v2_warns_unknown_inside_span_refinement() {
    let yaml = r#"
file_format: definition/2
span_refinements:
  - id: my.span.refined
    ref: my.span
    typo_in_span_refinement: bad
"#;
    assert_warns_about(yaml, "typo_in_span_refinement");
}

// ---- SpanName ----

#[test]
fn definition_v2_warns_unknown_inside_span_name() {
    let yaml = r#"
file_format: definition/2
spans:
  - type: my.span
    kind: client
    name:
      note: how to name
      typo_in_span_name: bad
    brief: t
    stability: stable
"#;
    assert_warns_about(yaml, "typo_in_span_name");
}

// ---- SpanGroupRef ----

#[test]
fn definition_v2_warns_unknown_inside_span_group_ref() {
    let yaml = r#"
file_format: definition/2
spans:
  - type: my.span
    kind: client
    name:
      note: how to name
    brief: t
    stability: stable
    attributes:
      - ref_group: some.group
        typo_in_span_group_ref: bad
"#;
    assert_warns_about(yaml, "typo_in_span_group_ref");
}

// ---- SpanAttributeRef ----

#[test]
fn definition_v2_warns_unknown_inside_span_attribute_ref() {
    let yaml = r#"
file_format: definition/2
spans:
  - type: my.span
    kind: client
    name:
      note: how to name
    brief: t
    stability: stable
    attributes:
      - ref: some.attr
        typo_in_span_attribute_ref: bad
"#;
    assert_warns_about(yaml, "typo_in_span_attribute_ref");
}

// ---- InternalAttributeGroup ----

#[test]
fn definition_v2_warns_unknown_inside_internal_attribute_group() {
    let yaml = r#"
file_format: definition/2
attribute_groups:
  - id: my.group
    visibility: internal
    typo_in_internal_attr_group: bad
"#;
    assert_warns_about(yaml, "typo_in_internal_attr_group");
}

// ---- PublicAttributeGroup ----

#[test]
fn definition_v2_warns_unknown_inside_public_attribute_group() {
    let yaml = r#"
file_format: definition/2
attribute_groups:
  - id: my.group
    visibility: public
    brief: t
    stability: stable
    typo_in_public_attr_group: bad
"#;
    assert_warns_about(yaml, "typo_in_public_attr_group");
}

// ---- Imports (shared with v1) ----

#[test]
fn definition_v2_warns_unknown_inside_imports() {
    let yaml = r#"
file_format: definition/2
imports:
  metrics:
    - foo.*
  typo_in_imports: bad
"#;
    assert_warns_about(yaml, "typo_in_imports");
}

// ---- Deprecated (shared with v1) ----

#[test]
fn definition_v2_warns_unknown_inside_deprecated_renamed() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    deprecated:
      reason: renamed
      renamed_to: my.attr.new
      note: gone
      typo_in_deprecated_renamed: bad
"#;
    assert_warns_about(yaml, "typo_in_deprecated_renamed");
}

#[test]
fn definition_v2_warns_unknown_inside_deprecated_obsoleted() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    deprecated:
      reason: obsoleted
      note: gone
      typo_in_deprecated_obsoleted: bad
"#;
    assert_warns_about(yaml, "typo_in_deprecated_obsoleted");
}

#[test]
fn definition_v2_warns_unknown_inside_deprecated_uncategorized() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    deprecated:
      reason: uncategorized
      note: gone
      typo_in_deprecated_uncategorized: bad
"#;
    assert_warns_about(yaml, "typo_in_deprecated_uncategorized");
}

// `Deprecated::Unspecified` has no input form — the deserializer rejects it.

// ---- EnumEntriesSpec (shared with v1) ----

#[test]
fn definition_v2_warns_unknown_inside_enum_member() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type:
      members:
        - id: ok
          value: ok
          typo_in_enum_member: bad
    brief: t
    stability: stable
"#;
    assert_warns_about(yaml, "typo_in_enum_member");
}

// ---- AttributeType enum-object form (shared with v1) ----

#[test]
fn definition_v2_warns_unknown_inside_attribute_type_enum() {
    let yaml = r#"
file_format: definition/2
attributes:
  - key: my.attr
    type:
      members:
        - id: ok
          value: ok
      typo_in_attribute_type_enum: bad
    brief: t
    stability: stable
"#;
    assert_warns_about(yaml, "typo_in_attribute_type_enum");
}

// ---- RequirementLevel object variants (shared with v1) ----

#[test]
fn definition_v2_warns_unknown_inside_requirement_level_conditionally_required() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    attributes:
      - ref: some.attr
        requirement_level:
          conditionally_required: when foo
          typo_in_req_level_conditionally_required: bad
"#;
    assert_warns_about(yaml, "typo_in_req_level_conditionally_required");
}

#[test]
fn definition_v2_warns_unknown_inside_requirement_level_recommended() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    attributes:
      - ref: some.attr
        requirement_level:
          recommended: do it
          typo_in_req_level_recommended: bad
"#;
    assert_warns_about(yaml, "typo_in_req_level_recommended");
}

#[test]
fn definition_v2_warns_unknown_inside_requirement_level_opt_in() {
    let yaml = r#"
file_format: definition/2
events:
  - name: my.event
    brief: t
    stability: stable
    attributes:
      - ref: some.attr
        requirement_level:
          opt_in: maybe
          typo_in_req_level_opt_in: bad
"#;
    assert_warns_about(yaml, "typo_in_req_level_opt_in");
}

// ---- definition/2.<minor>: known → fatal, newer → silent (mirrors manifest/resolved). ----

#[test]
fn definition_v2_known_minor_rejects_unknown_fatal() {
    let yaml = r#"
file_format: definition/2.0
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    typo_in_attr_def: bad
"#;
    match load(yaml) {
        WResult::FatalErr(Error::UnexpectedFields {
            fields,
            file_format,
            ..
        }) => {
            assert_eq!(file_format.to_string(), "definition/2.0");
            assert!(
                fields.iter().any(|f| f == "attributes[0].typo_in_attr_def"),
                "expected typo in fields list, got: {fields:?}"
            );
        }
        WResult::FatalErr(e) => panic!("expected UnexpectedFields, got fatal: {e:?}"),
        WResult::Ok(_) => panic!("expected fatal UnexpectedFields, got Ok"),
        WResult::OkWithNFEs(_, ws) => {
            panic!("expected fatal UnexpectedFields, got OkWithNFEs: {ws:?}")
        }
    }
}

#[test]
fn definition_v2_newer_minor_silently_tolerates_unknown() {
    let yaml = r#"
file_format: definition/2.99
attributes:
  - key: my.attr
    type: string
    brief: t
    stability: stable
    future_field: x
"#;
    match load(yaml) {
        WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {}
        WResult::FatalErr(e) => panic!("newer minor should tolerate unknowns, got fatal: {e:?}"),
    }
}

// ---- False-positive guard ----

#[test]
fn definition_v2_does_not_warn_for_explicit_default_values() {
    // Explicit defaults on `skip_serializing_if` fields must not be flagged.
    let yaml = r#"
file_format: definition/2
spans:
  - type: my.span
    kind: client
    name:
      note: how to name
    brief: t
    stability: stable
    attributes: []
    entity_associations: []
    note: ""
"#;
    let result = load(yaml);
    let unknown_warnings: Vec<String> = match &result {
        WResult::Ok(_) => vec![],
        WResult::OkWithNFEs(_, ws) => ws
            .iter()
            .map(|w| w.to_string())
            .filter(|s| s.contains("Unexpected fields") || s.contains("unknown"))
            .collect(),
        WResult::FatalErr(e) => panic!("expected non-fatal load, got fatal: {e:?}"),
    };
    assert!(
        unknown_warnings.is_empty(),
        "explicit defaults should not produce unknown-field warnings, got: {unknown_warnings:?}"
    );
}
