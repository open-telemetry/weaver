// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Attribute specification.

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::stability::StabilitySpec;

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
        examples: Option<ExamplesSpec>,
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
        requirement_level: Option<RequirementLevelSpec>,
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
        stability: Option<StabilitySpec>,
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
    },
    /// Attribute definition.
    Id {
        /// String that uniquely identifies the attribute.
        id: String,
        /// Either a string literal denoting the type as a primitive or an
        /// array type, a template type or an enum definition.
        r#type: AttributeTypeSpec,
        /// A brief description of the attribute.
        brief: Option<String>,
        /// Sequence of example values for the attribute or single example
        /// value. They are required only for string and string array
        /// attributes. Example values must be of the same type of the
        /// attribute. If only a single example is provided, it can directly
        /// be reported without encapsulating it into a sequence/dictionary.
        #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<ExamplesSpec>,
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
        requirement_level: RequirementLevelSpec,
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
        stability: Option<StabilitySpec>,
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
    },
}

impl AttributeSpec {
    /// Returns true if the attribute is required.
    pub fn is_required(&self) -> bool {
        matches!(
            self,
            AttributeSpec::Ref {
                requirement_level: Some(RequirementLevelSpec::Basic(
                    BasicRequirementLevelSpec::Required
                )),
                ..
            } | AttributeSpec::Id {
                requirement_level: RequirementLevelSpec::Basic(BasicRequirementLevelSpec::Required),
                ..
            }
        )
    }

    /// Returns the id of the attribute.
    pub fn id(&self) -> String {
        match self {
            AttributeSpec::Ref { r#ref, .. } => r#ref.clone(),
            AttributeSpec::Id { id, .. } => id.clone(),
        }
    }

    /// Returns the brief of the attribute.
    pub fn brief(&self) -> String {
        match self {
            AttributeSpec::Ref { brief, .. } => brief.clone().unwrap_or_default(),
            AttributeSpec::Id { brief, .. } => brief.clone().unwrap_or_default(),
        }
    }

    /// Returns the note of the attribute.
    pub fn note(&self) -> String {
        match self {
            AttributeSpec::Ref { note, .. } => note.clone().unwrap_or_default(),
            AttributeSpec::Id { note, .. } => note.clone(),
        }
    }

    /// Returns the tag of the attribute (if any).
    pub fn tag(&self) -> Option<String> {
        match self {
            AttributeSpec::Ref { tag, .. } => tag.clone(),
            AttributeSpec::Id { tag, .. } => tag.clone(),
        }
    }
}

/// The different types of attributes (specification).
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum AttributeTypeSpec {
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
impl Display for AttributeTypeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeTypeSpec::PrimitiveOrArray(t) => write!(f, "{}", t),
            AttributeTypeSpec::Template(t) => write!(f, "{}", t),
            AttributeTypeSpec::Enum { members, .. } => {
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
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
}

/// Implements a human readable display for EnumEntries.
impl Display for EnumEntriesSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id={}, type={}", self.id, self.value)
    }
}

/// The different types of values.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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

/// The different types of examples.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ExamplesSpec {
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
}

/// The different requirement level specifications.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum RequirementLevelSpec {
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
impl Display for RequirementLevelSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RequirementLevelSpec::Basic(brl) => write!(f, "{}", brl),
            RequirementLevelSpec::ConditionallyRequired { text } => {
                write!(f, "conditionally required (condition: {})", text)
            }
            RequirementLevelSpec::Recommended { text } => write!(f, "recommended ({})", text),
        }
    }
}

// Specifies the default requirement level as defined in the OTel
// specification.
impl Default for RequirementLevelSpec {
    fn default() -> Self {
        RequirementLevelSpec::Basic(BasicRequirementLevelSpec::Recommended)
    }
}

/// The different types of basic requirement levels.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BasicRequirementLevelSpec {
    /// A required requirement level.
    Required,
    /// An optional requirement level.
    Recommended,
    /// An opt-in requirement level.
    OptIn,
}

/// Implements a human readable display for BasicRequirementLevel.
impl Display for BasicRequirementLevelSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BasicRequirementLevelSpec::Required => write!(f, "required"),
            BasicRequirementLevelSpec::Recommended => write!(f, "recommended"),
            BasicRequirementLevelSpec::OptIn => write!(f, "opt-in"),
        }
    }
}
