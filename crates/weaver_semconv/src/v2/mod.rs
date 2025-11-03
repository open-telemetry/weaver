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
        attribute::AttributeDef, attribute_group::AttributeGroup, entity::Entity, event::Event,
        metric::Metric, span::Span,
    },
    YamlValue,
};

pub mod attribute;
pub mod attribute_group;
pub mod entity;
pub mod event;
pub mod metric;
pub mod signal_id;
pub mod span;

/// Common fields we want on all major components of semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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

/// A semconv file is a collection of semantic convention groups (i.e. [`GroupSpec`]).
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

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

impl SemConvSpecV2 {
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
- id: metric.my_metric
  type: metric
  metric_name: my_metric
  brief: Test metric
  stability: stable
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
- id: test
  type: attribute_group
  brief: test
  attributes:
  - ref: test.attribute
  visibility: internal
imports:
  metrics:
  - foo/*
"#,
        );
    }
}
