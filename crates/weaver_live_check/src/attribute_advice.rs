// SPDX-License-Identifier: Apache-2.0

//! Builtin advisors

use std::{collections::BTreeMap, path::PathBuf};

use serde_json::Value;
use weaver_checker::{
    violation::{Advice, Advisory, Violation},
    Engine,
};
use weaver_forge::jq;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec},
    deprecated::Deprecated,
    stability::Stability,
};

use crate::{attribute_live_check::AttributeLiveChecker, sample::SampleAttribute, Error};

/// Embedded default live check rego policies
pub const DEFAULT_LIVE_CHECK_REGO: &str =
    include_str!("../../../defaults/policies/advice/otel.rego");

/// Embedded default live check jq preprocessor
pub const DEFAULT_LIVE_CHECK_JQ: &str = include_str!("../../../defaults/jq/advice.jq");

/// Provides advice on a sample attribute
pub trait Advisor {
    /// Provide advice on a sample attribute
    fn advise(
        &mut self,
        attribute: &SampleAttribute,
        semconv_attribute: Option<&Attribute>,
    ) -> Result<Vec<Advice>, Error>;
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
    pub fn new(
        live_checker: &AttributeLiveChecker,
        policy_dir: &Option<PathBuf>,
        jq_preprocessor: &Option<PathBuf>,
    ) -> Result<Self, Error> {
        let mut engine = Engine::new();
        if let Some(path) = policy_dir {
            let _ = engine
                .add_policies(path, "*.rego")
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        } else {
            let _ = engine
                .add_policy(
                    "defaults/policies/advice/otel.rego",
                    DEFAULT_LIVE_CHECK_REGO,
                )
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        }

        // If there is a jq preprocessor then pass the live_checker data through it before adding it to the engine
        // Otherwise use the default jq preprocessor
        let jq_filter = if let Some(path) = jq_preprocessor {
            std::fs::read_to_string(path).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?
        } else {
            DEFAULT_LIVE_CHECK_JQ.to_owned()
        };

        let jq_result = jq::execute_jq(
            &serde_json::to_value(live_checker).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?,
            &jq_filter,
            &BTreeMap::new(),
        )
        .map_err(|e| Error::AdviceError {
            error: e.to_string(),
        })?;

        engine
            .add_data(&jq_result)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;

        Ok(RegoAdvisor { engine })
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
            .check(weaver_checker::PolicyStage::Advice)
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
