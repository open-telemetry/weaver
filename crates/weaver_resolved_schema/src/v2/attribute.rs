//! Attribute definitions for resolved schema.

use std::{collections::HashMap, fmt::Display};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::{self, AttributeType, Examples},
    v2::CommonFields,
};

/// The definition of an Attribute.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Hash, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct Attribute {
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

/// Reference to an attribute in the catalog.
#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, JsonSchema, Hash,
)]
pub struct AttributeRef(pub u32);

impl Display for AttributeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AttributeRef({})", self.0)
    }
}
