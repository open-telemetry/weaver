// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define entities going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    signal_requirement_level::SignalRequirementLevel,
    stability::Stability,
    v2::{attribute::AttributeRef, signal_id::SignalId, CommonFields},
    YamlValue,
};

/// Defines a new entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The type of the Entity.
    pub r#type: SignalId,
    /// The attributes that make the identity of the Entity.
    pub identity: Vec<AttributeRef>,
    /// The attributes that make the description of the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<AttributeRef>,
    /// The requirement level of the entity. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EntityRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the entity being refined.
    pub r#ref: SignalId,
    /// Refinements of the base entity's identity attributes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identity: Vec<AttributeRef>,
    /// Refinements or additional attributes to describe the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<AttributeRef>,
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
    fn test_entity_parsing() {
        let yaml = r#"type: my_entity
identity:
  - ref: some_attr
description:
  - ref: some_other_attr
brief: Test entity
stability: stable
"#;
        let entity = serde_yaml::from_str::<Entity>(yaml).expect("Failed to parse YAML string");
        assert_eq!(entity.r#type.to_string(), "my_entity");
        assert_eq!(entity.identity.len(), 1);
        assert_eq!(entity.description.len(), 1);
    }
}
