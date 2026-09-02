// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attributes going forward.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deprecated::Deprecated;
use crate::stability::Stability;
use crate::v2::{signal_id::SignalId, CommonFields};
use crate::YamlValue;
use weaver_common::ordered_float::OrderedF64;

/// Primitive or array types.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// An any type attribute (accepts any valid value).
    #[serde(rename = "any")]
    Any,
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
            PrimitiveOrArrayTypeSpec::Any => write!(f, "any"),
        }
    }
}

/// Template types.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// A any attribute.
    #[serde(rename = "template[any]")]
    Any,
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

impl Display for TemplateTypeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateTypeSpec::Boolean => write!(f, "template[boolean]"),
            TemplateTypeSpec::Int => write!(f, "template[int]"),
            TemplateTypeSpec::Double => write!(f, "template[double]"),
            TemplateTypeSpec::String => write!(f, "template[string]"),
            TemplateTypeSpec::Any => write!(f, "template[any]"),
            TemplateTypeSpec::Strings => write!(f, "template[string[]]"),
            TemplateTypeSpec::Ints => write!(f, "template[int[]]"),
            TemplateTypeSpec::Doubles => write!(f, "template[double[]]"),
            TemplateTypeSpec::Booleans => write!(f, "template[boolean[]]"),
        }
    }
}

/// The different types of values.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ValueSpec {
    /// A integer value.
    Int(i64),
    /// A double value.
    #[cfg_attr(feature = "openapi", schema(value_type = f64))]
    Double(OrderedF64),
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
}

impl Display for ValueSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueSpec::Int(v) => write!(f, "{v}"),
            ValueSpec::Double(v) => write!(f, "{v}"),
            ValueSpec::String(v) => write!(f, "{v}"),
            ValueSpec::Bool(v) => write!(f, "{v}"),
        }
    }
}

impl From<i64> for ValueSpec {
    fn from(value: i64) -> Self {
        ValueSpec::Int(value)
    }
}

impl From<f64> for ValueSpec {
    fn from(value: f64) -> Self {
        ValueSpec::Double(OrderedF64(value))
    }
}

impl From<String> for ValueSpec {
    fn from(value: String) -> Self {
        ValueSpec::String(value)
    }
}

impl From<&str> for ValueSpec {
    fn from(value: &str) -> Self {
        ValueSpec::String(value.to_owned())
    }
}

/// Possible enum entries.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[serde(from = "EnumEntriesSpecDeserialize")]
pub struct EnumEntriesSpec {
    /// String that uniquely identifies the enum entry.
    pub id: String,
    /// String, int, or boolean; value of the enum entry.
    /// If omitted, defaults to the value of `id`.
    pub value: ValueSpec,
    /// Brief description of the enum entry value.
    /// It defaults to the value of id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Stability of this enum value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Deprecation note.
    #[serde(
        deserialize_with = "crate::deprecated::deserialize_option_deprecated",
        default
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Annotations for the member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, YamlValue>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EnumEntriesSpecDeserialize {
    /// String that uniquely identifies the enum entry.
    id: String,
    /// String, int, or boolean; value of the enum entry.
    /// If omitted, defaults to the value of `id`.
    value: Option<ValueSpec>,
    /// Brief description of the enum entry value.
    /// It defaults to the value of id.
    brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    note: Option<String>,
    /// Stability of this enum value.
    stability: Option<Stability>,
    /// Deprecation note.
    #[serde(
        deserialize_with = "crate::deprecated::deserialize_option_deprecated",
        default
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    deprecated: Option<Deprecated>,
    /// Annotations for the member.
    annotations: Option<BTreeMap<String, YamlValue>>,
}

impl From<EnumEntriesSpecDeserialize> for EnumEntriesSpec {
    fn from(entry: EnumEntriesSpecDeserialize) -> Self {
        Self {
            value: entry
                .value
                .unwrap_or_else(|| ValueSpec::String(entry.id.clone())),
            id: entry.id,
            brief: entry.brief,
            note: entry.note,
            stability: entry.stability,
            deprecated: entry.deprecated,
            annotations: entry.annotations,
        }
    }
}

impl Display for EnumEntriesSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id={}, type={}", self.id, self.value)
    }
}

/// The different types of attributes (specification).
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum AttributeType {
    /// Primitive or array type.
    PrimitiveOrArray(PrimitiveOrArrayTypeSpec),
    /// A template type.
    Template(TemplateTypeSpec),
    /// An enum definition type.
    Enum {
        /// List of enum entries.
        members: Vec<EnumEntriesSpec>,
    },
}

impl Display for AttributeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeType::PrimitiveOrArray(t) => write!(f, "{t}"),
            AttributeType::Template(t) => write!(f, "{t}"),
            AttributeType::Enum { members, .. } => {
                let entries = members
                    .iter()
                    .map(|m| m.id.clone())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "enum {{{entries}}}")
            }
        }
    }
}

