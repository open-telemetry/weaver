// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define data going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use weaver_common::result::WResult;

use crate::{
    deprecated::Deprecated,
    group::GroupSpec,
    semconv::{Imports, SemConvSpecV1},
    stability::Stability,
    v2::{
        attribute::AttributeDef, attribute::AttributeRef, attribute_group::AttributeGroup,
        entity::Entity, entity::EntityRefinement, event::Event, event::EventRefinement,
        metric::Metric, metric::MetricRefinement, signal_id::SignalId, span::Span,
        span::SpanRefinement,
    },
    Error, YamlValue,
};

pub mod attribute;
pub mod attribute_group;
pub mod entity;
pub mod event;
pub mod metric;
pub mod signal_id;
pub mod span;

/// Common fields we want on all major components of semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Hash, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CommonFields {
    /// A brief description of the attribute or signal.
    pub brief: String,
    /// A more elaborate description of the attribute or signal.
    /// It defaults to an empty string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Specifies the stability of the attribute or signal.
    pub stability: Stability,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as description MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Annotations for the attribute or signal.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

/// A semconv file is a collection of attributes, signals, groups,
/// and imports.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpecV2 {
    /// A collection of semantic conventions for attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attributes: Vec<AttributeDef>,
    /// A collection of semantic conventions for Entity signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entities: Vec<Entity>,
    /// A collection of semantic conventions for Event signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) events: Vec<Event>,
    /// A collection of semantic conventions for Metric signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metrics: Vec<Metric>,
    /// A collection of semantic conventions for Span signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) spans: Vec<Span>,
    /// A collection of semantic conventions for AttributeGroups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attribute_groups: Vec<AttributeGroup>,

    /// A collection of semantic convention refinements for Entity signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entity_refinements: Vec<EntityRefinement>,
    /// A collection of semantic convention refinements for Event signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) event_refinements: Vec<EventRefinement>,
    /// A collection of semantic convention refinements for Metric signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metric_refinements: Vec<MetricRefinement>,
    /// A collection of semantic convention refinements for Span signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) span_refinements: Vec<SpanRefinement>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

impl SemConvSpecV2 {
    /// Creates a v2 semantic convention spec with the main signal sections.
    #[must_use]
    pub fn new(
        attributes: Vec<AttributeDef>,
        entities: Vec<Entity>,
        events: Vec<Event>,
        metrics: Vec<Metric>,
        spans: Vec<Span>,
    ) -> Self {
        Self {
            attributes,
            entities,
            events,
            metrics,
            spans,
            attribute_groups: vec![],
            entity_refinements: vec![],
            event_refinements: vec![],
            metric_refinements: vec![],
            span_refinements: vec![],
            imports: None,
        }
    }

    /// Returns the attribute definitions in this spec.
    #[must_use]
    pub fn attributes(&self) -> &[AttributeDef] {
        &self.attributes
    }

