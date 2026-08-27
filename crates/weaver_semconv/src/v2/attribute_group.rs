// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attribute groups going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::{attribute::AttributeOrGroupRef, signal_id::SignalId, CommonFields};

/// Internal attribute group implementation
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InternalAttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
}

/// Public attribute group implementation
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublicAttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// Attribute group definition.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "visibility")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AttributeGroup {
    /// An internal attribute group
    Internal(InternalAttributeGroup),
    /// A public attribute group
    Public(PublicAttributeGroup),
}

/// The group's visibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttributeGroupVisibilitySpec {
    /// An internal group.
    #[default]
    Internal,
    /// A public group.
    Public,
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

    #[test]
    fn test_attribute_group_parsing() {
        let yaml = r#"id: my_attr_group
brief: Test group
stability: development
visibility: public
"#;
        let attr_group =
            serde_yaml::from_str::<AttributeGroup>(yaml).expect("Failed to parse YAML string");
        match attr_group {
            AttributeGroup::Public(p) => {
                assert_eq!(p.id.to_string(), "my_attr_group");
                assert_eq!(p.common.brief, "Test group");
            }
            AttributeGroup::Internal(_) => panic!("Expected public attribute group"),
        }
    }
}
