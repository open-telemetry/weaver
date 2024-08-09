// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Attribute specification.

use crate::stability::Stability;
use crate::Error;
use ordered_float::OrderedFloat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Not;
use AttributeType::{Enum, PrimitiveOrArray, Template};

/// An attribute specification.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum AttributeSpec {
    /// Reference to another attribute.
    ///
    /// ref MUST have an id of an existing attribute.
    Ref {
        /// Reference an existing attribute.
        r#ref: String,
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
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
        /// Specifies the prefix of the attribute.
        /// If this parameter is set, the resolved id of the referenced attribute will
        /// have group prefix added to it.
        /// It defaults to false.
        #[serde(default)]
        #[serde(skip_serializing_if = "<&bool>::not")]
        prefix: bool,
    },
    /// Attribute definition.
    Id {
        /// String that uniquely identifies the attribute.
        id: String,
        /// Either a string literal denoting the type as a primitive or an
        /// array type, a template type or an enum definition.
        r#type: AttributeType,
        /// A brief description of the attribute.
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
        #[serde(default)]
        requirement_level: RequirementLevel,
        /// Specifies if the attribute is (especially) relevant for sampling
        /// and thus should be set at span start. It defaults to false.
        /// Note: this field is experimental.
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_relevant: Option<bool>,
        /// A more elaborate description of the attribute.
        /// It defaults to an empty string.
        #[serde(default)]
        note: String,
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
}

impl AttributeSpec {
    /// Returns true if the attribute is required.
    #[must_use]
    pub fn is_required(&self) -> bool {
        matches!(
            self,
            AttributeSpec::Ref {
                requirement_level: Some(RequirementLevel::Basic(
                    BasicRequirementLevelSpec::Required
                )),
                ..
            } | AttributeSpec::Id {
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                ..
            }
        )
    }

    /// Returns the id of the attribute.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            AttributeSpec::Ref { r#ref, .. } => r#ref.clone(),
            AttributeSpec::Id { id, .. } => id.clone(),
        }
    }

    /// Returns the brief of the attribute.
    #[must_use]
    pub fn brief(&self) -> String {
        match self {
            AttributeSpec::Ref { brief, .. } => brief.clone().unwrap_or_default(),
            AttributeSpec::Id { brief, .. } => brief.clone().unwrap_or_default(),
        }
    }

    /// Returns the note of the attribute.
    #[must_use]
    pub fn note(&self) -> String {
        match self {
            AttributeSpec::Ref { note, .. } => note.clone().unwrap_or_default(),
            AttributeSpec::Id { note, .. } => note.clone(),
        }
    }

    /// Returns the tag of the attribute (if any).
    #[must_use]
    pub fn tag(&self) -> Option<String> {
        match self {
            AttributeSpec::Ref { tag, .. } => tag.clone(),
            AttributeSpec::Id { tag, .. } => tag.clone(),
        }
    }
}

/// The different types of attributes (specification).
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum AttributeType {
    /// Primitive or array type.
    PrimitiveOrArray(PrimitiveOrArrayTypeSpec),
    /// A template type.
    Template(TemplateTypeSpec),
    /// An enum definition type.
    Enum {
        /// Set to false to not accept values other than the specified members.
        /// It defaults to true.
        #[serde(default = "default_as_true")]
        allow_custom_values: bool,
        /// List of enum entries.
        members: Vec<EnumEntriesSpec>,
    },
}

/// Implements a human readable display for AttributeType.
impl Display for AttributeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimitiveOrArray(t) => write!(f, "{}", t),
            Template(t) => write!(f, "{}", t),
            Enum { members, .. } => {
                let entries = members
                    .iter()
                    .map(|m| m.id.clone())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "enum {{{}}}", entries)
            }
        }
    }
}

/// Specifies the default value for allow_custom_values.
fn default_as_true() -> bool {
    true
}

/// Primitive or array types.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveOrArrayTypeSpec {
    /// A boolean attribute.
    Boolean,
    /// A integer attribute (signed 64 bit integer).
    Int,
    /// A double attribute (double precision floating point (IEEE 754-1985)).
    Double,
    /// A string attribute.
    String,
    /// An array of strings attribute.
    #[serde(rename = "string[]")]
    Strings,
    /// An array of integer attribute.
    #[serde(rename = "int[]")]
    Ints,
    /// An array of double attribute.
    #[serde(rename = "double[]")]
    Doubles,
    /// An array of boolean attribute.
    #[serde(rename = "boolean[]")]
    Booleans,
}

