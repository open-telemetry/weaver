// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved `AnyValue`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::attribute::{EnumEntriesSpec, Examples, RequirementLevel};
use weaver_semconv::stability::Stability;

/// An `AnyValue` definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnyValue {
    /// AnyValue name.
    pub name: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: String,
    /// A description of the type of the AnyValue
    /// e.g. "string", "string[]", "int", "enum<enum_id>", "map<id>{ int, string }"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_display: Option<String>,
    /// A brief description of the AnyValue.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub brief: String,
    /// Sequence of example values for the AnyValue or single example
    /// value. They are required only for primitive and primitive array
    /// values. Example values must be of the same type of the type.
    /// If only a single example is provided, it can directly be reported
    /// without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Specifies if the value is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the value is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the value is required.
    pub requirement_level: RequirementLevel,
    /// A more elaborate description of the any value.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the any value.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the value is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Identifies the definition of the "fields" of the value when the type is "map".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<AnyValue>>,
    /// Used when the type is "enum".
    /// Set to false to not accept values other than the specified members.
    /// It defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_custom_values: Option<bool>,
    /// Used when the type is "enum".
    /// List of enum entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<EnumEntriesSpec>>,
}
