// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved attribute.

use crate::catalog::Stability;
use crate::tags::Tags;
use crate::value::Value;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use weaver_semconv::attribute::AttributeSpec;

/// An attribute definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
    pub examples: Option<Example>,
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

/// The different types of attributes.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(tag = "type")]
pub enum AttributeType {
    /// A boolean attribute.
    Boolean,
    /// A integer attribute (signed 64 bit integer).
    Int,
    /// A double attribute (double precision floating point (IEEE 754-1985)).
    Double,
    /// A string attribute.
    String,
    /// An array of strings attribute.
    Strings,
    /// An array of integer attribute.
    Ints,
    /// An array of double attribute.
    Doubles,
    /// An array of boolean attribute.
    Booleans,

    /// A template boolean attribute.
    TemplateBoolean,
    /// A template integer attribute.
    #[serde(rename = "template[int]")]
    TemplateInt,
    /// A template double attribute.
    #[serde(rename = "template[double]")]
    TemplateDouble,
    /// A template string attribute.
    #[serde(rename = "template[string]")]
    TemplateString,
    /// A template array of strings attribute.
    #[serde(rename = "template[string[]]")]
    TemplateStrings,
    /// A template array of integer attribute.
    #[serde(rename = "template[int[]]")]
    TemplateInts,
    /// A template array of double attribute.
    #[serde(rename = "template[double[]]")]
    TemplateDoubles,
    /// A template array of boolean attribute.
    #[serde(rename = "template[boolean[]]")]
    TemplateBooleans,

    /// An enum definition type.
    Enum {
        /// Set to false to not accept values other than the specified members.
        /// It defaults to true.
        allow_custom_values: bool,
        /// List of enum entries.
        members: Vec<EnumEntries>,
    },
}

/// Possible enum entries.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct EnumEntries {
    /// String that uniquely identifies the enum entry.
    pub id: String,
    /// String, int, or boolean; value of the enum entry.
    pub value: Value,
    /// Brief description of the enum entry value.
    /// It defaults to the value of id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The different types of examples.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(tag = "type")]
pub enum Example {
    /// A boolean example.
    Bool {
        /// The value of the example.
        value: bool,
    },
    /// A integer example.
    Int {
        /// The value of the example.
        value: i64,
    },
    /// A double example.
    Double {
        /// The value of the example.
        value: OrderedFloat<f64>,
    },
    /// A string example.
    String {
        /// The value of the example.
        value: String,
    },
    /// A array of integers example.
    Ints {
        /// The value of the example.
        values: Vec<i64>,
    },
    /// A array of doubles example.
    Doubles {
        /// The value of the example.
        values: Vec<OrderedFloat<f64>>,
    },
    /// A array of bools example.
    Bools {
        /// The value of the example.
        values: Vec<bool>,
    },
    /// A array of strings example.
    Strings {
        /// The value of the example.
        values: Vec<String>,
    },
}

impl Example {
    /// Creates an example from a f64.
    pub fn from_f64(value: f64) -> Self {
        Example::Double {
            value: OrderedFloat(value),
        }
    }

    /// Creates an example from several f64.
    pub fn from_f64s(values: Vec<f64>) -> Self {
        Example::Doubles {
            values: values.into_iter().map(OrderedFloat).collect(),
        }
    }
}

/// The different requirement levels.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(tag = "type")]
pub enum RequirementLevel {
    /// A required requirement level.
    Required,
    /// An optional requirement level.
    Recommended {
        /// The description of the recommendation.
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// An opt-in requirement level.
    OptIn,
    /// A conditional requirement level.
    ConditionallyRequired {
        /// The description of the condition.
        #[serde(skip_serializing_if = "String::is_empty")]
        text: String,
    },
}

/// An internal reference to an attribute in the catalog.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct AttributeRef(pub u32);
