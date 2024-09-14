// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! AnyValue specification.

use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attribute::{BasicRequirementLevelSpec, EnumEntriesSpec, Examples, RequirementLevel};
use crate::stability::Stability;

/// The AnyValueTypeSpec is a specification of a value that can be of any type.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum AnyValueSpec {
    /// A boolean attribute.
    Boolean {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// A integer attribute (signed 64 bit integer).
    Int {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// A double attribute (double precision floating point (IEEE 754-1985)).
    Double {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// A string attribute.
    String {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// An array of strings attribute.
    #[serde(rename = "string[]")]
    Strings {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// An array of integer attribute.
    #[serde(rename = "int[]")]
    Ints {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// An array of double attribute.
    #[serde(rename = "double[]")]
    Doubles {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// An array of boolean attribute.
    #[serde(rename = "boolean[]")]
    Booleans {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
    },

    /// The value type is a map of key, value pairs
    Map {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,
        /// The collection of key, values where the value is an `AnyValueSpec`
        fields: Vec<AnyValueSpec>,
    },

    /// The value type will just be a bytes.
    Bytes {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec
    },

    /// The value type is not specified.
    Undefined {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec
    },

    /// An enum definition type.
    Enum {
        /// The common value specification
        #[serde(flatten)]
        common: AnyValueCommonSpec,

        /// Set to false to not accept values other than the specified members.
        /// It defaults to true.
        #[serde(default = "default_as_true")]
        allow_custom_values: bool,
        /// List of enum entries.
        members: Vec<EnumEntriesSpec>,
    }    
}

/// The Common Value specification for properties associated with an "AnyValue", this
/// is similar to the current `AttributeSpec` as at the proto level an Attribute
/// is defined as a "KeyValue".
/// While this is (currently) a duplication of the existing AttributeSpec, this is
/// to reduce the size of the change set.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AnyValueCommonSpec {
    /// String that uniquely identifies the enum entry.
    pub id: String,
    /// A brief description of the value
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub brief: String,
    /// A more elaborate description of the value.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Sequence of examples for the value or single example
    /// value. If only a single example is provided, it can
    /// directly be reported without encapsulating it
    /// into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Specifies if the field is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the field is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the field is required.
    pub requirement_level: RequirementLevel,
    /// Specifies if the body field is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

/// Implements a human readable display for AnyValueType.
impl Display for AnyValueSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyValueSpec::Map { fields, .. } => {
                let entries = fields
                    .iter()
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "map<{}>{{ {} }}", self.id(), entries)
            }
            AnyValueSpec::Boolean { .. } => write!(f, "boolean"),
            AnyValueSpec::Int { .. } => write!(f, "int"),
            AnyValueSpec::Double { .. } => write!(f, "double"),
            AnyValueSpec::String { .. } => write!(f, "string"),
            AnyValueSpec::Strings { .. } => write!(f, "string[]"),
            AnyValueSpec::Ints { .. } => write!(f, "int[]"),
            AnyValueSpec::Doubles { .. } => write!(f, "double[]"),
            AnyValueSpec::Booleans { .. } => write!(f, "boolean[]"),
            AnyValueSpec::Bytes { .. } => write!(f, "byte[]"),
            AnyValueSpec::Undefined { .. } => write!(f, "undefined"),
            AnyValueSpec::Enum { .. } => write!(f, "enum<{}>", self.id()),
        }
    }
}

impl AnyValueSpec {
    /// Returns the common value specification for each type.
    #[must_use]
    pub fn common(&self) -> &AnyValueCommonSpec {
        match self {
            AnyValueSpec::Boolean { common, .. } => common,
            AnyValueSpec::Int { common, .. } => common,
            AnyValueSpec::Double { common, .. } => common,
            AnyValueSpec::String { common, .. } => common,
            AnyValueSpec::Strings { common, .. } => common,
            AnyValueSpec::Ints { common, .. } => common,
            AnyValueSpec::Doubles { common, .. } => common,
            AnyValueSpec::Booleans { common, .. } => common,
            AnyValueSpec::Map { common, .. } => common,
            AnyValueSpec::Bytes { common, .. } => common,
            AnyValueSpec::Undefined { common, .. } => common,
            AnyValueSpec::Enum { common, .. } => common,
        }
    }

    /// Returns true if the any value is required.
    #[must_use]
    pub fn is_required(&self) -> bool {
        matches!(
            self.common(),
            AnyValueCommonSpec { requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required), .. }
        )
    }

    /// Returns the id of the any value.
    #[must_use]
    pub fn id(&self) -> String {
        let AnyValueCommonSpec { id, .. } = self.common();
        id.clone()
    }

    /// Returns the brief of the any value.
    #[must_use]
    pub fn brief(&self) -> String {
        let AnyValueCommonSpec { brief, .. } = self.common();
        brief.clone()
    }

    /// Returns the note of the any value.
    #[must_use]
    pub fn note(&self) -> String {
        let AnyValueCommonSpec { note, .. } = self.common();
        note.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::attribute::ValueSpec;

    use super::*;

    #[test]
    fn test_anyvalue_field_type_display() {
        #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
        pub struct BodySpec {
            pub body: AnyValueSpec,
        }

        let map = AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: None,
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                deprecated: None,
            },
            fields: vec![
                AnyValueSpec::Enum {
                    common: AnyValueCommonSpec {
                        id: "id_enum".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    },
                    allow_custom_values: true,
                    members: vec![
                        EnumEntriesSpec {
                            id: "id".to_owned(),
                            value: ValueSpec::Int(42),
                            brief: Some("brief".to_owned()),
                            note: Some("note".to_owned()),
                            stability: None,
                            deprecated: None,
                        }
                    ]
                },
                AnyValueSpec::Map {
                    common: AnyValueCommonSpec {
                        id: "id_map".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    },
                    fields: vec![
                        AnyValueSpec::Int {
                            common: AnyValueCommonSpec {
                                id: "id_int".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                                deprecated: None,
                            }
                        },
                        AnyValueSpec::Bytes {
                            common: AnyValueCommonSpec {
                                id: "id_bytes".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                                deprecated: None,
                            }
                        },
                        AnyValueSpec::String {
                            common: AnyValueCommonSpec {
                                id: "id_string".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                deprecated: None,
                            }
                        },
                        AnyValueSpec::Boolean {
                            common: AnyValueCommonSpec {
                                id: "id_bool".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                deprecated: None,
                            }
                        },
                        AnyValueSpec::Map {
                            common: AnyValueCommonSpec {
                                id: "id_nested_map".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                deprecated: None,
                            },
                            fields: vec![
                                AnyValueSpec::Ints {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_int".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                        deprecated: None,
                                    }
                                },
                                AnyValueSpec::Doubles {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_bytes".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                        deprecated: None,
                                    }
                                },
                                AnyValueSpec::Strings {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_string".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                        deprecated: None,
                                    }
                                },
                                AnyValueSpec::Booleans {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_bool".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                                        deprecated: None,
                                    }
                                },
                            ],
                        }
                    ],
                },
                AnyValueSpec::Int {
                    common: AnyValueCommonSpec {
                        id: "id_int".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                },
                AnyValueSpec::Bytes {
                    common: AnyValueCommonSpec {
                        id: "id_bytes".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                },
                AnyValueSpec::String {
                    common: AnyValueCommonSpec {
                        id: "id_string".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
                        deprecated: None,
                    }
                },
                AnyValueSpec::Boolean {
                    common: AnyValueCommonSpec {
                        id: "id_bool".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                },
                AnyValueSpec::Double {
                    common: AnyValueCommonSpec {
                        id: "id_double".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                },
                AnyValueSpec::Doubles {
                    common: AnyValueCommonSpec {
                        id: "id_doubles".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                },
            ],
        };
        let body = BodySpec {
            body: map.clone(),
        };

        let expected_yaml = fs::read_to_string("data/expected/any_value.yaml").unwrap().replace("\r\n", "\n");
        assert_eq!(expected_yaml, format!("{}", serde_yaml::to_string(&body).unwrap()), "{}", expected_yaml);

        let expected_json = fs::read_to_string("data/expected/any_value.json").unwrap().replace("\r\n", "\n");
        assert_eq!(expected_json, format!("{}", serde_json::to_string(&body).unwrap()), "{}", expected_json);

        assert_eq!(
            format!(
                "{}",
                map
            ),
            "map<id>{ enum<id_enum>, map<id_map>{ int, byte[], string, boolean, map<id_nested_map>{ int[], double[], string[], boolean[] } }, int, byte[], string, boolean, double, double[] }"
        );

        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Boolean {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "boolean"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Int {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "int"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Double {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "double"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::String {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "string"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Strings {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "string[]"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Ints {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "int[]"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Doubles {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "double[]"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Booleans {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "boolean[]"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Bytes {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "byte[]"
        );
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Undefined {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    }
                }
            ),
            "undefined"
        );        
        assert_eq!(
            format!(
                "{}",
                AnyValueSpec::Enum {
                    common: AnyValueCommonSpec {
                        id: "id".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                        requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Optional),
                        deprecated: None,
                    },
                    allow_custom_values: true,
                    members: vec![
                        EnumEntriesSpec {
                            id: "id".to_owned(),
                            value: ValueSpec::Int(42),
                            brief: Some("brief".to_owned()),
                            note: Some("note".to_owned()),
                            stability: None,
                            deprecated: None,
                        }
                    ]
                }
            ),
            "enum<id>"
        );
    }
}

/// Specifies the default value for allow_custom_values.
fn default_as_true() -> bool {
    true
}