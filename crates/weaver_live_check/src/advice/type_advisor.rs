// SPDX-License-Identifier: Apache-2.0

//! Type validation and required attribute checking advisor

use serde_json::json;
use std::{collections::HashSet, rc::Rc};
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_forge::v2::{event::EventAttribute, metric::MetricAttribute};
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::attribute::{
    AttributeType, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec, RequirementLevel,
    TemplateTypeSpec,
};

use super::{emit_findings, Advisor, FindingBuilder};
use crate::{
    otlp_logger::OtlpEmitter, sample_attribute::SampleAttribute, sample_metric::SampleInstrument,
    Error, Sample, SampleRef, VersionedAttribute, VersionedSignal,
    ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY, ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY,
    EXPECTED_VALUE_ADVICE_CONTEXT_KEY, INSTRUMENT_ADVICE_CONTEXT_KEY, TYPE_MISMATCH_ADVICE_TYPE,
    UNEXPECTED_INSTRUMENT_ADVICE_TYPE, UNIT_ADVICE_CONTEXT_KEY, UNIT_MISMATCH_ADVICE_TYPE,
};

/// An advisor that checks if a sample has the correct type
pub struct TypeAdvisor;

/// Trait to abstract over different attribute types for checking
trait CheckableAttribute {
    fn key(&self) -> &str;
    fn requirement_level(&self) -> &RequirementLevel;
    fn attribute_type(&self) -> &AttributeType;
}

impl CheckableAttribute for Attribute {
    fn key(&self) -> &str {
        &self.name
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.r#type
    }
}

impl CheckableAttribute for MetricAttribute {
    fn key(&self) -> &str {
        &self.base.key
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.base.r#type
    }
}

impl CheckableAttribute for EventAttribute {
    fn key(&self) -> &str {
        &self.base.key
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.base.r#type
    }
}

/// Checks if attributes from a resolved group are present in a list of sample attributes
///
/// Returns a list of advice for the attributes based on their RequirementLevel.
///
/// If an attribute is not present in the sample:
///
/// | RequirementLevel       | Live-check advice level |
/// |------------------------|-------------------------|
/// | Required               | Violation               |
/// | Recommended            | Improvement             |
/// | Opt-In                 | Information             |
/// | Conditionally Required | Information             |
fn check_attributes<T: CheckableAttribute>(
    semconv_attributes: &[T],
    sample_attributes: &[SampleAttribute],
    sample: &Sample,
) -> Vec<PolicyFinding> {
    // Create a HashSet of attribute names for O(1) lookups
    let attribute_set: HashSet<_> = sample_attributes
        .iter()
        .map(|attr| attr.name.as_str())
        .collect();

    let mut advice_list = Vec::new();
    for semconv_attribute in semconv_attributes {
        let key = semconv_attribute.key();
        // Check if this is a template attribute
        let is_template = matches!(
            semconv_attribute.attribute_type(),
            AttributeType::Template(_)
        );

        // For template attributes, check if any sample attribute starts with the template prefix
        // For non-template attributes, check for exact match
        let is_present = if is_template {
            sample_attributes
                .iter()
                .any(|attr| attr.name.starts_with(key))
        } else {
            attribute_set.contains(key)
        };

        if !is_present {
            let (advice_type, advice_level, message) = match semconv_attribute.requirement_level() {
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => (
                    "required_attribute_not_present".to_owned(),
                    FindingLevel::Violation,
                    format!("Required attribute '{}' is not present.", key),
                ),
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
                | RequirementLevel::Recommended { .. } => (
                    "recommended_attribute_not_present".to_owned(),
                    FindingLevel::Improvement,
                    format!("Recommended attribute '{}' is not present.", key),
                ),
                RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
                | RequirementLevel::OptIn { .. } => (
                    "opt_in_attribute_not_present".to_owned(),
                    FindingLevel::Information,
                    format!("Opt-in attribute '{}' is not present.", key),
                ),
                RequirementLevel::ConditionallyRequired { .. } => (
                    "conditionally_required_attribute_not_present".to_owned(),
                    FindingLevel::Information,
                    format!("Conditionally required attribute '{}' is not present.", key),
                ),
            };
            advice_list.push(PolicyFinding {
                id: advice_type,
                context: json!({
                    ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: key.to_owned()
                }),
                message,
                level: advice_level,
                signal_type: sample.signal_type(),
                signal_name: sample.signal_name(),
            });
        }
    }
    advice_list
}

