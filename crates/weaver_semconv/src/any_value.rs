// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! AnyValue specification.

use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attribute::Examples;
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
                write!(f, "map{{{}}}", entries)
            }
            AnyValueSpec::Bytes { .. } => write!(f, "byte[]"),
            AnyValueSpec::Undefined { .. } => write!(f, "undefined"),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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
            },
            fields: vec![
                AnyValueSpec::Map {
                    common: AnyValueCommonSpec {
                        id: "id_map".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    },
                    fields: vec![
                        AnyValueSpec::Int {
                            common: AnyValueCommonSpec {
                                id: "id_int".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                            }
                        },
                        AnyValueSpec::Bytes {
                            common: AnyValueCommonSpec {
                                id: "id_bytes".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                            }
                        },
                        AnyValueSpec::String {
                            common: AnyValueCommonSpec {
                                id: "id_string".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                            }
                        },
                        AnyValueSpec::Boolean {
                            common: AnyValueCommonSpec {
                                id: "id_bool".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                            }
                        },
                        AnyValueSpec::Map {
                            common: AnyValueCommonSpec {
                                id: "id_nested_map".to_owned(),
                                brief: "brief".to_owned(),
                                note: "note".to_owned(),
                                stability: None,
                                examples: None,
                            },
                            fields: vec![
                                AnyValueSpec::Ints {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_int".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                    }
                                },
                                AnyValueSpec::Doubles {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_bytes".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                    }
                                },
                                AnyValueSpec::Strings {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_string".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
                                    }
                                },
                                AnyValueSpec::Booleans {
                                    common: AnyValueCommonSpec {
                                        id: "id_nested_bool".to_owned(),
                                        brief: "brief".to_owned(),
                                        note: "note".to_owned(),
                                        stability: None,
                                        examples: None,
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
                    }
                },
                AnyValueSpec::Bytes {
                    common: AnyValueCommonSpec {
                        id: "id_bytes".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    }
                },
                AnyValueSpec::String {
                    common: AnyValueCommonSpec {
                        id: "id_string".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    }
                },
                AnyValueSpec::Boolean {
                    common: AnyValueCommonSpec {
                        id: "id_bool".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    }
                },
                AnyValueSpec::Double {
                    common: AnyValueCommonSpec {
                        id: "id_double".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    }
                },
                AnyValueSpec::Doubles {
                    common: AnyValueCommonSpec {
                        id: "id_doubles".to_owned(),
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        stability: None,
                        examples: None,
                    }
                },
            ],
        };
        let body = BodySpec {
            body: map,
        };

        let expected_yaml = fs::read_to_string("data/expected/any_value.yaml").unwrap().replace("\r\n", "\n");
        assert_eq!(expected_yaml, format!("{}", serde_yaml::to_string(&body).unwrap()), "{}", expected_yaml);

        let expected_json = fs::read_to_string("data/expected/any_value.json").unwrap().replace("\r\n", "\n");
        assert_eq!(expected_json, format!("{}", serde_json::to_string(&body).unwrap()), "{}", expected_json);
    }
}