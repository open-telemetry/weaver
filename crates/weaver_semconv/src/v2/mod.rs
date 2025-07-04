// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define data going forward.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    group::GroupSpec,
    stability::Stability,
    v2::{entity::EntityGroup, event::EventGroup, metric::MetricGroup, span::SpanGroup},
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
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        deserialize_with = "crate::deprecated::deserialize_option_deprecated",
        default
    )]
    pub deprecated: Option<Deprecated>,
    /// Annotations for the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, YamlValue>>,
}

/// A semantic convention file as defined [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
/// A semconv file is a collection of semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct V2SemconvSpec {
    // TODO - Attributes
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
}

impl V2SemconvSpec {
    /// Converts the version 2 schema into the version 1 group spec.
    pub(crate) fn into_v1_groups(self) -> Vec<GroupSpec> {
        self.entities
            .into_iter()
            .map(|e| e.into_v1_group())
            .chain(self.events.into_iter().map(|e| e.into_v1_group()))
            .chain(self.metrics.into_iter().map(|m| m.into_v1_group()))
            .chain(self.spans.into_iter().map(|s| s.into_v1_group()))
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let spec = serde_yaml::from_str::<V2SemconvSpec>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<Vec<GroupSpec>>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, spec.into_v1_groups());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Span
            r#"
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
"#,
            // V1 - Groups
            r#"
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
"#,
        );
    }
}