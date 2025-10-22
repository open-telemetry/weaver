// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attributes going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    attribute::{AttributeRole, AttributeSpec, AttributeType, Examples, RequirementLevel},
    deprecated::Deprecated,
    stability::Stability,
    v2::CommonFields,
    YamlValue,
};

/// A refinement of an Attribute for a signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeRef {
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
    /// Refines the attribute requirement level. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the original attribute requirement level is used. When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<RequirementLevel>,
    /// Refines the more elaborate description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the attribute.
    /// This denotes whether an attribute is stable for a specific
    /// signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the attribute is deprecated for this signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the attribute. These will be
    /// merged with annotations from the definition.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

impl AttributeRef {
    /// Converts a v2 refinement into a v1 AttributeSpec.
    #[must_use]
    pub fn into_v1_attribute(self) -> AttributeSpec {
        AttributeSpec::Ref {
            r#ref: self.r#ref,
            brief: self.brief,
            examples: self.examples,
            tag: None,
            requirement_level: self.requirement_level,
            sampling_relevant: None,
            note: self.note,
            stability: self.stability,
            deprecated: self.deprecated,
            prefix: false,
            annotations: if self.annotations.is_empty() {
                None
            } else {
                Some(self.annotations)
            },
            role: None,
        }
    }
    /// Converts a v2 refinement into a v1 AttributeSpec.
    #[must_use]
    pub fn into_v1_attribute_with_role(self, role: AttributeRole) -> AttributeSpec {
        AttributeSpec::Ref {
            r#ref: self.r#ref,
            brief: self.brief,
            examples: self.examples,
            tag: None,
            requirement_level: self.requirement_level,
            sampling_relevant: None,
            note: self.note,
            stability: self.stability,
            deprecated: self.deprecated,
            prefix: false,
            annotations: if self.annotations.is_empty() {
                None
            } else {
                Some(self.annotations)
            },
            role: Some(role),
        }
    }
}

/// The definition of an Attribute.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeDef {
    /// String that uniquely identifies the attribute.
    pub key: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: AttributeType,
    /// Sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Common fields (like brief, note, attributes).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl AttributeDef {
    /// Converts a v2 refinement into a v1 AttributeSpec.
    #[must_use]
    pub fn into_v1_attribute(self) -> AttributeSpec {
        AttributeSpec::Id {
            id: self.key,
            r#type: self.r#type,
            brief: Some(self.common.brief),
            examples: self.examples,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: self.common.note,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            role: None,
        }
    }
}

/// A reference to an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: String,
}

/// A reference to either an attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum AttributeOrGroupRef {
    /// Reference to an attribute.
    Attribute(AttributeRef),
    /// Reference to an attribute group.
    Group(GroupRef),
}

/// Helper function to split a vector of AttributeOrGroupRef into separate vectors
/// of AttributeSpec and group reference strings
#[must_use]
pub fn split_attributes_and_groups(
    attributes_and_groups: Vec<AttributeOrGroupRef>,
) -> (Vec<AttributeSpec>, Vec<String>) {
    let mut attributes = Vec::new();
    let mut groups = Vec::new();

    for item in attributes_and_groups {
        match item {
            AttributeOrGroupRef::Attribute(attr_ref) => {
                attributes.push(attr_ref.into_v1_attribute());
            }
            AttributeOrGroupRef::Group(group_ref) => groups.push(group_ref.ref_group),
        }
    }

    (attributes, groups)
}