    /// Returns the entity definitions in this spec.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Returns the event definitions in this spec.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Returns the metric definitions in this spec.
    #[must_use]
    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    /// Returns the span definitions in this spec.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Returns the JSON Schema for this type, with `file_format` injected as a
    /// documented-only property. The field is intentionally absent from the Rust
    /// struct (it is stripped before serde deserialization) but must appear in
    /// the schema for IDE auto-complete and documentation purposes.
    #[must_use]
    pub fn output_schema() -> schemars::Schema {
        let mut schema =
            serde_json::to_value(schemars::schema_for!(Self)).expect("Failed to serialize schema");
        if let Some(props) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
            let _ = props.insert(
                "file_format".to_owned(),
                serde_json::json!({
                    "description": "The file format version.",
                    "type": "string",
                    "const": "definition/2"
                }),
            );
        }
        serde_json::from_value(schema).expect("Failed to deserialize schema")
    }

    /// Validates invariants on the v2 model.
    ///
    /// This checks that every signal (metric, span, event, entity) declares a
    /// `requirement_level`. A missing requirement level is a non-fatal warning
    /// that is elevated to an error under `--future`.
    ///
    /// It also rejects entities and entity refinements that list the same
    /// attribute under both `identity` and `description` — the two lists
    /// assign contradicting roles, so this is a fatal definition error.
    pub(crate) fn validate(self, provenance: &str) -> WResult<Self, Error> {
        let mut errors: Vec<Error> = vec![];
        let mut fatal_errors: Vec<Error> = vec![];

        let mut check = |missing: bool, group_id: String| {
            if missing {
                errors.push(Error::MissingRequirementLevelWarning {
                    path_or_url: provenance.to_owned(),
                    group_id,
                });
            }
        };

        for m in &self.metrics {
            check(m.requirement_level.is_none(), format!("metric.{}", m.name));
        }
        for s in &self.spans {
            check(s.requirement_level.is_none(), format!("span.{}", s.r#type));
        }
        for e in &self.events {
            check(e.requirement_level.is_none(), format!("event.{}", e.name));
        }
        for e in &self.entities {
            check(
                e.requirement_level.is_none(),
                format!("entity.{}", e.r#type),
            );
        }

        let mut check_identity_overlap =
            |identity: &[AttributeRef], description: &[AttributeRef], group_id: &SignalId| {
                for attr in description {
                    if identity.iter().any(|i| i.r#ref == attr.r#ref) {
                        fatal_errors.push(Error::AttributeInIdentityAndDescription {
                            path_or_url: provenance.to_owned(),
                            group_id: group_id.to_string(),
                            attribute_id: attr.r#ref.clone(),
                        });
                    }
                }
            };
        for e in &self.entities {
            check_identity_overlap(&e.identity, &e.description, &e.r#type);
        }
        for r in &self.entity_refinements {
            check_identity_overlap(&r.identity, &r.description, &r.id);
        }

        if !fatal_errors.is_empty() {
            return WResult::FatalErr(Error::CompoundError(fatal_errors));
        }
        WResult::with_non_fatal_errors(self, errors)
    }

    /// Converts the version 2 schema into the version 1 group spec.
    pub(crate) fn into_v1_specification(self, file_name: &str) -> SemConvSpecV1 {
        log::debug!("Translating v2 spec into v1 spec for {file_name}");

        let mut groups = Vec::new();

        // Only create synthetic attribute group if there are attribute definitions
        if !self.attributes.is_empty() {
            groups.push(GroupSpec {
                id: format!("registry.{file_name}"),
                r#type: crate::group::GroupType::AttributeGroup,
                attributes: self
                    .attributes
                    .into_iter()
                    .map(|a| a.into_v1_attribute())
                    .collect(),
                brief: "<synthetic v2>".to_owned(),
                is_v2: true,
                span_name: None,
                ..Default::default()
            });
        }

        // Add all other groups
        groups.extend(self.entities.into_iter().map(|e| e.into_v1_group()));
        groups.extend(self.events.into_iter().map(|e| e.into_v1_group()));
        groups.extend(self.metrics.into_iter().map(|m| m.into_v1_group()));
        groups.extend(self.spans.into_iter().map(|s| s.into_v1_group()));
        groups.extend(
            self.attribute_groups
                .into_iter()
                .map(|ag| ag.into_v1_group()),
        );

        // Add all refinements
        groups.extend(
            self.entity_refinements
                .into_iter()
                .map(|e| e.into_v1_group()),
        );
        groups.extend(
            self.event_refinements
                .into_iter()
                .map(|e| e.into_v1_group()),
        );
        groups.extend(
            self.metric_refinements
                .into_iter()
                .map(|m| m.into_v1_group()),
        );
        groups.extend(self.span_refinements.into_iter().map(|s| s.into_v1_group()));

        SemConvSpecV1 {
            groups,
            imports: self.imports,
        }
    }
    /// True if this specification holds no definitions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
            && self.entities.is_empty()
            && self.events.is_empty()
            && self.metrics.is_empty()
            && self.spans.is_empty()
            && self.attribute_groups.is_empty()
            && self.entity_refinements.is_empty()
            && self.event_refinements.is_empty()
            && self.metric_refinements.is_empty()
            && self.span_refinements.is_empty()
    }
}

impl Default for CommonFields {
    fn default() -> Self {
        Self {
            brief: Default::default(),
            note: Default::default(),
            stability: Stability::Alpha,
            deprecated: Default::default(),
            annotations: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_yaml(v2: &str) -> Vec<Error> {
        let spec = serde_yaml::from_str::<SemConvSpecV2>(v2).expect("Failed to parse YAML string");
        match spec.validate("<test>") {
            WResult::Ok(_) => vec![],
            WResult::OkWithNFEs(_, nfes) => nfes,
            WResult::FatalErr(e) => vec![e],
        }
    }

    #[test]
    fn test_v2_missing_requirement_level_warns() {
        // Each signal type missing requirement_level produces a warning.
        for (signal, yaml) in [
            (
                "metric.my_metric",
                "metrics:\n  - name: my_metric\n    brief: b\n    stability: stable\n    instrument: counter\n    unit: \"1\"\n",
            ),
            (
                "span.my_span",
                "spans:\n  - type: my_span\n    brief: b\n    stability: stable\n    kind: client\n    name:\n      note: n\n",
            ),
            (
                "event.my_event",
                "events:\n  - name: my_event\n    brief: b\n    stability: stable\n",
            ),
            (
                "entity.my_entity",
                "entities:\n  - type: my_entity\n    brief: b\n    stability: stable\n    identity:\n      - ref: some.attr\n",
            ),
        ] {
            let errors = validate_yaml(yaml);
            assert!(
                errors.iter().any(|e| matches!(
                    e,
                    Error::MissingRequirementLevelWarning { group_id, .. } if group_id == signal
                )),
                "expected MissingRequirementLevelWarning for `{signal}`, got: {errors:?}"
            );
        }
    }

    #[test]
    fn test_v2_requirement_level_set_no_warning() {
        for level in ["recommended", "opt_in"] {
            let yaml = format!(
                "metrics:\n  - name: my_metric\n    brief: b\n    stability: stable\n    instrument: counter\n    unit: \"1\"\n    requirement_level: {level}\n"
            );
            let errors = validate_yaml(&yaml);
            assert!(
                !errors
                    .iter()
                    .any(|e| matches!(e, Error::MissingRequirementLevelWarning { .. })),
                "did not expect a warning when requirement_level={level}, got: {errors:?}"
            );
        }
    }

    #[test]
    fn test_v2_missing_requirement_level_future_mode() {
        use miette::{Diagnostic, Severity};
        use weaver_common::diagnostic::{
            disable_future_mode, enable_future_mode, DiagnosticMessage,
        };

        let errors =
            validate_yaml("spans:\n  - type: my_span\n    brief: b\n    stability: stable\n    kind: client\n    name:\n      note: n\n");
        let warn = errors.into_iter().next().expect("expected a warning");
        assert_eq!(warn.severity(), Some(Severity::Warning));

        // Under --future the warning is elevated to an error.
        enable_future_mode();
        let diag = DiagnosticMessage::new(warn);
        assert!(!diag.is_warning(), "expected error severity under --future");
        disable_future_mode();
    }

    #[test]
    fn test_v2_attribute_in_identity_and_description_fails() {
        for (group_id, yaml) in [
            (
                "my_entity",
                "entities:\n  - type: my_entity\n    brief: b\n    stability: stable\n    requirement_level: recommended\n    identity:\n      - ref: some.attr\n    description:\n      - ref: some.attr\n",
            ),
            (
                "my_refinement",
                "entity_refinements:\n  - id: my_refinement\n    ref: my_entity\n    identity:\n      - ref: some.attr\n    description:\n      - ref: some.attr\n",
            ),
        ] {
            let errors = validate_yaml(yaml);
            assert!(
                errors.iter().any(|e| matches!(
                    e,
                    Error::CompoundError(inner) if inner.iter().any(|e| matches!(
                        e,
                        Error::AttributeInIdentityAndDescription { group_id: g, attribute_id, .. }
                            if g == group_id && attribute_id == "some.attr"
                    ))
                )),
                "expected AttributeInIdentityAndDescription for `{group_id}`, got: {errors:?}"
            );
        }
    }

    #[test]
    fn test_v2_attribute_in_identity_or_description_only_ok() {
        let errors = validate_yaml(
            "entities:\n  - type: my_entity\n    brief: b\n    stability: stable\n    requirement_level: recommended\n    identity:\n      - ref: some.attr\n    description:\n      - ref: other.attr\n",
        );
        assert!(
            !errors
                .iter()
                .any(|e| matches!(e, Error::AttributeInIdentityAndDescription { .. })),
            "did not expect AttributeInIdentityAndDescription, got: {errors:?}"
        );
    }

    fn parse_and_translate(v2: &str, v1: &str) {
        let spec = serde_yaml::from_str::<SemConvSpecV2>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<SemConvSpecV1>(v1).expect("Failed to parse expected YAML");
        let result = spec.into_v1_specification("test_attribute_group");
        let result_yaml = serde_yaml::to_string(&result).expect("Unable to write YAML from v1");
        assert_eq!(
            expected, result,
            "Expected yaml\n:{v1}\nFound yaml:\n{result_yaml}"
        );
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Span
            r#"
attributes:
  - key: test.attribute
    type: int
    brief: A test attribute
    stability: stable
attribute_groups:
  - id: test
    visibility: internal
    attributes:
      - ref: test.attribute
metrics:
  - name: my_metric
    brief: Test metric
    stability: stable
    instrument: histogram
    unit: s
    attributes:
      - ref_group: test
entities:
  - type: my_entity
    identity:
      - ref: some_attr
    description:
      - ref: some_other_attr
    brief: Test entity
    stability: stable
events:
  - name: my_event
    brief: Test event
    stability: stable
spans:
  - type: my_span
    name:
      note: "{some} {name}"
    stability: stable
    kind: client
    brief: Test span
imports:
  metrics:
    - foo/*
"#,
            // V1 - Groups
            r#"
groups:
- id: registry.test_attribute_group
  type: attribute_group
  brief: <synthetic v2>
  is_v2: true
  attributes:
  - id: test.attribute
    type: int
    brief: A test attribute
    requirement_level: recommended
    stability: stable
- id: entity.my_entity
  type: entity
  name: my_entity
  brief: Test entity
  stability: stable
  is_v2: true
  attributes:
  - ref: some_attr
    role: identifying
  - ref: some_other_attr
    role: descriptive
- id: event.my_event
  type: event
  name: my_event
  brief: Test event
  stability: stable
  is_v2: true
- id: metric.my_metric
  type: metric
  metric_name: my_metric
  brief: Test metric
  stability: stable
  is_v2: true
  instrument: histogram
  unit: s
  include_groups:
  - test
- id: span.my_span
  type: span
  brief: Test span
  name: my_span
  span_kind: client
  stability: stable
  is_v2: true
  span_name:
    note: "{some} {name}"
- id: test
  type: attribute_group
  brief: test
  is_v2: true
  attributes:
  - ref: test.attribute
  visibility: internal
imports:
  metrics:
  - foo/*
"#,
        );
    }

    #[test]
    fn test_refinements() {
        parse_and_translate(
            r#"
metric_refinements:
  - id: metric.my.refined.metric
    ref: base.metric
    brief: Refined metric brief
span_refinements:
  - id: span.my.refined.span
    ref: base.span
event_refinements:
  - id: event.my.refined.event
    ref: base.event
    brief: Refined event brief
entity_refinements:
  - id: entity.my.refined.entity
    ref: base.entity
    brief: Refined entity brief
"#,
            r#"
groups:
- id: entity.my.refined.entity
  type: entity
  name: entity.my.refined.entity
  brief: Refined entity brief
  extends: entity.base.entity
  is_v2: true
- id: event.my.refined.event
  type: event
  name: event.my.refined.event
  brief: Refined event brief
  extends: event.base.event
  is_v2: true
- id: metric.my.refined.metric
  type: metric
  brief: Refined metric brief
  extends: metric.base.metric
  is_v2: true
- id: span.my.refined.span
  type: span
  brief: ""
  name: span.my.refined.span
  extends: span.base.span
  is_v2: true
"#,
        );
    }

    #[test]
    fn test_semconv_spec_v2_is_empty() {
        let empty_spec = SemConvSpecV2 {
            attributes: vec![],
            entities: vec![],
            events: vec![],
            metrics: vec![],
            spans: vec![],
            attribute_groups: vec![],
            entity_refinements: vec![],
            event_refinements: vec![],
            metric_refinements: vec![],
            span_refinements: vec![],
            imports: None,
        };
        assert!(empty_spec.is_empty());

        let non_empty_spec = SemConvSpecV2 {
            attributes: vec![AttributeDef {
                key: "test".to_owned(),
                r#type: crate::attribute::AttributeType::PrimitiveOrArray(
                    crate::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
            }],
            ..empty_spec.clone()
        };
        assert!(!non_empty_spec.is_empty());
    }

    #[test]
    fn test_output_schema_contains_file_format() {
        let schema = SemConvSpecV2::output_schema();
        let value = serde_json::to_value(&schema).expect("Failed to serialize schema");
        let file_format = value
            .get("properties")
            .and_then(|p| p.get("file_format"))
            .expect("Expected 'file_format' in schema properties");
        assert_eq!(
            file_format.get("const").and_then(|v| v.as_str()),
            Some("definition/2")
        );
    }

    #[test]
    fn test_semconv_spec_v2_constructor_and_accessors() {
        let spec = SemConvSpecV2::new(
            vec![AttributeDef {
                key: "test".to_owned(),
                r#type: crate::attribute::AttributeType::PrimitiveOrArray(
                    crate::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: String::new(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(spec.attributes().len(), 1);
        assert!(spec.entities().is_empty());
        assert!(spec.events().is_empty());
        assert!(spec.metrics().is_empty());
        assert!(spec.spans().is_empty());
        assert!(!spec.is_empty());
    }
}
