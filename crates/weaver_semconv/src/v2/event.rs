// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define events going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    group::GroupSpec,
    stability::Stability,
    v2::{
        attribute::{split_attributes_and_groups, AttributeOrGroupRef},
        signal_id::SignalId,
        CommonFields,
    },
    YamlValue,
};

/// Defines a new event.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The name of the event.
    pub name: SignalId,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this event should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing event.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the event being refined.
    pub r#ref: SignalId,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this event should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,

    /// Refines the brief description of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the signal is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the signal.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

impl Event {
    /// Converts a v2 event into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let (attribute_refs, include_groups) = split_attributes_and_groups(self.attributes);
        GroupSpec {
            id: format!("event.{}", &self.name),
            r#type: crate::group::GroupType::Event,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            include_groups,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: attribute_refs,
            span_kind: None,
            events: Default::default(),
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.name.into_v1()),
            display_name: None,
            body: None,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            entity_associations: self.entity_associations,
            visibility: None,
        }
    }
}

impl EventRefinement {
    /// Converts a v2 event refinement into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let (attribute_refs, include_groups) = split_attributes_and_groups(self.attributes);
        GroupSpec {
            id: self.id.to_string(),
            r#type: crate::group::GroupType::Event,
            brief: self.brief.unwrap_or_default(),
            note: self.note.unwrap_or_default(),
            prefix: Default::default(),
            extends: Some(format!("event.{}", &self.r#ref)),
            include_groups,
            stability: self.stability,
            deprecated: self.deprecated,
            attributes: attribute_refs,
            span_kind: None,
            events: Default::default(),
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.id.into_v1()),
            display_name: None,
            body: None,
            annotations: if self.annotations.is_empty() {
                None
            } else {
                Some(self.annotations)
            },
            entity_associations: self.entity_associations,
            visibility: None,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let event = serde_yaml::from_str::<Event>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, event.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Event
            r#"name: my_event
brief: Test event
stability: stable
"#,
            // V1 - Group
            r#"id: event.my_event
type: event
name: my_event
brief: Test event
stability: stable
"#,
        );
    }
}