/// Implements a human readable display for PrimitiveOrArrayType.
impl Display for PrimitiveOrArrayTypeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimitiveOrArrayTypeSpec::Boolean => write!(f, "boolean"),
            PrimitiveOrArrayTypeSpec::Int => write!(f, "int"),
            PrimitiveOrArrayTypeSpec::Double => write!(f, "double"),
            PrimitiveOrArrayTypeSpec::String => write!(f, "string"),
            PrimitiveOrArrayTypeSpec::Strings => write!(f, "string[]"),
            PrimitiveOrArrayTypeSpec::Ints => write!(f, "int[]"),
            PrimitiveOrArrayTypeSpec::Doubles => write!(f, "double[]"),
            PrimitiveOrArrayTypeSpec::Booleans => write!(f, "boolean[]"),
        }
    }
}

/// Template types.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemplateTypeSpec {
    /// A boolean attribute.
    #[serde(rename = "template[boolean]")]
    Boolean,
    /// A integer attribute.
    #[serde(rename = "template[int]")]
    Int,
    /// A double attribute.
    #[serde(rename = "template[double]")]
    Double,
    /// A string attribute.
    #[serde(rename = "template[string]")]
    String,
    /// An array of strings attribute.
    #[serde(rename = "template[string[]]")]
    Strings,
    /// An array of integer attribute.
    #[serde(rename = "template[int[]]")]
    Ints,
    /// An array of double attribute.
    #[serde(rename = "template[double[]]")]
    Doubles,
    /// An array of boolean attribute.
    #[serde(rename = "template[boolean[]]")]
    Booleans,
}

/// Implements a human readable display for TemplateType.
impl Display for TemplateTypeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateTypeSpec::Boolean => write!(f, "template[boolean]"),
            TemplateTypeSpec::Int => write!(f, "template[int]"),
            TemplateTypeSpec::Double => write!(f, "template[double]"),
            TemplateTypeSpec::String => write!(f, "template[string]"),
            TemplateTypeSpec::Strings => write!(f, "template[string[]]"),
            TemplateTypeSpec::Ints => write!(f, "template[int[]]"),
            TemplateTypeSpec::Doubles => write!(f, "template[double[]]"),
            TemplateTypeSpec::Booleans => write!(f, "template[boolean[]]"),
        }
    }
}

/// Possible enum entries.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnumEntriesSpec {
    /// String that uniquely identifies the enum entry.
    pub id: String,
    /// String, int, or boolean; value of the enum entry.
    pub value: ValueSpec,
    /// Brief description of the enum entry value.
    /// It defaults to the value of id.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// Stability of this enum value.
    pub stability: Option<Stability>,
    /// Deprecation note.
    pub deprecated: Option<String>,
}

/// Implements a human readable display for EnumEntries.
impl Display for EnumEntriesSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id={}, type={}", self.id, self.value)
    }
}

/// The different types of values.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ValueSpec {
    /// A integer value.
    Int(i64),
    /// A double value.
    Double(OrderedFloat<f64>),
    /// A string value.
    String(String),
}

/// Implements a human readable display for Value.
impl Display for ValueSpec {
    /// Formats the value.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueSpec::Int(v) => write!(f, "{}", v),
            ValueSpec::Double(v) => write!(f, "{}", v),
            ValueSpec::String(v) => write!(f, "{}", v),
        }
    }
}

/// Allows to convert a i64 into a ValueSpec.
impl From<i64> for ValueSpec {
    /// Converts a i64 into a ValueSpec.
    fn from(value: i64) -> Self {
        ValueSpec::Int(value)
    }
}

/// Allows to convert a f64 into a ValueSpec.
impl From<f64> for ValueSpec {
    /// Converts a f64 into a ValueSpec.
    fn from(value: f64) -> Self {
        ValueSpec::Double(OrderedFloat(value))
    }
}

/// Allows to convert a String into a ValueSpec.
impl From<String> for ValueSpec {
    /// Converts a String into a ValueSpec.
    fn from(value: String) -> Self {
        ValueSpec::String(value)
    }
}

/// Allows to convert a &str into a ValueSpec.
impl From<&str> for ValueSpec {
    /// Converts a &str into a ValueSpec.
    fn from(value: &str) -> Self {
        ValueSpec::String(value.to_owned())
    }
}

