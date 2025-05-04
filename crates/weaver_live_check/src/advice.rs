// SPDX-License-Identifier: Apache-2.0

//! Builtin advisors

use std::{collections::BTreeMap, path::PathBuf, rc::Rc};

use serde::Serialize;
use serde_json::Value;
use weaver_checker::{
    violation::{Advice, AdviceLevel, Violation},
    Engine,
};
use weaver_forge::{jq, registry::ResolvedGroup};
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, PrimitiveOrArrayTypeSpec, TemplateTypeSpec, ValueSpec},
    deprecated::Deprecated,
    stability::Stability,
};

use crate::{live_checker::LiveChecker, Error, SampleRef};

/// Embedded default live check rego policies
pub const DEFAULT_LIVE_CHECK_REGO: &str =
    include_str!("../../../defaults/policies/live_check_advice/otel.rego");

/// Default live check rego policy path - used in error messages
pub const DEFAULT_LIVE_CHECK_REGO_POLICY_PATH: &str =
    "defaults/policies/live_check_advice/otel.rego";

/// Embedded default live check jq preprocessor
pub const DEFAULT_LIVE_CHECK_JQ: &str = include_str!("../../../defaults/jq/advice.jq");

/// Provides advice on a sample
pub trait Advisor {
    /// Provide advice on a sample
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        registry_attribute: Option<&Rc<Attribute>>,
        registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error>;
}

fn deprecated_to_value(deprecated: &Deprecated) -> Value {
    match deprecated {
        Deprecated::Renamed { .. } => Value::String("renamed".to_owned()),
        Deprecated::Obsoleted { .. } => Value::String("obsoleted".to_owned()),
        Deprecated::Uncategorized { .. } => Value::String("uncategorized".to_owned()),
    }
}

