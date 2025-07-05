// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define events going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    any_value::AnyValueSpec,
    group::GroupSpec,
    v2::{attribute::AttributeRef, CommonFields},
};

/// A MetricGroup defines a new metric.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventGroup {
    /// The name of the event.
    pub name: String,
    /// The event body definition
    pub body: AnyValueSpec,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeRef>,
    /// Which resources this event should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Common fields (like brief, note, attributes).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl EventGroup {
    /// Converts a v2 event into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        GroupSpec {
            id: format!("event.{}", &self.name),
            r#type: crate::group::GroupType::Event,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| a.into_v1_attribute())
                .collect(),
            span_kind: None,
            events: Default::default(),
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.name),
            display_name: None,
            body: Some(self.body),
            annotations: self.common.annotations,
            entity_associations: self.entity_associations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let event = serde_yaml::from_str::<EventGroup>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, event.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Event
            r#"name: my_event
body:
  id: body
  type: string
  requirement_level: required
brief: Test event
stability: stable
"#,
            // V1 - Group
            r#"id: event.my_event
type: event
name: my_event
brief: Test event
stability: stable
body:
  id: body
  type: string
  requirement_level: required
"#,
        );
    }
}
