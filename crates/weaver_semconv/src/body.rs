// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Body Field specification.

use serde::{Deserialize, Serialize};

use crate::attribute::{
    AttributeType, BasicRequirementLevelSpec, Examples, RequirementLevel, ValueSpec,
};
use crate::stability::Stability;

/// A body specification
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum BodySpec {
    /// The collection of body fields associated with a body definition
    Fields {
        /// The collection of body fields associated with a body definition
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fields: Vec<BodyFieldSpec>,
    },
    /// The body field value.
    Value {
        /// The body field value.
        value: ValueSpec,
    },
}

impl BodySpec {
    /// Returns true if the body field is required.
    #[must_use]
    pub fn has_fields(&self) -> bool {
        match self {
            BodySpec::Fields { fields } => !fields.is_empty(),
            BodySpec::Value { value: _ } => false,
        }
    }
}

/// A body field specification.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum BodyFieldSpec {
    /// Reference to an attribute.
    ///
    /// ref MUST have an id of an existing attribute.
    Ref {
        /// Reference an existing attribute.
        r#ref: String,
        /// The alias to use for the referenced attribute in this body field,
        /// if not specified, the id of the referenced attribute is used.
        #[serde(skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
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
        /// Specifies if the attribute is mandatory. Can be "required",
        /// "conditionally_required", "recommended" or "opt_in". When omitted,
        /// the attribute is "recommended". When set to
        /// "conditionally_required", the string provided as <condition> MUST
        /// specify the conditions under which the attribute is required.
        #[serde(skip_serializing_if = "Option::is_none")]
        requirement_level: Option<RequirementLevel>,
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
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
    },
    /// Body Field definition.
    Id {
        /// String that uniquely identifies the body field.
        id: String,
        /// Either a string literal denoting the type as a primitive or an
        /// array type, a template type or an enum definition.
        r#type: AttributeType,
        /// A brief description of the body field.
        brief: Option<String>,
        /// Sequence of example values for the body field or single example
        /// value. They are required only for string and string array
        /// attributes. Example values must be of the same type of the
        /// body field. If only a single example is provided, it can directly
        /// be reported without encapsulating it into a sequence/dictionary.
        #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<Examples>,
        /// Specifies if the body field is mandatory. Can be "required",
        /// "conditionally_required", "recommended" or "opt_in". When omitted,
        /// the body field is "recommended". When set to
        /// "conditionally_required", the string provided as <condition> MUST
        /// specify the conditions under which the body field is required.
        #[serde(default)]
        requirement_level: RequirementLevel,
        /// A more elaborate description of the body field.
        /// It defaults to an empty string.
        #[serde(default)]
        note: String,
        /// Specifies the stability of the body field.
        /// Note that, if stability is missing but deprecated is present, it will
        /// automatically set the stability to deprecated. If deprecated is
        /// present and stability differs from deprecated, this will result in an
        /// error.
        #[serde(skip_serializing_if = "Option::is_none")]
        stability: Option<Stability>,
        /// Specifies if the body field is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
    },
}

impl BodyFieldSpec {
    /// Returns true if the body field is required.
    #[must_use]
    pub fn is_required(&self) -> bool {
        matches!(
            self,
            BodyFieldSpec::Id {
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                ..
            } | BodyFieldSpec::Ref {
                requirement_level: Some(RequirementLevel::Basic(
                    BasicRequirementLevelSpec::Required
                )),
                ..
            }
        )
    }

    /// Returns the id of the body field.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            BodyFieldSpec::Ref { r#ref, alias, .. } => {
                if alias.is_some() {
                    alias.clone().unwrap_or_default()
                } else {
                    r#ref.clone()
                }
            }
            BodyFieldSpec::Id { id, .. } => id.clone(),
        }
    }

    /// Returns the brief of the body field.
    #[must_use]
    pub fn brief(&self) -> String {
        match self {
            BodyFieldSpec::Ref { brief, .. } => brief.clone().unwrap_or_default(),
            BodyFieldSpec::Id { brief, .. } => brief.clone().unwrap_or_default(),
        }
    }

    /// Returns the note of the body field.
    #[must_use]
    pub fn note(&self) -> String {
        match self {
            BodyFieldSpec::Ref { note, .. } => note.clone().unwrap_or_default(),
            BodyFieldSpec::Id { note, .. } => note.clone(),
        }
    }
}

// /// The different types of body fields (specification).
// #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// #[serde(untagged)]
// pub enum BodyFieldType {
//     /// Primitive or array type.
//     PrimitiveOrArray(PrimitiveOrArrayTypeSpec),
//     /// A template type.
//     Template(TemplateTypeSpec),
//     /// An enum definition type.
//     Enum {
//         /// Set to false to not accept values other than the specified members.
//         /// It defaults to true.
//         #[serde(default = "default_as_true")]
//         allow_custom_values: bool,
//         /// List of enum entries.
//         members: Vec<EnumEntriesSpec>,
//     },
// }

/// Implements a human readable display for BodyFieldType.
// impl Display for BodyFieldType {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self {
//             BodyFieldType::PrimitiveOrArray(t) => write!(f, "{}", t),
//             BodyFieldType::Template(t) => write!(f, "{}", t),
//             BodyFieldType::Enum { members, .. } => {
//                 let entries = members
//                     .iter()
//                     .map(|m| m.id.clone())
//                     .collect::<Vec<String>>()
//                     .join(", ");
//                 write!(f, "enum {{{}}}", entries)
//             }
//         }
//     }
// }

/// Specifies the default value for allow_custom_values.
// fn default_as_true() -> bool {
//     true
// }

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
    fn test_body_field() {
        let attr = BodyFieldSpec::Id {
            id: "id".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            brief: Some("brief".to_owned()),
            examples: Some(Examples::Int(42)),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            note: "note".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
        };
        assert_eq!(attr.id(), "id");
        assert_eq!(attr.brief(), "brief");
        assert_eq!(attr.note(), "note");
        assert!(attr.is_required());

        let attr = BodyFieldSpec::Ref {
            r#ref: "ref".to_owned(),
            alias: None,
            brief: Some("brief".to_owned()),
            examples: Some(Examples::Int(42)),
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::Required)),
            note: Some("note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
        };
        assert_eq!(attr.id(), "ref");
        assert_eq!(attr.brief(), "brief");
        assert_eq!(attr.note(), "note");
        assert!(attr.is_required());

        let attr = BodyFieldSpec::Ref {
            r#ref: "ref".to_owned(),
            alias: Some("theAlias".to_owned()),
            brief: Some("brief".to_owned()),
            examples: Some(Examples::Int(42)),
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::Required)),
            note: Some("note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
        };
        assert_eq!(attr.id(), "theAlias");
        assert_eq!(attr.brief(), "brief");
        assert_eq!(attr.note(), "note");
        assert!(attr.is_required());
    }
}

/// An attribute definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct BodyFieldSpecWithProvenance {
    /// The body field definition.
    pub body_field: BodyFieldSpec,
    /// The provenance of the body field (path or URL).
    pub provenance: String,
}
