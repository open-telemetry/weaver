// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define entities going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    v2::{
        attribute::{AttributeRef, Examples, RequirementLevel},
        signal_id::SignalId,
        signal_requirement_level::SignalRequirementLevel,
        stability::Stability,
        CommonFields,
    },
    YamlValue,
};

/// A refinement of an Attribute for an entity's identity.
///
/// Identity attributes are always required and do not accept a `requirement_level`.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct IdentityAttributeRef {
    /// Reference an existing attribute by key.
    pub r#ref: String,
    /// Refines the brief description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refined sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Deprecated: Identity attributes are always required and should not specify `requirement_level`.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub requirement_level: Option<RequirementLevel>,
    /// Refines the more elaborate description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Additional annotations for the attribute. These will be
    /// merged with annotations from the definition.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

/// Defines a new entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The type of the Entity.
    pub r#type: SignalId,
    /// The attributes that make the identity of the Entity.
    pub identity: Vec<IdentityAttributeRef>,
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
    ///
    /// A refinement must not change *which* attributes identify the entity: it
    /// may only refine attributes the base entity already lists under
    /// `identity`.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identity: Vec<IdentityAttributeRef>,
    /// Refinements or additional attributes to describe the Entity.
    ///
    /// Attributes listed here have the descriptive role.
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

    #[test]
    fn test_entity_identity_accepts_deprecated_requirement_level() {
        let entity_yaml = r#"type: my_entity
identity:
  - ref: some_attr
    requirement_level: required
brief: Test entity
stability: stable
"#;
        let entity = serde_yaml::from_str::<Entity>(entity_yaml)
            .expect("requirement_level on entity identity should deserialize for warning emission");
        assert!(entity.identity[0].requirement_level.is_some());
        let serialized = serde_yaml::to_string(&entity).expect("Failed to serialize entity");
        assert!(!serialized.contains("requirement_level"));

        let refinement_yaml = r#"id: my_entity.refined
ref: my_entity
identity:
  - ref: some_attr
    requirement_level: required
"#;
        let refinement = serde_yaml::from_str::<EntityRefinement>(refinement_yaml).expect(
            "requirement_level on entity refinement identity should deserialize for warning emission",
        );
        assert!(refinement.identity[0].requirement_level.is_some());
        let serialized_ref =
            serde_yaml::to_string(&refinement).expect("Failed to serialize entity refinement");
        assert!(!serialized_ref.contains("requirement_level"));
    }
}
