// Pluggable advisors

use convert_case::{Boundary, Case, Casing};
use serde::Serialize;
use serde_json::Value;
use weaver_resolved_schema::attribute::Attribute;

use crate::{attribute_health::AttributeHealthChecker, sample::SampleAttribute};

/// Represents a health check advice
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    /// The key of the advice e.g. "is_deprecated"
    pub key: String,
    /// The value of the advice e.g. "true"
    pub value: Value,
    /// The message of the advice e.g. "This attribute is deprecated"
    pub message: String,
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
                    message: "This attribute is deprecated".to_owned(),
                })
            } else {
                None
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
                message: "This attribute is not in snake case".to_owned(),
            })
        } else {
            None
        }
    }
}
