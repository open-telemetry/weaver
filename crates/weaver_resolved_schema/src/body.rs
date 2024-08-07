// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved `BodyField`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::attribute::{AttributeType, Examples, RequirementLevel};
use weaver_semconv::body::{BodyFieldSpec, BodySpec};
use weaver_semconv::stability::Stability;

/// A `Body` definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// The body specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<BodyField>>,
    // Not yet defined in the spec or implemented in the resolver
    // The body value when there are no fields
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub value: Option<Value>
}

/// A `BodyField` definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyField {
    /// Field name.
    pub name: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: AttributeType,
    /// A brief description of the field.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// Sequence of example values for the field or single example
    /// value. They are required only for string and string array
    /// fields. Example values must be of the same type of the
    /// field. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Specifies if the field is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the field is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the field is required.
    pub requirement_level: RequirementLevel,
    /// A more elaborate description of the field.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the field.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the field is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

/// An unresolved body definition.
#[derive(Debug, Deserialize, Clone)]
pub struct UnresolvedBody {
    /// The body specification.
    pub spec: BodySpec,
}

/// An unresolved body field definition.
#[derive(Debug, Deserialize, Clone)]
pub struct UnresolvedBodyField {
    /// The body field specification.
    pub spec: BodyFieldSpec,
}
