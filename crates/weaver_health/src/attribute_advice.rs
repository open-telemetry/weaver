// Pluggable advisors

use convert_case::{Boundary, Case, Casing};
use serde::Serialize;
use serde_json::Value;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec},
    stability::Stability,
};

use crate::{attribute_health::AttributeHealthChecker, sample::SampleAttribute};

/// The advisory level of an advice
#[derive(Debug, Clone, PartialEq, Serialize, PartialOrd, Ord, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Advisory {
    /// Useful context without action needed
    Information,
    /// Suggested change that would improve things
    Improvement,
    /// Something that breaks compliance rules
    Violation,
}

/// Represents a health check advice
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    /// The key of the advice e.g. "is_deprecated"
    pub key: String,
    /// The value of the advice e.g. "true"
    pub value: Value,
    /// The message of the advice e.g. "This attribute is deprecated"
    pub message: String,
    /// The advisory of the advice e.g. "violation"
    pub advisory: Advisory,
}

/// Provides advice on a sample attribute
pub trait Advisor {
    /// Provide advice on a sample attribute
    fn advise(
        &self,
        attribute: &SampleAttribute,
        health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice>;

    //TODO conclude(&self) -> Option<Vec<Advice>>;
    // Provide an overall summary of the advice e.g. LengthyAttributeNameAdvisor
    // could provide statistics on the length of the attribute names: min, max, avg
    // Each statistic would be an Advice with a key like "min_length", "max_length", "avg_length"
}

/// An advisor that checks if an attribute is deprecated
pub struct DeprecatedAdvisor;
impl Advisor for DeprecatedAdvisor {
    fn advise(
        &self,
        _attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        if let Some(attribute) = semconv_attribute {
            if attribute.deprecated.is_some() {
                Some(Advice {
                    key: "is_deprecated".to_owned(),
                    value: Value::Bool(true),
                    message: "Is deprecated".to_owned(),
                    advisory: Advisory::Violation,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// An advisor that checks if an attribute is stable from the stability field in the semantic convention
/// The value will be the stability level
pub struct StabilityAdvisor;
// TODO: Configurable Advisory level, strictly stable would mean Violation

impl Advisor for StabilityAdvisor {
    fn advise(
        &self,
        _attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        if let Some(attribute) = semconv_attribute {
            match attribute.stability {
                Some(ref stability) if *stability != Stability::Stable => Some(Advice {
                    key: "stability".to_owned(),
                    value: Value::String(stability.to_string()),
                    message: "Is not stable".to_owned(),
                    advisory: Advisory::Improvement,
                }),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// An advisor that checks if an attribute is in snake case
pub struct CorrectCaseAdvisor;
impl Advisor for CorrectCaseAdvisor {
    fn advise(
        &self,
        attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        // Don't provide advice if the attribute is a match
        if semconv_attribute.is_some() {
            return None;
        }

        let is_snake_case = attribute
            .name
            .without_boundaries(&Boundary::digits())
            .to_case(Case::Snake)
            == attribute.name;
        if !is_snake_case {
            Some(Advice {
                key: "correct_case".to_owned(),
                value: Value::Bool(false),
                message: "Is not in snake case".to_owned(),
                advisory: Advisory::Violation,
            })
        } else {
            None
        }
    }
}

/// An advisor that checks if an attribute has a namespace - a prefix before the first dot
pub struct HasNamespaceAdvisor;
impl Advisor for HasNamespaceAdvisor {
    fn advise(
        &self,
        attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        // Don't provide advice if the attribute is a match
        if semconv_attribute.is_some() {
            return None;
        }

        let has_namespace = attribute.name.contains('.');
        if !has_namespace {
            Some(Advice {
                key: "has_namespace".to_owned(),
                value: Value::Bool(false),
                message: "Does not have a namespace".to_owned(),
                advisory: Advisory::Improvement,
            })
        } else {
            None
        }
    }
}

/// An advisor that checks if an attribute has the correct type
pub struct TypeAdvisor;
impl Advisor for TypeAdvisor {
    fn advise(
        &self,
        attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        // Only provide advice if the attribute is a match and the type is present
        match (semconv_attribute, attribute.r#type.as_ref()) {
            (Some(semconv_attribute), Some(attribute_type)) => {
                let semconv_attribute_type = match &semconv_attribute.r#type {
                    AttributeType::PrimitiveOrArray(primitive_or_array_type_spec) => {
                        primitive_or_array_type_spec
                    }
                    AttributeType::Template(template_type_spec) => &match template_type_spec {
                        TemplateTypeSpec::Boolean => PrimitiveOrArrayTypeSpec::Boolean,
                        TemplateTypeSpec::Int => PrimitiveOrArrayTypeSpec::Int,
                        TemplateTypeSpec::Double => PrimitiveOrArrayTypeSpec::Double,
                        TemplateTypeSpec::String => PrimitiveOrArrayTypeSpec::String,
                        TemplateTypeSpec::Strings => PrimitiveOrArrayTypeSpec::Strings,
                        TemplateTypeSpec::Ints => PrimitiveOrArrayTypeSpec::Ints,
                        TemplateTypeSpec::Doubles => PrimitiveOrArrayTypeSpec::Doubles,
                        TemplateTypeSpec::Booleans => PrimitiveOrArrayTypeSpec::Booleans,
                    },
                    AttributeType::Enum { .. } => {
                        // Special case: Enum variants can be either string or int
                        if attribute_type != &PrimitiveOrArrayTypeSpec::String
                            && attribute_type != &PrimitiveOrArrayTypeSpec::Int
                        {
                            return Some(Advice {
                                key: "type".to_owned(),
                                value: Value::String(attribute_type.to_string()),
                                message: "Type should be `string` or `int`".to_owned(),
                                advisory: Advisory::Violation,
                            });
                        } else {
                            return None;
                        }
                    }
                };

                if attribute_type != semconv_attribute_type {
                    Some(Advice {
                        key: "type".to_owned(),
                        value: Value::String(attribute_type.to_string()),
                        message: format!("Type should be `{}`", semconv_attribute_type),
                        advisory: Advisory::Violation,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// An advisor that reports if the given value is not a defined variant in the enum
pub struct EnumAdvisor;
impl Advisor for EnumAdvisor {
    fn advise(
        &self,
        attribute: &SampleAttribute,
        _health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        // Only provide advice if the semconv_attribute is an enum and the attribute has a value and type
        match (
            semconv_attribute,
            attribute.value.as_ref(),
            attribute.r#type.as_ref(),
        ) {
            (Some(semconv_attribute), Some(attribute_value), Some(attribute_type)) => {
                if let AttributeType::Enum { members, .. } = &semconv_attribute.r#type {
                    let mut is_found = false;
                    for member in members {
                        if match attribute_type {
                            PrimitiveOrArrayTypeSpec::Int => {
                                if let Some(int_value) = attribute_value.as_i64() {
                                    member.value == ValueSpec::Int(int_value)
                                } else {
                                    false
                                }
                            }
                            PrimitiveOrArrayTypeSpec::String => {
                                if let Some(string_value) = attribute_value.as_str() {
                                    member.value == ValueSpec::String(string_value.to_owned())
                                } else {
                                    false
                                }
                            }
                            _ => {
                                // Any other type is not supported - the TypeAdvisor should have already caught this
                                return None;
                            }
                        } {
                            is_found = true;
                            break;
                        }
                    }

                    if !is_found {
                        return Some(Advice {
                            key: "enum".to_owned(),
                            value: attribute_value.clone(),
                            message: "Is not a defined variant".to_owned(),
                            advisory: Advisory::Information,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }
}
