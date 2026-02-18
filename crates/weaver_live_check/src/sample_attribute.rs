// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample attributes

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use weaver_checker::FindingLevel;
use weaver_semconv::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};

use crate::{
    advice::FindingBuilder, live_checker::LiveChecker, Error, FindingId, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef, VersionedSignal,
    ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY,
};

/// Represents a sample telemetry attribute parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SampleAttribute {
    /// The name of the attribute
    pub name: String,
    /// The value of the attribute
    pub value: Option<Value>,
    /// The type of the attribute's value
    /// This may be available in the upstream source, an o11y vendor for example
    /// or inferred from the value
    pub r#type: Option<PrimitiveOrArrayTypeSpec>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl<'de> Deserialize<'de> for SampleAttribute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SampleAttributeHelper {
            name: String,
            value: Option<Value>,
            r#type: Option<PrimitiveOrArrayTypeSpec>,
        }

        let helper = SampleAttributeHelper::deserialize(deserializer)?;

        // If type is None but value exists, infer the type
        let inferred_type = match (&helper.r#type, &helper.value) {
            (None, Some(value)) => Self::infer_type(value),
            _ => helper.r#type,
        };

        Ok(SampleAttribute {
            name: helper.name,
            value: helper.value,
            r#type: inferred_type,
            live_check_result: None,
        })
    }
}

impl TryFrom<&str> for SampleAttribute {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            // exit on empty line
            return Err(Self::Error::IngestEmptyLine);
        }
        // If the line follows the pattern name=value, split it
        if let Some((name, value)) = trimmed.split_once('=') {
            let json_value = serde_json::from_str(value.trim()).unwrap_or(json!(value.trim()));
            let r#type = SampleAttribute::infer_type(&json_value);
            let sample_attribute = SampleAttribute {
                name: name.trim().to_owned(),
                value: Some(json_value),
                r#type,
                live_check_result: None,
            };
            return Ok(sample_attribute);
        }
        // If the line is just a name, return it
        Ok(SampleAttribute {
            name: trimmed.to_owned(),
            value: None,
            r#type: None,
            live_check_result: None,
        })
    }
}

impl SampleAttribute {
    /// Infer the type of the attribute from the value
    #[must_use]
    pub fn infer_type(value: &Value) -> Option<PrimitiveOrArrayTypeSpec> {
        match value {
            Value::Null => None,
            Value::Bool(_) => Some(PrimitiveOrArrayTypeSpec::Boolean),
            Value::Number(_) => {
                // Int or double?
                if value.is_i64() || value.is_u64() {
                    Some(PrimitiveOrArrayTypeSpec::Int)
                } else {
                    Some(PrimitiveOrArrayTypeSpec::Double)
                }
            }
            Value::String(_) => Some(PrimitiveOrArrayTypeSpec::String),
            Value::Array(_) => {
                // Strings, Ints, Doubles or Booleans?
                // Get the first non-null element to check types
                if let Some(arr) = value.as_array() {
                    let first_value = arr.iter().find(|v| !v.is_null());

                    match first_value {
                        Some(first) => {
                            // Check if all elements match the type of the first element
                            if first.is_boolean()
                                && arr.iter().all(|v| v.is_null() || v.is_boolean())
                            {
                                Some(PrimitiveOrArrayTypeSpec::Booleans)
                            } else if (first.is_i64() || first.is_u64())
                                && arr.iter().all(|v| v.is_null() || v.is_i64() || v.is_u64())
                            {
                                Some(PrimitiveOrArrayTypeSpec::Ints)
                            } else if first.is_number()
                                && arr.iter().all(|v| v.is_null() || v.is_number())
                            {
                                Some(PrimitiveOrArrayTypeSpec::Doubles)
                            } else if first.is_string()
                                && arr.iter().all(|v| v.is_null() || v.is_string())
                            {
                                Some(PrimitiveOrArrayTypeSpec::Strings)
                            } else {
                                // Mixed or unsupported types
                                None
                            }
                        }
                        None => None, // Empty or all-null array
                    }
                } else {
                    None
                }
            }
            Value::Object(_) => None, // Unsupported
        }
    }

    fn update_stats(&mut self, stats: &mut LiveCheckStatistics) {
        stats.inc_entity_count("attribute");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        let mut seen_attribute_name = self.name.clone();
        if let Some(result) = &mut self.live_check_result {
            for advice in &mut result.all_advice {
                // If the advice is a template, adjust the name
                if advice.id == FindingId::TemplateAttribute.to_string() {
                    if let Some(template_name) = advice.context["template_name"].as_str() {
                        seen_attribute_name = template_name.to_owned();
                    }
                }
            }
        }
        stats.add_attribute_name_to_coverage(seen_attribute_name);
    }
}