/// An advisor that checks if an attribute is deprecated
pub struct DeprecatedAdvisor;
impl Advisor for DeprecatedAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        registry_attribute: Option<&Rc<Attribute>>,
        registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error> {
        match sample {
            SampleRef::Attribute(_sample_attribute) => {
                let mut advices = Vec::new();
                if let Some(attribute) = registry_attribute {
                    if let Some(deprecated) = &attribute.deprecated {
                        advices.push(Advice {
                            advice_type: "deprecated".to_owned(),
                            value: deprecated_to_value(deprecated),
                            message: deprecated.to_string(),
                            advice_level: AdviceLevel::Violation,
                        });
                    }
                }
                Ok(advices)
            }
            SampleRef::Metric(_sample_metric) => {
                let mut advices = Vec::new();
                if let Some(group) = registry_group {
                    if let Some(deprecated) = &group.deprecated {
                        advices.push(Advice {
                            advice_type: "deprecated".to_owned(),
                            value: deprecated_to_value(deprecated),
                            message: deprecated.to_string(),
                            advice_level: AdviceLevel::Violation,
                        });
                    }
                }
                Ok(advices)
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// An advisor that checks if an attribute is stable from the stability field in the semantic convention
/// The value will be the stability level
pub struct StabilityAdvisor;
// TODO: Configurable Advice level, strictly stable would mean Violation

impl Advisor for StabilityAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        registry_attribute: Option<&Rc<Attribute>>,
        registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error> {
        match sample {
            SampleRef::Attribute(_sample_attribute) => {
                let mut advices = Vec::new();
                if let Some(attribute) = registry_attribute {
                    match attribute.stability {
                        Some(ref stability) if *stability != Stability::Stable => {
                            advices.push(Advice {
                                advice_type: "stability".to_owned(),
                                value: Value::String(stability.to_string()),
                                message: "Is not stable".to_owned(),
                                advice_level: AdviceLevel::Improvement,
                            });
                        }
                        _ => {}
                    }
                }
                Ok(advices)
            }
            SampleRef::Metric(_sample_metric) => {
                let mut advices = Vec::new();
                if let Some(group) = registry_group {
                    match group.stability {
                        Some(ref stability) if *stability != Stability::Stable => {
                            advices.push(Advice {
                                advice_type: "stability".to_owned(),
                                value: Value::String(stability.to_string()),
                                message: "Is not stable".to_owned(),
                                advice_level: AdviceLevel::Improvement,
                            });
                        }
                        _ => {}
                    }
                }
                Ok(advices)
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// An advisor that checks if an attribute has the correct type
pub struct TypeAdvisor;
impl Advisor for TypeAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        registry_attribute: Option<&Rc<Attribute>>,
        registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                // Only provide advice if the attribute is a match and the type is present
                match (registry_attribute, sample_attribute.r#type.as_ref()) {
                    (Some(semconv_attribute), Some(attribute_type)) => {
                        let semconv_attribute_type = match &semconv_attribute.r#type {
                            AttributeType::PrimitiveOrArray(primitive_or_array_type_spec) => {
                                primitive_or_array_type_spec
                            }
                            AttributeType::Template(template_type_spec) => {
                                &match template_type_spec {
                                    TemplateTypeSpec::Boolean => PrimitiveOrArrayTypeSpec::Boolean,
                                    TemplateTypeSpec::Int => PrimitiveOrArrayTypeSpec::Int,
                                    TemplateTypeSpec::Double => PrimitiveOrArrayTypeSpec::Double,
                                    TemplateTypeSpec::String => PrimitiveOrArrayTypeSpec::String,
                                    TemplateTypeSpec::Any => PrimitiveOrArrayTypeSpec::Any,
                                    TemplateTypeSpec::Strings => PrimitiveOrArrayTypeSpec::Strings,
                                    TemplateTypeSpec::Ints => PrimitiveOrArrayTypeSpec::Ints,
                                    TemplateTypeSpec::Doubles => PrimitiveOrArrayTypeSpec::Doubles,
                                    TemplateTypeSpec::Booleans => {
                                        PrimitiveOrArrayTypeSpec::Booleans
                                    }
                                }
                            }
                            AttributeType::Enum { .. } => {
                                // Special case: Enum variants can be either string or int
                                if attribute_type != &PrimitiveOrArrayTypeSpec::String
                                    && attribute_type != &PrimitiveOrArrayTypeSpec::Int
                                {
                                    return Ok(vec![Advice {
                                        advice_type: "type_mismatch".to_owned(),
                                        value: Value::String(attribute_type.to_string()),
                                        message: "Type should be `string` or `int`".to_owned(),
                                        advice_level: AdviceLevel::Violation,
                                    }]);
                                } else {
                                    return Ok(Vec::new());
                                }
                            }
                        };

                        if !attribute_type.is_compatible(semconv_attribute_type) {
                            Ok(vec![Advice {
                                advice_type: "type_mismatch".to_owned(),
                                value: Value::String(attribute_type.to_string()),
                                message: format!("Type should be `{}`", semconv_attribute_type),
                                advice_level: AdviceLevel::Violation,
                            }])
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    _ => Ok(Vec::new()),
                }
            }
            SampleRef::Metric(sample_metric) => {
                let mut advice_list = Vec::new();
                if let Some(semconv_metric) = registry_group {
                    if let Some(semconv_instrument) = &semconv_metric.instrument {
                        if semconv_instrument != &sample_metric.instrument {
                            advice_list.push(Advice {
                                advice_type: "instrument_mismatch".to_owned(),
                                value: Value::String(sample_metric.instrument.to_string()),
                                message: format!("Instrument should be `{}`", semconv_instrument),
                                advice_level: AdviceLevel::Violation,
                            });
                        }
                    }
                    if let Some(semconv_unit) = &semconv_metric.unit {
                        if semconv_unit != &sample_metric.unit {
                            advice_list.push(Advice {
                                advice_type: "unit_mismatch".to_owned(),
                                value: Value::String(sample_metric.unit.clone()),
                                message: format!("Unit should be `{}`", semconv_unit),
                                advice_level: AdviceLevel::Violation,
                            });
                        }
                    }
                }
                Ok(advice_list)
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
        sample: SampleRef<'_>,
        registry_attribute: Option<&Rc<Attribute>>,
        _registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                // Only provide advice if the registry_attribute is an enum and the attribute has a value and type
                match (
                    registry_attribute,
                    sample_attribute.value.as_ref(),
                    sample_attribute.r#type.as_ref(),
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
                                            member.value
                                                == ValueSpec::String(string_value.to_owned())
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
                                    advice_type: "undefined_enum_variant".to_owned(),
                                    value: attribute_value.clone(),
                                    message: "Is not a defined variant".to_owned(),
                                    advice_level: AdviceLevel::Information,
                                }]);
                            }
                        }
                        Ok(Vec::new())
                    }
                    _ => Ok(Vec::new()),
                }
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
        live_checker: &LiveChecker,
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
                .add_policy(DEFAULT_LIVE_CHECK_REGO_POLICY_PATH, DEFAULT_LIVE_CHECK_REGO)
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

    fn check<T>(&mut self, input: T) -> Result<Vec<Advice>, Error>
    where
        T: Serialize,
    {
        self.engine
            .set_input(&input)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        let violations = self
            .engine
            .check(weaver_checker::PolicyStage::LiveCheckAdvice)
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

impl Advisor for RegoAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        _registry_attribute: Option<&Rc<Attribute>>,
        _registry_group: Option<&Rc<ResolvedGroup>>,
    ) -> Result<Vec<Advice>, Error> {
        self.check(sample)
    }
}