/// The different types of examples.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum Examples {
    /// A boolean example.
    Bool(bool),
    /// A integer example.
    Int(i64),
    /// A double example.
    Double(OrderedFloat<f64>),
    /// A string example.
    String(String),
    /// A array of integers example.
    Ints(Vec<i64>),
    /// A array of doubles example.
    Doubles(Vec<OrderedFloat<f64>>),
    /// A array of bools example.
    Bools(Vec<bool>),
    /// A array of strings example.
    Strings(Vec<String>),
    /// List of arrays of integers example.
    ListOfInts(Vec<Vec<i64>>),
    /// List of arrays of doubles example.
    ListOfDoubles(Vec<Vec<OrderedFloat<f64>>>),
    /// List of arrays of bools example.
    ListOfBools(Vec<Vec<bool>>),
    /// List of arrays of strings example.
    ListOfStrings(Vec<Vec<String>>),
}

impl Examples {
    /// Validation logic for the group.
    pub(crate) fn validate(
        &self,
        future_mode: bool,
        attr_type: &AttributeType,
        group_id: &str,
        attr_id: &str,
        path_or_url: &str,
    ) -> Result<(), Error> {
        match (self, attr_type) {
            (Examples::Bool(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean))
            | (Examples::Int(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int))
            | (Examples::Double(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double))
            | (Examples::String(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String))
            | (Examples::Ints(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int))
            | (Examples::Doubles(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double))
            | (Examples::Bools(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean))
            | (Examples::Strings(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String))
            | (Examples::ListOfInts(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints))
            | (Examples::ListOfDoubles(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles))
            | (Examples::ListOfBools(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans))
            | (Examples::ListOfStrings(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings)) => {
                Ok(())
            }
            (_, Enum { .. }) => {
                // enum types are open so it's not possible to validate the examples
                Ok(())
            }
            // Only if future mode is disabled, we allow to have examples following
            // the conventions used in semconv 1.27.0 and earlier.
            (Examples::Ints(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints))
            | (Examples::Doubles(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles))
            | (Examples::Bools(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans))
            | (Examples::Strings(_), PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings))
                if !future_mode =>
            {
                Ok(())
            }
            (Examples::String(_), Template(TemplateTypeSpec::String))
            | (Examples::Strings(_), Template(TemplateTypeSpec::String))
            | (Examples::String(_), Template(TemplateTypeSpec::Strings))
            | (Examples::Strings(_), Template(TemplateTypeSpec::Strings)) => Ok(()),
            _ => Err(Error::InvalidExample {
                path_or_url: path_or_url.to_owned(),
                group_id: group_id.to_owned(),
                attribute_id: attr_id.to_owned(),
                error: format!("All examples must be of type `{}`", attr_type),
            }),
        }
    }
}

/// The different requirement level specifications.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum RequirementLevel {
    /// A basic requirement level.
    Basic(BasicRequirementLevelSpec),
    /// A conditional requirement level.
    ConditionallyRequired {
        /// The description of the condition.
        #[serde(rename = "conditionally_required")]
        text: String,
    },
    /// A recommended requirement level.
    Recommended {
        /// The description of the recommendation.
        #[serde(rename = "recommended")]
        text: String,
    },
}

/// Implements a human readable display for RequirementLevel.
impl Display for RequirementLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RequirementLevel::Basic(brl) => write!(f, "{}", brl),
            RequirementLevel::ConditionallyRequired { text } => {
                write!(f, "conditionally required (condition: {})", text)
            }
            RequirementLevel::Recommended { text } => write!(f, "recommended ({})", text),
        }
    }
}

// Specifies the default requirement level as defined in the OTel
// specification.
impl Default for RequirementLevel {
    fn default() -> Self {
        RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
    }
}

/// The different types of basic requirement levels.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BasicRequirementLevelSpec {
    /// A required requirement level.
    Required,
    /// An optional requirement level.
    Recommended,
    /// An opt-in requirement level.
    OptIn,
    /// An optional requirement level.
    Optional,
}

/// Implements a human readable display for BasicRequirementLevel.
impl Display for BasicRequirementLevelSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BasicRequirementLevelSpec::Required => write!(f, "required"),
            BasicRequirementLevelSpec::Recommended => write!(f, "recommended"),
            BasicRequirementLevelSpec::OptIn => write!(f, "opt-in"),
            BasicRequirementLevelSpec::Optional => write!(f, "optional"),
        }
    }
}