/// The different types of examples.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum Examples {
    /// A boolean example.
    Bool(bool),
    /// A integer example.
    Int(i64),
    /// A double example.
    #[cfg_attr(feature = "openapi", schema(value_type = f64))]
    Double(OrderedF64),
    /// A string example.
    String(String),
    /// A any example.
    Any(ValueSpec),
    /// A array of integers example.
    Ints(Vec<i64>),
    /// A array of doubles example.
    #[cfg_attr(feature = "openapi", schema(value_type = Vec<f64>))]
    Doubles(Vec<OrderedF64>),
    /// A array of bools example.
    Bools(Vec<bool>),
    /// A array of strings example.
    Strings(Vec<String>),
    /// A array of anys example.
    Anys(Vec<ValueSpec>),
    /// List of arrays of integers example.
    ListOfInts(Vec<Vec<i64>>),
    /// List of arrays of doubles example.
    #[cfg_attr(feature = "openapi", schema(value_type = Vec<Vec<f64>>))]
    ListOfDoubles(Vec<Vec<OrderedF64>>),
    /// List of arrays of bools example.
    ListOfBools(Vec<Vec<bool>>),
    /// List of arrays of strings example.
    ListOfStrings(Vec<Vec<String>>),
}

impl Examples {
    /// Creates an example from a f64.
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        Examples::Double(OrderedF64(value))
    }

    /// Creates an example from several f64.
    #[must_use]
    pub fn from_f64s(values: Vec<f64>) -> Self {
        Examples::Doubles(values.into_iter().map(OrderedF64).collect())
    }
}

/// The different types of basic requirement levels.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BasicRequirementLevelSpec {
    /// A required requirement level.
    Required,
    /// An optional requirement level.
    Recommended,
    /// An opt-in requirement level.
    OptIn,
}

impl Display for BasicRequirementLevelSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BasicRequirementLevelSpec::Required => write!(f, "required"),
            BasicRequirementLevelSpec::Recommended => write!(f, "recommended"),
            BasicRequirementLevelSpec::OptIn => write!(f, "opt-in"),
        }
    }
}

/// The different requirement level specifications.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// An opt in requirement level.
    OptIn {
        /// The description of the recommendation.
        #[serde(rename = "opt_in")]
        text: String,
    },
}

impl Display for RequirementLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RequirementLevel::Basic(brl) => write!(f, "{brl}"),
            RequirementLevel::ConditionallyRequired { text } => {
                write!(f, "conditionally required (condition: {text})")
            }
            RequirementLevel::Recommended { text } => write!(f, "recommended ({text})"),
            RequirementLevel::OptIn { text } => write!(f, "opt in ({text})"),
        }
    }
}

impl Default for RequirementLevel {
    fn default() -> Self {
        RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
    }
}

/// A refinement of an Attribute for a signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeRef {
    /// Reference an existing attribute by key.
    pub r#ref: String,

    /// Refines the brief description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refined sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Refines the attribute requirement level. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the original attribute requirement level is used. When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<RequirementLevel>,
    /// Refines the more elaborate description of the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Additional annotations for the attribute. These will be
    /// merged with annotations from the definition.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

/// The definition of an Attribute.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeDef {
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

/// A reference to an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: SignalId,
}

/// A reference to either an attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum AttributeOrGroupRef {
    /// Reference to an attribute.
    Attribute(AttributeRef),
    /// Reference to an attribute group.
    Group(GroupRef),
}

