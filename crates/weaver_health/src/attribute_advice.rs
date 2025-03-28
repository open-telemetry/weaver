// Pluggable advisors

use std::collections::HashSet;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec},
    deprecated::Deprecated,
    stability::Stability,
};

use crate::{attribute_health::AttributeHealthChecker, sample::SampleAttribute};

/// The advisory level of an advice
#[derive(Debug, Clone, PartialEq, Serialize, PartialOrd, Ord, Eq, Hash)]
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
            attribute.deprecated.as_ref().map(|deprecated| Advice {
                key: "deprecated".to_owned(),
                value: match deprecated {
                    Deprecated::Renamed { .. } => Value::String("renamed".to_owned()),
                    Deprecated::Obsoleted { .. } => Value::String("obsoleted".to_owned()),
                    Deprecated::Uncategorized { .. } => Value::String("uncategorized".to_owned()),
                },
                message: deprecated.to_string(),
                advisory: Advisory::Violation,
            })
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

/// An advisor that checks if an attribute matches name formatting rules
pub struct NameFormatAdvisor {
    regex: Regex,
}
impl NameFormatAdvisor {
    #[must_use]
    /// Create a new NameFormatAdvisor
    pub fn new(pattern: &str) -> Self {
        NameFormatAdvisor {
            regex: Regex::new(pattern).expect("regex pattern must be valid"),
        }
    }
}
impl Default for NameFormatAdvisor {
    fn default() -> Self {
        Self::new(r"^[a-z][a-z0-9]*([._][a-z0-9]+)*$")
    }
}

impl Advisor for NameFormatAdvisor {
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

        if !self.regex.is_match(&attribute.name) {
            Some(Advice {
                key: "invalid_format".to_owned(),
                value: Value::String(attribute.name.clone()),
                message: "Does not match name formatting rules".to_owned(),
                advisory: Advisory::Violation,
            })
        } else {
            None
        }
    }
}

/// An advisor that provides advice on the namespace of an attribute
pub struct NamespaceAdvisor {
    namespace_separator: char,
    semconv_namespaces: HashSet<String>,
}
impl NamespaceAdvisor {
    #[must_use]
    /// Create a new NamespaceAdvisor
    pub fn new(namespace_separator: char, health_checker: &AttributeHealthChecker) -> Self {
        let mut semconv_namespaces = HashSet::new();
        for group in &health_checker.registry.groups {
            for attribute in &group.attributes {
                // Extract namespace (everything to the left of the last separator)
                // repeat until the last separator is found
                let mut name = attribute.name.clone();
                while let Some(last_separator_pos) = name.rfind(namespace_separator) {
                    let namespace = name[..last_separator_pos].to_string();
                    let _ = semconv_namespaces.insert(namespace);
                    name = name[..last_separator_pos].to_string();
                }
            }
        }
        NamespaceAdvisor {
            namespace_separator,
            semconv_namespaces,
        }
    }

    /// Find a namespace in the registry
    #[must_use]
    fn find_namespace(&self, namespace: &str) -> Option<String> {
        let mut namespace = namespace.to_owned();
        while !self.semconv_namespaces.contains(&namespace) {
            if let Some(last_dot_pos) = namespace.rfind('.') {
                namespace = namespace[..last_dot_pos].to_string();
            } else {
                return None;
            }
        }
        Some(namespace)
    }

    /// Find an attribute from a namespace search
    #[must_use]
    fn find_attribute_from_namespace(
        &self,
        namespace: &str,
        health_checker: &AttributeHealthChecker,
    ) -> Option<Attribute> {
        if let Some(attribute) = health_checker.find_attribute(namespace) {
            Some(attribute.clone())
        } else if let Some(last_separator_pos) = namespace.rfind(self.namespace_separator) {
            let new_namespace = &namespace[..last_separator_pos];
            self.find_attribute_from_namespace(new_namespace, health_checker)
        } else {
            None
        }
    }
}

impl Advisor for NamespaceAdvisor {
    fn advise(
        &self,
        attribute: &SampleAttribute,
        health_checker: &AttributeHealthChecker,
        semconv_attribute: Option<&Attribute>,
    ) -> Option<Advice> {
        // Don't provide advice if the attribute is a match
        if semconv_attribute.is_some() {
            return None;
        }

        if let Some(last_separator_pos) = attribute.name.rfind(self.namespace_separator) {
            let namespace = attribute.name[..last_separator_pos].to_string();

            // Has a namespace that matches an existing attribute
            if let Some(found_attr) = self.find_attribute_from_namespace(&namespace, health_checker)
            {
                return Some(Advice {
                    key: "illegal_namespace".to_owned(),
                    value: Value::String(found_attr.name),
                    message: "Namespace matches existing attribute".to_owned(),
                    advisory: Advisory::Violation,
                });
            }

            // Extends an existing namespace
            if let Some(existing_namespace) = self.find_namespace(&namespace) {
                return Some(Advice {
                    key: "extends_namespace".to_owned(),
                    value: Value::String(existing_namespace),
                    message: "Extends existing namespace".to_owned(),
                    advisory: Advisory::Information,
                });
            }
        } else {
            // Does not have a namespace
            return Some(Advice {
                key: "missing_namespace".to_owned(),
                value: Value::String(attribute.name.clone()),
                message: "Does not have a namespace".to_owned(),
                advisory: Advisory::Improvement,
            });
        }

        None
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
                                key: "type_mismatch".to_owned(),
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
                        key: "type_mismatch".to_owned(),
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
                            key: "undefined_enum_variant".to_owned(),
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
