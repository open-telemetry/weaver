// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attribute groups going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::{GroupSpec, GroupType},
    v2::{
        attribute::{split_attributes_and_groups, AttributeOrGroupRef},
        signal_id::SignalId,
        CommonFields,
    },
};

/// Internal attribute group implementation
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InternalAttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
}

/// A group defines an attribute group, an entity, or a signal.
/// Mandatory fields is: `id`. Groups are expected to have `attributes`,
/// `include_groups` or both
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(tag = "visibility")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AttributeGroup {
    /// An internal attribute group
    Internal {
        /// The name of the attribute group, must be unique.
        id: SignalId,

        /// List of attributes and group references that belong to this group
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<AttributeOrGroupRef>,
    },
    /// A public attribute group
    Public {
        /// The name of the attribute group, must be unique.
        id: SignalId,

        /// List of attributes and group references that belong to this group
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<AttributeOrGroupRef>,

        /// Common fields (like brief, note, annotations).
        #[serde(flatten)]
        common: CommonFields,
    },
}

impl AttributeGroup {
    /// Converts a v2 attribute group into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        match self {
            AttributeGroup::Internal { id, attributes } => {
                let (attribute_refs, include_groups) = split_attributes_and_groups(attributes);

                GroupSpec {
                    id: format!("{}", &id),
                    r#type: GroupType::AttributeGroup,
                    brief: format!("{}", &id),
                    note: "".to_owned(),
                    prefix: Default::default(),
                    extends: None,
                    include_groups,
                    stability: None,
                    deprecated: None,
                    attributes: attribute_refs,
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: Some(AttributeGroupVisibilitySpec::Internal),
                }
            }
            AttributeGroup::Public {
                id,
                attributes,
                common,
            } => {
                let (attributes, include_groups) = split_attributes_and_groups(attributes);

                GroupSpec {
                    id: format!("{}", id),
                    r#type: GroupType::AttributeGroup,
                    brief: common.brief,
                    note: common.note,
                    prefix: Default::default(),
                    extends: None,
                    include_groups,
                    stability: Some(common.stability),
                    deprecated: common.deprecated,
                    attributes,
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    display_name: None,
                    body: None,
                    annotations: Some(common.annotations),
                    entity_associations: vec![],
                    visibility: Some(AttributeGroupVisibilitySpec::Public),
                }
            }
        }
    }
}

/// The group's visibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
pub enum AttributeGroupVisibilitySpec {
    /// An internal group.
    Internal,
    /// A public group.
    Public,
}

impl Default for AttributeGroupVisibilitySpec {
    fn default() -> Self {
        Self::Internal
    }
}

impl std::fmt::Display for AttributeGroupVisibilitySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeGroupVisibilitySpec::Internal => write!(f, "internal"),
            AttributeGroupVisibilitySpec::Public => write!(f, "public"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let attr_group =
            serde_yaml::from_str::<AttributeGroup>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, attr_group.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Group
            r#"id: my_attr_group
brief: Test group
stability: development
attributes:
"#,
            // V1 - Group
            r#"id: my_attr_group
type: attribute_group
brief: Test group
stability: development
"#,
        );
    }
}