impl Examples {
    /// Creates an example from a f64.
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        Examples::Double(OrderedFloat(value))
    }

    /// Creates an example from several f64.
    #[must_use]
    pub fn from_f64s(values: Vec<f64>) -> Self {
        Examples::Doubles(values.into_iter().map(OrderedFloat).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_spec_display() {
        assert_eq!(format!("{}", ValueSpec::Int(42)), "42");
        assert_eq!(format!("{}", ValueSpec::Double(OrderedFloat(42.0))), "42");
        assert_eq!(format!("{}", ValueSpec::String("42".to_owned())), "42");
    }

    #[test]
    fn test_requirement_level_spec_display() {
        assert_eq!(
            format!(
                "{}",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
            ),
            "required"
        );
        assert_eq!(
            format!(
                "{}",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
            ),
            "recommended"
        );
        assert_eq!(
            format!(
                "{}",
                RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
            ),
            "opt-in"
        );
        assert_eq!(
            format!(
                "{}",
                RequirementLevel::ConditionallyRequired {
                    text: "condition".to_owned()
                }
            ),
            "conditionally required (condition: condition)"
        );
        assert_eq!(
            format!(
                "{}",
                RequirementLevel::Recommended {
                    text: "recommendation".to_owned()
                }
            ),
            "recommended (recommendation)"
        );
    }

    #[test]
    fn test_basic_requirement_level_spec_display() {
        assert_eq!(
            format!("{}", BasicRequirementLevelSpec::Required),
            "required"
        );
        assert_eq!(
            format!("{}", BasicRequirementLevelSpec::Recommended),
            "recommended"
        );
        assert_eq!(format!("{}", BasicRequirementLevelSpec::OptIn), "opt-in");
    }

    #[test]
    fn test_attribute_type_display() {
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean)),
            "boolean"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int)),
            "int"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double)),
            "double"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)),
            "string"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings)),
            "string[]"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints)),
            "int[]"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles)),
            "double[]"
        );
        assert_eq!(
            format!("{}", PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans)),
            "boolean[]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Boolean)),
            "template[boolean]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Int)),
            "template[int]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Double)),
            "template[double]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::String)),
            "template[string]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Strings)),
            "template[string[]]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Ints)),
            "template[int[]]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Doubles)),
            "template[double[]]"
        );
        assert_eq!(
            format!("{}", Template(TemplateTypeSpec::Booleans)),
            "template[boolean[]]"
        );
        assert_eq!(
            format!(
                "{}",
                Enum {
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
    fn test_primitive_or_array_type_spec_display() {
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Boolean), "boolean");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Int), "int");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Double), "double");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::String), "string");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Strings), "string[]");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Ints), "int[]");
        assert_eq!(format!("{}", PrimitiveOrArrayTypeSpec::Doubles), "double[]");
        assert_eq!(
            format!("{}", PrimitiveOrArrayTypeSpec::Booleans),
            "boolean[]"
        );
    }

    #[test]
    fn test_template_type_spec_display() {
        assert_eq!(
            format!("{}", TemplateTypeSpec::Boolean),
            "template[boolean]"
        );
        assert_eq!(format!("{}", TemplateTypeSpec::Int), "template[int]");
        assert_eq!(format!("{}", TemplateTypeSpec::Double), "template[double]");
        assert_eq!(format!("{}", TemplateTypeSpec::String), "template[string]");
        assert_eq!(
            format!("{}", TemplateTypeSpec::Strings),
            "template[string[]]"
        );
        assert_eq!(format!("{}", TemplateTypeSpec::Ints), "template[int[]]");
        assert_eq!(
            format!("{}", TemplateTypeSpec::Doubles),
            "template[double[]]"
        );
        assert_eq!(
            format!("{}", TemplateTypeSpec::Booleans),
            "template[boolean[]]"
        );
    }

    #[test]
    fn test_enum_entries_spec_display() {
        let entries = EnumEntriesSpec {
            id: "id".to_owned(),
            value: ValueSpec::Int(42),
            brief: Some("brief".to_owned()),
            note: Some("note".to_owned()),
            stability: None,
            deprecated: None,
        };
        assert_eq!(format!("{}", entries), "id=id, type=42");
    }

    #[test]
    fn test_examples_from_f64() {
        assert_eq!(
            Examples::from_f64(42.0),
            Examples::Double(OrderedFloat(42.0))
        );
    }

    #[test]
    fn test_examples_from_f64s() {
        assert_eq!(
            Examples::from_f64s(vec![42.0, 43.0]),
            Examples::Doubles(vec![OrderedFloat(42.0), OrderedFloat(43.0)])
        );
    }

    #[test]
    fn test_attribute() {
        let attr = AttributeSpec::Id {
            id: "id".to_owned(),
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            brief: Some("brief".to_owned()),
            examples: Some(Examples::Int(42)),
            tag: Some("tag".to_owned()),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: Some(true),
            note: "note".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
        };
        assert_eq!(attr.id(), "id");
        assert_eq!(attr.brief(), "brief");
        assert_eq!(attr.note(), "note");
        assert_eq!(attr.tag(), Some("tag".to_owned()));
        assert!(attr.is_required());

        let attr = AttributeSpec::Ref {
            r#ref: "ref".to_owned(),
            brief: Some("brief".to_owned()),
            examples: Some(Examples::Int(42)),
            tag: Some("tag".to_owned()),
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::Required)),
            sampling_relevant: Some(true),
            note: Some("note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: Some("deprecated".to_owned()),
            prefix: false,
        };
        assert_eq!(attr.id(), "ref");
        assert_eq!(attr.brief(), "brief");
        assert_eq!(attr.note(), "note");
        assert_eq!(attr.tag(), Some("tag".to_owned()));
        assert!(attr.is_required());
    }

    #[test]
    fn test_examples_bool() {
        let yaml = "---\ntrue";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::Bool(true));
    }

    #[test]
    fn test_examples_int() {
        let yaml = "---\n42";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::Int(42));
    }

    #[test]
    fn test_examples_double() {
        let yaml = "---\n3.15";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::Double(OrderedFloat(3.15)));
    }

    #[test]
    fn test_examples_string() {
        let yaml = "---\n\"foo\"";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::String("foo".to_owned()));
    }

    #[test]
    fn test_examples_strings() {
        let yaml = "---\n- \"foo\"\n- \"bar\"";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::Strings(vec!["foo".to_owned(), "bar".to_owned()])
        );
    }

    #[test]
    fn test_examples_ints() {
        let yaml = "---\n- 42\n- 43";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::Ints(vec![42, 43]));
    }

    #[test]
    fn test_examples_doubles() {
        let yaml = "---\n- 3.15\n- 2.71";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::Doubles(vec![OrderedFloat(3.15), OrderedFloat(2.71)])
        );
    }

    #[test]
    fn test_examples_bools() {
        let yaml = "---\n- true\n- false";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::Bools(vec![true, false]));
    }

    #[test]
    fn test_examples_list_of_ints() {
        let yaml = "---\n- [42, 43]\n- [44, 45]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]));
    }

    #[test]
    fn test_examples_list_of_doubles() {
        let yaml = "---\n- [3.15, 2.71]\n- [1.41, 1.61]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfDoubles(vec![
                vec![OrderedFloat(3.15), OrderedFloat(2.71)],
                vec![OrderedFloat(1.41), OrderedFloat(1.61)]
            ])
        );
    }

    #[test]
    fn test_examples_list_of_bools() {
        let yaml = "---\n- [true, false]\n- [false, true]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfBools(vec![vec![true, false], vec![false, true]])
        );
    }

    #[test]
    fn test_examples_list_of_strings() {
        let yaml = "---\n- [\"foo\", \"bar\"]\n- [\"baz\", \"qux\"]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfStrings(vec![
                vec!["foo".to_owned(), "bar".to_owned()],
                vec!["baz".to_owned(), "qux".to_owned()]
            ])
        );
    }

    #[test]
    fn test_examples_list_of_ints_array_style() {
        let yaml = "[ [42, 43], [44, 45] ]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ex, Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]));
    }

    #[test]
    fn test_examples_list_of_doubles_array_style() {
        let yaml = "[ [3.15, 2.71], [1.41, 1.61] ]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfDoubles(vec![
                vec![OrderedFloat(3.15), OrderedFloat(2.71)],
                vec![OrderedFloat(1.41), OrderedFloat(1.61)]
            ])
        );
    }

    #[test]
    fn test_examples_list_of_bools_array_style() {
        let yaml = "[ [true, false], [false, true] ]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfBools(vec![vec![true, false], vec![false, true]])
        );
    }

    #[test]
    fn test_examples_list_of_strings_array_style() {
        let yaml = "[ [\"foo\", \"bar\"], [\"baz\", \"qux\"] ]";
        let ex: Examples = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            ex,
            Examples::ListOfStrings(vec![
                vec!["foo".to_owned(), "bar".to_owned()],
                vec!["baz".to_owned(), "qux".to_owned()]
            ])
        );
    }

    #[test]
    fn test_examples_validate() {
        let attr_int = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int);
        let attr_ints = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints);
        let attr_double = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double);
        let attr_doubles = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles);
        let attr_str = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String);
        let attr_strs = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings);
        let attr_bool = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean);
        let attr_bools = PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans);

        // === Test int-like examples ===
        let examples = Examples::Int(42);
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_str, "grp", "attr", "url")
            .is_err());

        let examples = Examples::Ints(vec![42, 43]);
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_str, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]);
        assert!(examples
            .validate(true, &attr_ints, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfStrings(vec![
            vec!["42".to_owned(), "43".to_owned()],
            vec!["44".to_owned(), "45".to_owned()],
        ]);
        assert!(examples
            .validate(true, &attr_ints, "grp", "attr", "url")
            .is_err());

        // Non-strict validation
        let examples = Examples::Ints(vec![42, 43]);
        assert!(examples
            .validate(false, &attr_ints, "grp", "attr", "url")
            .is_ok());

        // === Test string-like examples ===
        let examples = Examples::String("foo".to_owned());
        assert!(examples
            .validate(true, &attr_str, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::Strings(vec!["foo".to_owned(), "bar".to_owned()]);
        assert!(examples
            .validate(true, &attr_str, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfStrings(vec![
            vec!["foo".to_owned(), "bar".to_owned()],
            vec!["baz".to_owned(), "qux".to_owned()],
        ]);
        assert!(examples
            .validate(true, &attr_strs, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_ints, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]);
        assert!(examples
            .validate(true, &attr_str, "grp", "attr", "url")
            .is_err());

        // Non-strict validation
        let examples = Examples::Strings(vec!["foo".to_owned(), "bar".to_owned()]);
        assert!(examples
            .validate(false, &attr_str, "grp", "attr", "url")
            .is_ok());

        // === Test bool-like examples ===
        let examples = Examples::Bool(true);
        assert!(examples
            .validate(true, &attr_bool, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::Bools(vec![true, false]);
        assert!(examples
            .validate(true, &attr_bool, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfBools(vec![vec![true, false], vec![false, true]]);
        assert!(examples
            .validate(true, &attr_bools, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_bool, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]);
        assert!(examples
            .validate(true, &attr_bool, "grp", "attr", "url")
            .is_err());

        // Non-strict validation
        let examples = Examples::Bools(vec![true, false]);
        assert!(examples
            .validate(false, &attr_bools, "grp", "attr", "url")
            .is_ok());

        // === Test double-like examples ===
        let examples = Examples::Double(OrderedFloat(42.0));
        assert!(examples
            .validate(true, &attr_double, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::Doubles(vec![OrderedFloat(42.0), OrderedFloat(43.0)]);
        assert!(examples
            .validate(true, &attr_double, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_int, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfDoubles(vec![
            vec![OrderedFloat(42.0), OrderedFloat(43.0)],
            vec![OrderedFloat(44.0), OrderedFloat(45.0)],
        ]);
        assert!(examples
            .validate(true, &attr_doubles, "grp", "attr", "url")
            .is_ok());
        assert!(examples
            .validate(true, &attr_double, "grp", "attr", "url")
            .is_err());

        let examples = Examples::ListOfInts(vec![vec![42, 43], vec![44, 45]]);
        assert!(examples
            .validate(true, &attr_double, "grp", "attr", "url")
            .is_err());

        // Non-strict validation
        let examples = Examples::Doubles(vec![OrderedFloat(42.0), OrderedFloat(43.0)]);
        assert!(examples
            .validate(false, &attr_doubles, "grp", "attr", "url")
            .is_ok());
    }
}

/// An attribute definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct AttributeSpecWithProvenance {
    /// The attribute definition.
    pub attribute: AttributeSpec,
    /// The provenance of the attribute (path or URL).
    pub provenance: String,
}
