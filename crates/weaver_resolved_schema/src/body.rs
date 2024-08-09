// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved body field.

use crate::attribute::AttributeRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::attribute::{AttributeType, Examples, RequirementLevel};
use weaver_semconv::body::{BodyFieldSpec, BodySpec};
use weaver_semconv::stability::Stability;

/// An attribute definition.
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

/// An attribute definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyField {
    /// Attribute name.
    pub name: String,
    /// A reference to an attribute definition, used to populate the relevant
    /// fields of the body field, unless they are overridden by the body field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#attr: Option<AttributeRef>,
    /// A alias to use for the field for the referenced attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
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
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
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
