// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define events going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    entity_association::EntityAssociation,
    v2::{
        attribute::AttributeOrGroupRef, signal_id::SignalId,
        signal_requirement_level::SignalRequirementLevel, stability::Stability, CommonFields,
    },
    YamlValue,
};

/// Defines a new event.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The name of the event.
    pub name: SignalId,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this event should be associated with.
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,
    /// The requirement level of the event. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing event.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
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
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_parsing() {
        let yaml = r#"name: my_event
brief: Test event
stability: stable
"#;
        let event = serde_yaml::from_str::<Event>(yaml).expect("Failed to parse YAML string");
        assert_eq!(event.name.to_string(), "my_event");
    }
}