impl LiveCheckRunner for SampleAttribute {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        // find the attribute in the registry
        let semconv_attribute = {
            if let Some(attribute) = live_checker.find_attribute(&self.name) {
                Some(attribute)
            } else {
                live_checker.find_template(&self.name)
            }
        };
        if semconv_attribute.is_none() {
            let finding = FindingBuilder::new(FindingId::MissingAttribute)
                .context(json!({ ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: self.name.clone() }))
                .message(format!(
                    "Attribute '{}' does not exist in the registry.",
                    self.name
                ))
                .level(FindingLevel::Violation)
                .signal(parent_signal)
                .build_and_emit(
                    &SampleRef::Attribute(self),
                    live_checker.otlp_emitter.as_ref().map(|rc| rc.as_ref()),
                    parent_signal,
                );

            result.add_advice(finding);
        } else {
            // Provide an info advice if the attribute is a template
            if let Some(attribute) = &semconv_attribute {
                if let AttributeType::Template(_) = attribute.r#type() {
                    let finding = FindingBuilder::new(FindingId::TemplateAttribute)
                        .context(json!({ ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: self.name.clone(), "template_name": attribute.name() }))
                        .message(format!("Attribute '{}' is a template", self.name))
                        .level(FindingLevel::Information)
                        .signal(parent_signal)
                        .build_and_emit(
                            &SampleRef::Attribute(self),
                            live_checker.otlp_emitter.as_ref().map(|rc| rc.as_ref()),
                            parent_signal,
                        );

                    result.add_advice(finding);
                }
            }
        }

        // run advisors on the attribute
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(
                SampleRef::Attribute(self),
                parent_signal,
                semconv_attribute.clone(),
                parent_group.clone(),
                live_checker.otlp_emitter.clone(),
            )?;
            result.add_advice_list(advice_list);
        }
        self.live_check_result = Some(result);
        self.update_stats(stats);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_infer_types() {
        assert_eq!(SampleAttribute::infer_type(&json!(null)), None);
        assert_eq!(
            SampleAttribute::infer_type(&json!(true)),
            Some(PrimitiveOrArrayTypeSpec::Boolean)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!(42)),
            Some(PrimitiveOrArrayTypeSpec::Int)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!(3.15)),
            Some(PrimitiveOrArrayTypeSpec::Double)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!("hello")),
            Some(PrimitiveOrArrayTypeSpec::String)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!([true, false, null])),
            Some(PrimitiveOrArrayTypeSpec::Booleans)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!([1, 2, null, 3])),
            Some(PrimitiveOrArrayTypeSpec::Ints)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!([1.1, 2.2, null, 3.0])),
            Some(PrimitiveOrArrayTypeSpec::Doubles)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!(["a", "b", null, "c"])),
            Some(PrimitiveOrArrayTypeSpec::Strings)
        );
        assert_eq!(
            SampleAttribute::infer_type(&json!([1, "string", true])),
            None
        );
        assert_eq!(SampleAttribute::infer_type(&json!([])), None);
        assert_eq!(SampleAttribute::infer_type(&json!([null, null])), None);
        assert_eq!(SampleAttribute::infer_type(&json!({"key": "value"})), None);
    }

    #[test]
    fn test_deserialize_from_json() {
        // Test with type explicitly provided
        let json_with_type = r#"{"name": "test", "value": 42, "type": "string"}"#;
        let attr: SampleAttribute = serde_json::from_str(json_with_type).unwrap();
        assert_eq!(attr.name, "test");
        assert_eq!(attr.value, Some(json!(42)));
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::String));

        // Test with type inferred
        let json_without_type = r#"{"name": "test", "value": 42}"#;
        let attr: SampleAttribute = serde_json::from_str(json_without_type).unwrap();
        assert_eq!(attr.name, "test");
        assert_eq!(attr.value, Some(json!(42)));
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Int));

        // Test with no value
        let json_null_value = r#"{"name": "test"}"#;
        let attr: SampleAttribute = serde_json::from_str(json_null_value).unwrap();
        assert_eq!(attr.name, "test");
        assert_eq!(attr.value, None);
        assert_eq!(attr.r#type, None);

        // Test with string value
        let json_string_value = r#"{"name": "test", "value": "hello"}"#;
        let attr: SampleAttribute = serde_json::from_str(json_string_value).unwrap();
        assert_eq!(attr.name, "test");
        assert_eq!(attr.value, Some(json!("hello")));
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::String));

        // Test with array value
        let json_array_value = r#"{"name": "test", "value": [1, 2, 3]}"#;
        let attr: SampleAttribute = serde_json::from_str(json_array_value).unwrap();
        assert_eq!(attr.name, "test");
        assert_eq!(attr.value, Some(json!([1, 2, 3])));
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Ints));
    }
}
