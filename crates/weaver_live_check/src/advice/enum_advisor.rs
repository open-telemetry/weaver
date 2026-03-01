// SPDX-License-Identifier: Apache-2.0

//! Enum value validation advisor

use serde_json::json;
use std::rc::Rc;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_semconv::attribute::{AttributeType, PrimitiveOrArrayTypeSpec, ValueSpec};

use super::{Advisor, FindingBuilder};
use crate::{
    otlp_logger::OtlpEmitter, Error, FindingId, Sample, SampleRef, VersionedAttribute,
    VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, ATTRIBUTE_VALUE_ADVICE_CONTEXT_KEY,
};

/// An advisor that reports if the given value is not a defined variant in the enum
pub struct EnumAdvisor;

impl Advisor for EnumAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        _registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                // Only provide advice if the registry_attribute is an enum and the attribute has a value and type
                match (
                    registry_attribute,
                    sample_attribute.value.as_ref(),
                    sample_attribute.r#type.as_ref(),
                ) {
                    (Some(semconv_attribute), Some(attribute_value), Some(attribute_type)) => {
                        if let AttributeType::Enum { members, .. } = semconv_attribute.r#type() {
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
                                let finding = FindingBuilder::new(FindingId::UndefinedEnumVariant)
                                    .context(json!({
                                        ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: &sample_attribute.name,
                                        ATTRIBUTE_VALUE_ADVICE_CONTEXT_KEY: attribute_value,
                                    }))
                                    .message(format!(
                                    "Enum attribute '{}' has value '{}' which is not documented.",
                                    sample_attribute.name,
                                    attribute_value
                                        .as_str()
                                        .unwrap_or(&attribute_value.to_string())
                                ))
                                    .level(FindingLevel::Information)
                                    .signal(signal)
                                    .build_and_emit(&sample, otlp_emitter.as_deref(), signal);

                                return Ok(vec![finding]);
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