/// Helper function to split a vector of AttributeOrGroupRef into separate vectors
/// of AttributeRef and SignalId group references
#[must_use]
pub fn split_attributes_and_groups(
    attributes_and_groups: Vec<AttributeOrGroupRef>,
) -> (Vec<AttributeRef>, Vec<SignalId>) {
    let mut attributes = Vec::new();
    let mut groups = Vec::new();

    for item in attributes_and_groups {
        match item {
            AttributeOrGroupRef::Attribute(attr_ref) => {
                attributes.push(attr_ref);
            }
            AttributeOrGroupRef::Group(group_ref) => groups.push(group_ref.ref_group),
        }
    }

    (attributes, groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_ref_rejects_stability_and_deprecated() {
        for (field, name) in [
            ("stability: stable", "stability"),
            ("deprecated:\n  reason: obsoleted", "deprecated"),
        ] {
            let yaml = format!("ref: my.attribute\n{field}\n");
            let err = serde_yaml::from_str::<AttributeRef>(&yaml)
                .expect_err("stability/deprecated must not be allowed on attribute refs");
            assert_eq!(
                err.to_string(),
                format!(
                    "unknown field `{name}`, expected one of \
                     `ref`, `brief`, `examples`, `requirement_level`, `note`, `annotations` \
                     at line 2 column 1"
                )
            );
        }
    }

    #[test]
    fn test_display_implementations() {
        assert_eq!(PrimitiveOrArrayTypeSpec::Boolean.to_string(), "boolean");
        assert_eq!(PrimitiveOrArrayTypeSpec::Int.to_string(), "int");
        assert_eq!(PrimitiveOrArrayTypeSpec::Double.to_string(), "double");
        assert_eq!(PrimitiveOrArrayTypeSpec::String.to_string(), "string");
        assert_eq!(PrimitiveOrArrayTypeSpec::Any.to_string(), "any");
        assert_eq!(PrimitiveOrArrayTypeSpec::Strings.to_string(), "string[]");
        assert_eq!(PrimitiveOrArrayTypeSpec::Ints.to_string(), "int[]");
        assert_eq!(PrimitiveOrArrayTypeSpec::Doubles.to_string(), "double[]");
        assert_eq!(PrimitiveOrArrayTypeSpec::Booleans.to_string(), "boolean[]");

        assert_eq!(TemplateTypeSpec::Boolean.to_string(), "template[boolean]");
        assert_eq!(TemplateTypeSpec::Int.to_string(), "template[int]");
        assert_eq!(TemplateTypeSpec::Double.to_string(), "template[double]");
        assert_eq!(TemplateTypeSpec::String.to_string(), "template[string]");
        assert_eq!(TemplateTypeSpec::Any.to_string(), "template[any]");
        assert_eq!(TemplateTypeSpec::Strings.to_string(), "template[string[]]");
        assert_eq!(TemplateTypeSpec::Ints.to_string(), "template[int[]]");
        assert_eq!(TemplateTypeSpec::Doubles.to_string(), "template[double[]]");
        assert_eq!(
            TemplateTypeSpec::Booleans.to_string(),
            "template[boolean[]]"
        );

        assert_eq!(ValueSpec::Int(42).to_string(), "42");
        assert_eq!(ValueSpec::Double(2.5.into()).to_string(), "2.5");
        assert_eq!(ValueSpec::String("abc".to_owned()).to_string(), "abc");
        assert_eq!(ValueSpec::Bool(true).to_string(), "true");

        assert_eq!(BasicRequirementLevelSpec::Required.to_string(), "required");
        assert_eq!(
            BasicRequirementLevelSpec::Recommended.to_string(),
            "recommended"
        );
        assert_eq!(BasicRequirementLevelSpec::OptIn.to_string(), "opt-in");

        assert_eq!(
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required).to_string(),
            "required"
        );
        assert_eq!(
            RequirementLevel::ConditionallyRequired {
                text: "when active".to_owned()
            }
            .to_string(),
            "conditionally required (condition: when active)"
        );
        assert_eq!(
            RequirementLevel::Recommended {
                text: "for speed".to_owned()
            }
            .to_string(),
            "recommended (for speed)"
        );
        assert_eq!(
            RequirementLevel::OptIn {
                text: "exp".to_owned()
            }
            .to_string(),
            "opt in (exp)"
        );

        assert_eq!(
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String).to_string(),
            "string"
        );
        assert_eq!(
            AttributeType::Template(TemplateTypeSpec::Int).to_string(),
            "template[int]"
        );
        let entry = EnumEntriesSpec {
            id: "success".to_owned(),
            value: ValueSpec::Int(200),
            brief: None,
            note: None,
            stability: None,
            deprecated: None,
            annotations: Default::default(),
        };
        assert_eq!(entry.to_string(), "id=success, type=200");

        assert_eq!(
            AttributeType::Enum {
                members: vec![entry]
            }
            .to_string(),
            "enum {success}"
        );
    }

    #[test]
    fn test_value_spec_from_implementations() {
        assert_eq!(ValueSpec::from(100i64), ValueSpec::Int(100));
        assert_eq!(ValueSpec::from(2.5f64), ValueSpec::Double(2.5.into()));
        assert_eq!(ValueSpec::from("val"), ValueSpec::String("val".to_owned()));
        assert_eq!(
            ValueSpec::from("val".to_owned()),
            ValueSpec::String("val".to_owned())
        );
    }

    #[test]
    fn test_examples_helpers() {
        assert_eq!(Examples::from_f64(3.5), Examples::Double(3.5.into()));
        assert_eq!(
            Examples::from_f64s(vec![1.0, 2.0]),
            Examples::Doubles(vec![1.0.into(), 2.0.into()])
        );
    }

    #[test]
    fn test_split_attributes_and_groups() {
        let items = vec![
            AttributeOrGroupRef::Attribute(AttributeRef {
                r#ref: "attr1".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            }),
            AttributeOrGroupRef::Group(GroupRef {
                ref_group: SignalId::from("my_group"),
            }),
        ];

        let (attrs, groups) = split_attributes_and_groups(items);
        assert_eq!(attrs.len(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], SignalId::from("my_group"));
    }

    #[test]
    fn test_enum_entries_spec_deserialization_fallback() {
        let yaml = r#"
id: error_code
brief: An error code
"#;
        let entry: EnumEntriesSpec =
            serde_yaml::from_str(yaml).expect("Failed to deserialize EnumEntriesSpec");
        assert_eq!(entry.id, "error_code");
        // When value is omitted, it defaults to String(id)
        assert_eq!(entry.value, ValueSpec::String("error_code".to_owned()));
        assert_eq!(entry.brief, Some("An error code".to_owned()));
    }
}
