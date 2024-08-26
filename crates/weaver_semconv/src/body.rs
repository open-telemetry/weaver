// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Body Field specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::attribute::{AttributeType, BasicRequirementLevelSpec, Examples, RequirementLevel};
use crate::stability::Stability;

/// A body specification
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum BodySpec {
    /// The collection of body fields associated with a body definition
    Fields {
        /// Identifies that the type of the body is a map of fields or a string.
        r#type: BodyType,
        /// A brief description of the body.
        #[serde(skip_serializing_if = "String::is_empty")]
        #[serde(default)]
        brief: String,
        /// A more elaborate description of the body.
        /// It defaults to an empty string.
        #[serde(skip_serializing_if = "String::is_empty")]
        #[serde(default)]
        note: String,
        /// Specifies the stability of the body.
        #[serde(skip_serializing_if = "Option::is_none")]
        stability: Option<Stability>,
        /// Sequence of example values for the body or single example
        /// value. They are required only for string types. Example values
        /// must be of the same type of the body. If only a single example is
        /// provided, it can directly be reported without encapsulating it
        /// into a sequence/dictionary.
        #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<Examples>,
        /// Identifies the definition of the "fields" of the body when the body type is "map".
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fields: Vec<BodyFieldSpec>,
    },
    /// The body will just be a string.
    String {
        /// Identifies that the type of the body is a string.
        r#type: BodyType,
        /// A brief description of the body.
        #[serde(skip_serializing_if = "String::is_empty")]
        #[serde(default)]
        brief: String,
        /// A more elaborate description of the body.
        /// It defaults to an empty string.
        #[serde(skip_serializing_if = "String::is_empty")]
        #[serde(default)]
        note: String,
        /// Specifies the stability of the body.
        #[serde(skip_serializing_if = "Option::is_none")]
        stability: Option<Stability>,
        /// Sequence of example values for the body or single example
        /// value. They are required only for string types. Example values
        /// must be of the same type of the body. If only a single example is
        /// provided, it can directly be reported without encapsulating it
        /// into a sequence/dictionary.
        #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<Examples>,
    },
}

impl BodySpec {
    /// Returns true if the body field is required.
    #[must_use]
    pub fn has_fields(&self) -> bool {
        match self {
            BodySpec::Fields { fields, .. } => !fields.is_empty(),
            BodySpec::String { .. } => false,
        }
    }
}

/// Identifies the different types of body (specification).
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodyType {
    /// A map body type.
    Map,
    /// A string body type.
    String,
}

/// Implements a human readable display for PrimitiveOrArrayType.
impl Display for BodyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyType::String => write!(f, "string"),
            BodyType::Map => write!(f, "map"),
        }
    }
}

/// A `BodyField` specification.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct BodyFieldSpec {
    /// String that uniquely identifies the body field.
    pub id: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: AttributeType,
    /// A brief description of the body field.
    pub brief: String,
    /// Sequence of example values for the body field or single example
    /// value. They are required only for string and string array
    /// fields. Example values must be of the same type of the
    /// body field. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Specifies if the body field is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the body field is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the body field is required.
    #[serde(default)]
    pub requirement_level: RequirementLevel,
    /// A more elaborate description of the body field.
    /// It defaults to an empty string.
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the body field.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the body field is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

impl BodyFieldSpec {
    /// Returns true if the body field is required.
    #[must_use]
    pub fn is_required(&self) -> bool {
        matches!(
            self,
            BodyFieldSpec {
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{
        EnumEntriesSpec, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec,
    };

    #[test]
    fn test_body_field_type_display() {
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean)
            ),
            "boolean"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int)
            ),
            "int"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double)
            ),
            "double"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
            ),
            "string"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings)
            ),
            "string[]"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints)
            ),
            "int[]"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles)
            ),
            "double[]"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans)
            ),
            "boolean[]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Boolean)),
            "template[boolean]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Int)),
            "template[int]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Double)),
            "template[double]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::String)),
            "template[string]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Strings)),
            "template[string[]]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Ints)),
            "template[int[]]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Doubles)),
            "template[double[]]"
        );
        assert_eq!(
            format!("{}", AttributeType::Template(TemplateTypeSpec::Booleans)),
            "template[boolean[]]"
        );
        assert_eq!(
            format!(
                "{}",
                AttributeType::Enum {
                    allow_custom_values: true,
                    members: vec![EnumEntriesSpec {
                        id: "id".to_owned(),
                        value: ValueSpec::Int(42),
                        brief: Some("brief".to_owned()),
                        note: Some("note".to_owned()),
                        stability: None,
                        deprecated: None,
                    }]
                }
            ),
            "enum {id}"
        );
    }

    #[test]
    fn test_field_body() {
        let body = BodySpec::Fields {
            r#type: BodyType::Map,
            brief: "brief".to_owned(),
            note: "note".to_owned(),
            stability: Some(Stability::Stable),
            examples: Some(Examples::Int(42)),
            fields: vec![BodyFieldSpec {
                id: "id".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                brief: "brief".to_owned(),
                examples: Some(Examples::Int(42)),
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: Some("deprecated".to_owned()),
            }],
        };

        assert!(matches!(body, BodySpec::Fields { .. }));
        assert!(!matches!(body, BodySpec::String { .. }));
        assert!(body.has_fields());

        if let BodySpec::Fields {
            brief,
            note,
            fields,
            ..
        } = body
        {
            assert_eq!(brief, "brief");
            assert_eq!(note, "note");
            assert!(fields.len() == 1);
        }
    }

    #[test]
    fn test_string_body() {
        let body = BodySpec::String {
            r#type: BodyType::String,
            brief: "brief".to_owned(),
            note: "note".to_owned(),
            stability: Some(Stability::Stable),
            examples: Some(Examples::String("{key: value}".to_owned())),
        };

        assert!(matches!(body, BodySpec::String { .. }));
        assert!(!matches!(body, BodySpec::Fields { .. }));
        assert!(!body.has_fields());

        if let BodySpec::String { brief, note, .. } = body {
            assert_eq!(brief, "brief");
            assert_eq!(note, "note");
        }
    }

    #[test]
    fn test_body_field() {
        let field = BodyFieldSpec {
            id: "id".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            brief: "brief".to_owned(),
            examples: Some(Examples::Int(42)),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            note: "note".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
        };
        assert_eq!(field.id, "id");
        assert_eq!(field.brief.to_owned(), "brief".to_owned());
        assert_eq!(field.note, "note");
        assert!(field.is_required());
    }
}

/// A Body Field definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct BodyFieldSpecWithProvenance {
    /// The body field definition.
    pub body_field: BodyFieldSpec,
    /// The provenance of the body field (path or URL).
    pub provenance: String,
}
