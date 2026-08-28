//! Attribute definitions for resolved schema.

use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::v2::{
    attribute::{AttributeType, Examples},
    CommonFields,
};

use crate::v2::{provenance::Provenance, Signal};

/// The definition of an Attribute.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Hash, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// The provenance of the attribute.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// Reference to an attribute in the catalog.
#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, JsonSchema, Hash,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AttributeRef(pub u32);

impl Display for AttributeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AttributeRef({})", self.0)
    }
}

impl Signal for Attribute {
    fn id(&self) -> &str {
        &self.key
    }

    fn common(&self) -> &CommonFields {
        &self.common
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_semconv::v2::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};

    #[test]
    fn test_attribute_and_ref() {
        let attr_ref = AttributeRef(12);
        assert_eq!(attr_ref.to_string(), "AttributeRef(12)");

        let attr = Attribute {
            key: "service.name".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: CommonFields {
                brief: "Service name".to_owned(),
                note: "".to_owned(),
                stability: Default::default(),
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        };

        assert_eq!(attr.id(), "service.name");
        assert_eq!(attr.common().brief, "Service name");
    }
}
