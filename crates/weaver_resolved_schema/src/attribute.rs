// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved attribute.

use crate::tags::Tags;
use crate::value::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::ops::Not;
use weaver_semconv::attribute::{AttributeSpec, AttributeType, Examples, RequirementLevel};
use weaver_semconv::stability::Stability;

/// An attribute definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    /// Attribute name.
    pub name: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: AttributeType,
    /// A brief description of the attribute.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// Sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Associates a tag ("sub-group") to the attribute. It carries no
    /// particular semantic meaning but can be used e.g. for filtering
    /// in the markdown generator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    /// Note: this field is experimental.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
    /// A more elaborate description of the attribute.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the attribute.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the attribute is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Specifies the prefix of the attribute.
    /// If this parameter is set, the resolved id of the referenced attribute will
    /// have group prefix added to it.
    /// It defaults to false.
    #[serde(default)]
    #[serde(skip_serializing_if = "<&bool>::not")]
    pub prefix: bool,
    /// A set of tags for the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,

    /// The value of the attribute.
    /// Note: This is only used in a telemetry schema specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

/// An unresolved attribute definition.
#[derive(Debug, Deserialize, Clone)]
pub struct UnresolvedAttribute {
    /// The attribute specification.
    pub spec: AttributeSpec,
}

/// An internal reference to an attribute in the catalog.
#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, JsonSchema,
)]
pub struct AttributeRef(pub u32);

impl Display for AttributeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AttributeRef({})", self.0)
    }
}
