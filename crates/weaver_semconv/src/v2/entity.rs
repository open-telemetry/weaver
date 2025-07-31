// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define entities going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::GroupSpec,
    v2::{attribute::AttributeRef, CommonFields},
};

/// An EntityGroup defines a new entity.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityGroup {
    /// The type of the Entity.
    pub r#type: String,
    /// The attributes that make the identity of the Entity.
    pub identity: Vec<AttributeRef>,
    /// The attributes that make the description of the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<AttributeRef>,
    /// Common fields (like brief, note, attributes).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl EntityGroup {
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
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes,
            span_kind: None,
            events: Default::default(),
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.r#type),
            display_name: None,
            body: None,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            entity_associations: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let entity = serde_yaml::from_str::<EntityGroup>(v2).expect("Failed to parse YAML string");
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