impl Advisor for TypeAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        parent_signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                // Only provide advice if the attribute is a match and the type is present
                match (registry_attribute, sample_attribute.r#type.as_ref()) {
                    (Some(semconv_attribute), Some(attribute_type)) => {
                        let semconv_attribute_type = match &semconv_attribute.r#type() {
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
                                    let name = &sample_attribute.name;
                                    let finding = FindingBuilder::new(TYPE_MISMATCH_ADVICE_TYPE)
                                        .context(json!({
                                            ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: name,
                                            ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY: attribute_type,
                                        }))
                                        .message(format!(
                                            "Enum attribute '{}' has type '{}'. Enum value type should be 'string' or 'int'.",
                                            name, attribute_type
                                        ))
                                        .level(FindingLevel::Violation)
                                        .signal(parent_signal)
                                        .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                                    return Ok(vec![finding]);
                                } else {
                                    return Ok(Vec::new());
                                }
                            }
                        };

                        if !attribute_type.is_compatible(semconv_attribute_type) {
                            let name = &sample_attribute.name;
                            let finding = FindingBuilder::new(TYPE_MISMATCH_ADVICE_TYPE)
                                .context(json!({
                                    ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: name,
                                    ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY: attribute_type,
                                    EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_attribute_type,
                                }))
                                .message(format!(
                                    "Attribute '{}' has type '{}'. Type should be '{}'.",
                                    name, attribute_type, semconv_attribute_type
                                ))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            Ok(vec![finding])
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    _ => Ok(Vec::new()),
                }
            }
            SampleRef::Metric(sample_metric) => {
                // Check the instrument and unit of the metric
                let mut advice_list = Vec::new();

                if let Some(semconv_metric) = registry_group {
                    match &sample_metric.instrument {
                        SampleInstrument::Unsupported(name) => {
                            let finding = FindingBuilder::new(UNEXPECTED_INSTRUMENT_ADVICE_TYPE)
                                .context(json!({
                                    INSTRUMENT_ADVICE_CONTEXT_KEY: name,
                                }))
                                .message(format!("Instrument '{name}' is not supported"))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            advice_list.push(finding);
                        }
                        SampleInstrument::Supported(sample_instrument) => {
                            if let Some(semconv_instrument) = semconv_metric.instrument() {
                                if semconv_instrument != sample_instrument {
                                    let finding = FindingBuilder::new(UNEXPECTED_INSTRUMENT_ADVICE_TYPE)
                                        .context(json!({
                                            INSTRUMENT_ADVICE_CONTEXT_KEY: sample_instrument,
                                            EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_instrument,
                                        }))
                                        .message(format!(
                                            "Instrument should be '{semconv_instrument}', but found '{sample_instrument}'."
                                        ))
                                        .level(FindingLevel::Violation)
                                        .signal(parent_signal)
                                        .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                                    advice_list.push(finding);
                                }
                            }
                        }
                    }

                    if let Some(semconv_unit) = semconv_metric.unit() {
                        if semconv_unit != &sample_metric.unit {
                            let unit = &sample_metric.unit;
                            let finding = FindingBuilder::new(UNIT_MISMATCH_ADVICE_TYPE)
                                .context(json!({
                                    UNIT_ADVICE_CONTEXT_KEY: unit,
                                    EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_unit,
                                }))
                                .message(format!(
                                    "Unit should be '{semconv_unit}', but found '{unit}'."
                                ))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            advice_list.push(finding);
                        }
                    }
                }
                Ok(advice_list)
            }
            SampleRef::NumberDataPoint(sample_number_data_point) => {
                if let Some(semconv_metric) = registry_group {
                    let advice_list = match &*semconv_metric {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_number_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Metric(metric) => check_attributes(
                            &metric.attributes,
                            &sample_number_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Event(_event) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            SampleRef::HistogramDataPoint(sample_histogram_data_point) => {
                if let Some(semconv_metric) = registry_group {
                    let advice_list = match &*semconv_metric {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_histogram_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Metric(metric) => check_attributes(
                            &metric.attributes,
                            &sample_histogram_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Event(_event) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            SampleRef::Log(sample_log) => {
                if let Some(semconv_event) = registry_group {
                    let advice_list = match &*semconv_event {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_log.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Event(event) => check_attributes(
                            &event.attributes,
                            &sample_log.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Metric(_metric) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::sample_attribute::SampleAttribute;
    use crate::sample_metric::{SampleInstrument, SampleMetric};
    use weaver_checker::FindingLevel;
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::{
        AttributeType::PrimitiveOrArray, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec,
        RequirementLevel,
    };

    fn create_test_attribute(name: &str, requirement_level: RequirementLevel) -> Attribute {
        Attribute {
            name: name.to_owned(),
            requirement_level,
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "test attribute".to_owned(),
            examples: None,
            tag: None,
            stability: None,
            deprecated: None,
            sampling_relevant: None,
            note: "".to_owned(),
            prefix: false,
            annotations: None,
            role: None,
            tags: None,
            value: None,
        }
    }

    fn create_sample_attribute(name: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            value: None,
            r#type: None,
            live_check_result: None,
        }
    }

    #[test]
    fn test_check_attributes_all_requirement_levels() {
        let semconv_attributes = vec![
            create_test_attribute(
                "required_attr",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ),
            create_test_attribute(
                "recommended_basic",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ),
            create_test_attribute(
                "recommended_text",
                RequirementLevel::Recommended {
                    text: "This is recommended".to_owned(),
                },
            ),
            create_test_attribute(
                "opt_in_basic",
                RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn),
            ),
            create_test_attribute(
                "opt_in_text",
                RequirementLevel::OptIn {
                    text: "This is opt-in".to_owned(),
                },
            ),
            create_test_attribute(
                "conditional",
                RequirementLevel::ConditionallyRequired {
                    text: "Required when X".to_owned(),
                },
            ),
        ];

        // Provide no attributes
        let sample_attributes = vec![];

        // Use a dummy Sample for signal_type and signal_name
        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });

        let advice = check_attributes(&semconv_attributes, &sample_attributes, &sample);
        assert_eq!(advice.len(), 6);

        // Verify each advice type and level
        let advice_map: HashMap<_, _> = advice.iter().map(|a| (a.id.clone(), a.level)).collect();

        assert_eq!(
            advice_map.get("recommended_attribute_not_present"),
            Some(&FindingLevel::Improvement)
        );
        assert_eq!(
            advice_map.get("opt_in_attribute_not_present"),
            Some(&FindingLevel::Information)
        );
        assert_eq!(
            advice_map.get("conditionally_required_attribute_not_present"),
            Some(&FindingLevel::Information)
        );
        assert_eq!(
            advice_map.get("required_attribute_not_present"),
            Some(&FindingLevel::Violation)
        );

        // Count advice levels
        let violations = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Violation)
            .count();
        let improvements = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Improvement)
            .count();
        let information = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Information)
            .count();

        assert_eq!(violations, 1);
        assert_eq!(improvements, 2);
        assert_eq!(information, 3);
    }

    #[test]
    fn test_check_attributes_no_missing_attributes() {
        let semconv_attributes = vec![
            create_test_attribute(
                "attr1",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ),
            create_test_attribute(
                "attr2",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ),
        ];
        let sample_attributes = vec![
            create_sample_attribute("attr1"),
            create_sample_attribute("attr2"),
        ];

        // Use a dummy Sample for signal_type and signal_name
        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });
        let advice = check_attributes(&semconv_attributes, &sample_attributes, &sample);
        assert!(advice.is_empty());
    }

    #[test]
    fn test_check_attributes_template_type() {
        use weaver_semconv::attribute::{AttributeType, TemplateTypeSpec};

        // Create a template attribute like "weaver.finding.context"
        let template_attribute = Attribute {
            name: "weaver.finding.context".to_owned(),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            r#type: AttributeType::Template(TemplateTypeSpec::Any),
            brief: "Template attribute for context".to_owned(),
            examples: None,
            tag: None,
            stability: None,
            deprecated: None,
            sampling_relevant: None,
            note: "".to_owned(),
            prefix: false,
            annotations: None,
            role: None,
            tags: None,
            value: None,
        };

        let semconv_attributes = vec![template_attribute];

        // Test 1: Template attribute with matching prefix - should NOT generate advice
        let sample_attributes_with_match = vec![
            create_sample_attribute("weaver.finding.context.foo"),
            create_sample_attribute("weaver.finding.context.bar"),
        ];

        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });

        let advice = check_attributes(&semconv_attributes, &sample_attributes_with_match, &sample);
        assert!(
            advice.is_empty(),
            "Expected no advice when template attribute has matching prefixed attributes"
        );

        // Test 2: Template attribute without matching prefix - SHOULD generate advice
        let sample_attributes_without_match = vec![
            create_sample_attribute("other.attribute"),
            create_sample_attribute("another.attribute"),
        ];

        let advice = check_attributes(
            &semconv_attributes,
            &sample_attributes_without_match,
            &sample,
        );
        assert_eq!(
            advice.len(),
            1,
            "Expected advice when template attribute has no matching prefixed attributes"
        );
        assert_eq!(advice[0].id, "recommended_attribute_not_present");
        assert_eq!(advice[0].level, FindingLevel::Improvement);

        // Test 3: Mix of template and non-template attributes
        let regular_attribute = create_test_attribute(
            "regular.attr",
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
        );
        let mixed_semconv_attributes = vec![
            Attribute {
                name: "template.attr".to_owned(),
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                r#type: AttributeType::Template(TemplateTypeSpec::String),
                brief: "Template attribute".to_owned(),
                examples: None,
                tag: None,
                stability: None,
                deprecated: None,
                sampling_relevant: None,
                note: "".to_owned(),
                prefix: false,
                annotations: None,
                role: None,
                tags: None,
                value: None,
            },
            regular_attribute,
        ];

        let mixed_sample_attributes = vec![
            create_sample_attribute("template.attr.key1"), // Matches template
            create_sample_attribute("regular.attr"),       // Matches regular
        ];

        let advice = check_attributes(&mixed_semconv_attributes, &mixed_sample_attributes, &sample);
        assert!(
            advice.is_empty(),
            "Expected no advice when both template and regular attributes are present"
        );
    }
}
