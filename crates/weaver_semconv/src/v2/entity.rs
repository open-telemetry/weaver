// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define entities going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    group::GroupSpec,
    stability::Stability,
    v2::{attribute::AttributeRef, signal_id::SignalId, CommonFields},
    YamlValue,
};

/// Defines a new entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the entity being refined.
    pub r#ref: SignalId,
    /// The additionaly attributes to describe of the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<AttributeRef>,
    /// Refines the brief description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the signal is deprecated.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

impl Entity {
    /// Converts a v2 entity into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let attributes = self
            .identity
            .into_iter()
            .map(|a| a.into_v1_attribute_with_role(crate::attribute::AttributeRole::Identifying))
            .chain(self.description.into_iter().map(|a| {
                a.into_v1_attribute_with_role(crate::attribute::AttributeRole::Descriptive)
            }))
            .collect();

        GroupSpec {
            id: format!("entity.{}", &self.r#type),
            r#type: crate::group::GroupType::Entity,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            include_groups: vec![],
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes,
            span_kind: None,
            events: Default::default(),
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.r#type.into_v1()),
            display_name: None,
            body: None,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            entity_associations: Default::default(),
            visibility: None,
        }
    }
}

impl EntityRefinement {
    /// Converts a v2 entity refinement into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let attributes = self
            .description
            .into_iter()
            .map(|a| a.into_v1_attribute_with_role(crate::attribute::AttributeRole::Descriptive))
            .collect();

        GroupSpec {
            id: self.id.to_string(),
            r#type: crate::group::GroupType::Entity,
            brief: self.brief.unwrap_or_default(),
            note: self.note.unwrap_or_default(),
            prefix: Default::default(),
            extends: Some(format!("entity.{}", &self.r#ref)),
            include_groups: vec![],
            stability: self.stability,
            deprecated: self.deprecated,
            attributes,
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
            entity_associations: Default::default(),
            visibility: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let entity = serde_yaml::from_str::<Entity>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, entity.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Entity
            r#"type: my_entity
identity:
  - ref: some_attr
description:
  - ref: some_other_attr
brief: Test entity
stability: stable
"#,
            // V1 - Group
            r#"id: entity.my_entity
type: entity
name: my_entity
brief: Test entity
stability: stable
attributes:
  - ref: some_attr
    role: identifying
  - ref: some_other_attr
    role: descriptive
"#,
        );
    }
}
