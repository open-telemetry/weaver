// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define data going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    group::GroupSpec,
    semconv::{Imports, SemConvSpecV1},
    stability::Stability,
    v2::{
        attribute::AttributeDef, entity::EntityGroup, event::EventGroup, metric::MetricGroup,
        span::SpanGroup,
    },
    YamlValue,
};

pub mod attribute;
pub mod entity;
pub mod event;
pub mod metric;
pub mod span;

/// Common fields we want on all major components of semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommonFields {
    /// A brief description of the span.
    pub brief: String,
    /// A more elaborate description of the span.
    /// It defaults to an empty string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Specifies the stability of the span.
    pub stability: Stability,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as description MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub deprecated: Option<Deprecated>,
    /// Annotations for the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, YamlValue>>,
}

/// A semantic convention file as defined [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
/// A semconv file is a collection of semantic convention groups (i.e. [`GroupSpec`]).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpecV2 {
    /// A collection of semantic conventions for attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attributes: Vec<AttributeDef>,
    /// A collection of semantic conventions for Entity signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entities: Vec<EntityGroup>,
    /// A collection of semantic conventions for Event signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) events: Vec<EventGroup>,
    /// A collection of semantic conventions for Metric signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metrics: Vec<MetricGroup>,
    /// A collection of semantic conventions for Span signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) spans: Vec<SpanGroup>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

impl SemConvSpecV2 {
    /// Converts the version 2 schema into the version 1 group spec.
    pub(crate) fn into_v1_specification(self, attribute_group_name: &str) -> SemConvSpecV1 {
        SemConvSpecV1 {
            groups: vec![GroupSpec {
                id: format!("registry.{attribute_group_name}"),
                r#type: crate::group::GroupType::AttributeGroup,
                attributes: self
                    .attributes
                    .into_iter()
                    .map(|a| a.into_v1_attribute())
                    .collect(),
                brief: "<synthetic v2>".to_owned(),
                ..Default::default()
            }]
            .into_iter()
            .chain(self.entities.into_iter().map(|e| e.into_v1_group()))
            .chain(self.events.into_iter().map(|e| e.into_v1_group()))
            .chain(self.metrics.into_iter().map(|m| m.into_v1_group()))
            .chain(self.spans.into_iter().map(|s| s.into_v1_group()))
            .collect(),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
metrics:
  - name: my_metric
    brief: Test metric
    stability: stable
    instrument: histogram
    unit: s
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
    body:
      id: body
      type: string
      requirement_level: required
    brief: Test event
    stability: stable
spans:
  - type: my_span
    name: "{some} {name}"
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
  body:
    id: body
    type: string
    requirement_level: required
- id: metric.my_metric
  type: metric
  metric_name: my_metric
  brief: Test metric
  stability: stable
  instrument: histogram
  unit: s
- id: span.my_span
  type: span
  brief: Test span
  name: "{some} {name}"
  span_kind: client
  stability: stable
imports:
  metrics:
    - foo/*
"#,
        );
    }
}
