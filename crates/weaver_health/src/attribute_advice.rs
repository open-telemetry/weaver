// Pluggable advisors

use convert_case::{Boundary, Case, Casing};
use serde::Serialize;
use serde_json::Value;
use weaver_resolved_schema::attribute::Attribute;

use crate::{attribute_health::AttributeHealthChecker, sample::SampleAttribute};

/// The severity of the advice
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Severity {
    /// An error
    Error,
    /// A warning
    Warning,
    /// Information
    Info,
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
    /// The severity of the advice e.g. "error"
    pub severity: Severity,
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
                    severity: Severity::Error,
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
                message: "Is not in snake case".to_owned(),
                severity: Severity::Error,
            })
        } else {
            None
        }
    }
}
