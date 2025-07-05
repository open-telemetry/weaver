// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attributes going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Not;

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

    // TODO - Simplify the options below for "override" / "refine" focus.
    /// A brief description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    brief: Option<String>,
    /// Sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    examples: Option<Examples>,
    /// Associates a tag ("sub-group") to the attribute. It carries no
    /// particular semantic meaning but can be used e.g. for filtering
    /// in the markdown generator.
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    requirement_level: Option<RequirementLevel>,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    /// Note: this field is experimental.
    #[serde(skip_serializing_if = "Option::is_none")]
    sampling_relevant: Option<bool>,
    /// A more elaborate description of the attribute.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    /// Specifies the stability of the attribute.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    stability: Option<Stability>,
    /// Specifies if the attribute is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        deserialize_with = "crate::deprecated::deserialize_option_deprecated",
        default
    )]
    deprecated: Option<Deprecated>,
    /// Specifies the prefix of the attribute.
    /// If this parameter is set, the resolved id of the referenced attribute will
    /// have group prefix added to it.
    /// It defaults to false.
    #[serde(default)]
    #[serde(skip_serializing_if = "<&bool>::not")]
    prefix: bool,
    /// Annotations for the attribute.
    annotations: Option<BTreeMap<String, YamlValue>>,
}

impl AttributeRef {
    /// Converts a v2 refinement into a v1 AttributeSpec.
    #[must_use]
    pub fn into_v1_attribute(self) -> AttributeSpec {
        AttributeSpec::Ref {
            r#ref: self.r#ref,
            brief: self.brief,
            examples: self.examples,
            tag: self.tag,
            requirement_level: self.requirement_level,
            sampling_relevant: self.sampling_relevant,
            note: self.note,
            stability: self.stability,
            deprecated: self.deprecated,
            prefix: self.prefix,
            annotations: self.annotations,
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
            tag: self.tag,
            requirement_level: self.requirement_level,
            sampling_relevant: self.sampling_relevant,
            note: self.note,
            stability: self.stability,
            deprecated: self.deprecated,
            prefix: self.prefix,
            annotations: self.annotations,
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
    key: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    r#type: AttributeType,
    /// Sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    examples: Option<Examples>,
    /// Associates a tag ("sub-group") to the attribute. It carries no
    /// particular semantic meaning but can be used e.g. for filtering
    /// in the markdown generator.
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
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
            tag: self.tag,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: self.common.note,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            annotations: self.common.annotations,
            role: None,
        }
    }
}
