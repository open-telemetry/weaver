// Builtin advisors

use serde_json::Value;
use weaver_checker::{
    violation::{Advice, Advisory, Violation},
    Engine,
};
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec},
    deprecated::Deprecated,
    stability::Stability,
};

use crate::{attribute_health::AttributeHealthChecker, sample::SampleAttribute, Error};

/// Provides advice on a sample attribute
pub trait Advisor {
    /// Provide advice on a sample attribute
    fn advise(
        &mut self,
        attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error>;

    //TODO conclude(&self) -> Option<Vec<Advice>>;
    // Provide an overall summary of the advice e.g. LengthyAttributeNameAdvisor
    // could provide statistics on the length of the attribute names: min, max, avg
    // Each statistic would be an Advice with a key like "min_length", "max_length", "avg_length"
}

/// An advisor that checks if an attribute is deprecated
pub struct DeprecatedAdvisor;
impl Advisor for DeprecatedAdvisor {
    fn advise(
        &mut self,
        _attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error> {
        let mut advices = Vec::new();
        if let Some(attribute) = semconv_attribute {
            if let Some(deprecated) = &attribute.deprecated {
                advices.push(Advice {
                    key: "deprecated".to_owned(),
                    value: match deprecated {
                        Deprecated::Renamed { .. } => Value::String("renamed".to_owned()),
                        Deprecated::Obsoleted { .. } => Value::String("obsoleted".to_owned()),
                        Deprecated::Uncategorized { .. } => {
                            Value::String("uncategorized".to_owned())
                        }
                    },
                    message: deprecated.to_string(),
                    advisory: Advisory::Violation,
                });
            }
        }
        Ok(advices)
    }
}

/// An advisor that checks if an attribute is stable from the stability field in the semantic convention
/// The value will be the stability level
pub struct StabilityAdvisor;
// TODO: Configurable Advisory level, strictly stable would mean Violation

impl Advisor for StabilityAdvisor {
    fn advise(
        &mut self,
        _attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error> {
        let mut advices = Vec::new();
        if let Some(attribute) = semconv_attribute {
            match attribute.stability {
                Some(ref stability) if *stability != Stability::Stable => {
                    advices.push(Advice {
                        key: "stability".to_owned(),
                        value: Value::String(stability.to_string()),
                        message: "Is not stable".to_owned(),
                        advisory: Advisory::Improvement,
                    });
                }
                _ => {}
            }
        }
        Ok(advices)
    }
}

/// An advisor that checks if an attribute has the correct type
pub struct TypeAdvisor;
impl Advisor for TypeAdvisor {
    fn advise(
        &mut self,
        attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error> {
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
                            return Ok(vec![Advice {
                                key: "type_mismatch".to_owned(),
                                value: Value::String(attribute_type.to_string()),
                                message: "Type should be `string` or `int`".to_owned(),
                                advisory: Advisory::Violation,
                            }]);
                        } else {
                            return Ok(Vec::new());
                        }
                    }
                };

                if attribute_type != semconv_attribute_type {
                    Ok(vec![Advice {
                        key: "type_mismatch".to_owned(),
                        value: Value::String(attribute_type.to_string()),
                        message: format!("Type should be `{}`", semconv_attribute_type),
                        advisory: Advisory::Violation,
                    }])
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// An advisor that reports if the given value is not a defined variant in the enum
pub struct EnumAdvisor;
impl Advisor for EnumAdvisor {
    fn advise(
        &mut self,
        attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error> {
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
                                return Ok(Vec::new());
                            }
                        } {
                            is_found = true;
                            break;
                        }
                    }

                    if !is_found {
                        return Ok(vec![Advice {
                            key: "undefined_enum_variant".to_owned(),
                            value: attribute_value.clone(),
                            message: "Is not a defined variant".to_owned(),
                            advisory: Advisory::Information,
                        }]);
                    }
                }
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// An advisor which runs a rego policy on the attribute
pub struct RegoAdvisor {
    engine: Engine,
}
impl RegoAdvisor {
    /// Create a new RegoAdvisor
    #[must_use]
    pub fn new(health_checker: &AttributeHealthChecker, policy_path: &str) -> Self {
        let mut engine = Engine::new();
        let _ = engine
            .add_policy_from_file(policy_path)
            .expect("Failed to load policy file"); // TODO: handle error

        engine
            .add_data(health_checker)
            .expect("Failed to add health checker data");

        RegoAdvisor { engine }
    }
}
impl Advisor for RegoAdvisor {
    fn advise(
        &mut self,
        attribute: &SampleAttribute,
        _semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error> {
        self.engine
            .set_input(attribute)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        let violations = self
            .engine
            .check(weaver_checker::PolicyStage::BeforeResolution)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        // Extract advice from violations
        Ok(violations
            .iter()
            .filter_map(|violation| {
                if let Violation::Advice(advice) = violation {
                    Some(advice.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<Advice>>())
    }
}
