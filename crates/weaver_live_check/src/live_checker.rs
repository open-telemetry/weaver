// SPDX-License-Identifier: Apache-2.0

//! Runs advisors on attributes to check for compliance with the registry

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_semconv::attribute::AttributeType;

use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;

use crate::{
    advice::Advisor,
    sample_attribute::SampleAttribute,
    sample_span::{SampleSpanEvent, SampleSpanLink},
    LiveCheckResult, Sample, MISSING_ATTRIBUTE_ADVICE_TYPE, TEMPLATE_ATTRIBUTE_ADVICE_TYPE,
};

/// Provides advice for telemetry samples
#[derive(Serialize)]
pub struct LiveChecker {
    /// The resolved registry
    pub registry: ResolvedRegistry,
    semconv_attributes: HashMap<String, Attribute>,
    semconv_templates: HashMap<String, Attribute>,
    #[serde(skip)]
    advisors: Vec<Advisor>,
    #[serde(skip)]
    templates_by_length: Vec<(String, Attribute)>,
}

impl LiveChecker {
    #[must_use]
    /// Create a new LiveChecker
    pub fn new(registry: ResolvedRegistry, advisors: Vec<Advisor>) -> Self {
        // Create a hashmap of attributes for quick lookup
        let mut semconv_attributes = HashMap::new();
        let mut semconv_templates = HashMap::new();
        let mut templates_by_length = Vec::new();

        for group in &registry.groups {
            for attribute in &group.attributes {
                match attribute.r#type {
                    AttributeType::Template(_) => {
                        templates_by_length.push((attribute.name.clone(), attribute.clone()));
                        let _ = semconv_templates.insert(attribute.name.clone(), attribute.clone());
                    }
                    _ => {
                        let _ =
                            semconv_attributes.insert(attribute.name.clone(), attribute.clone());
                    }
                }
            }
        }

        // Sort templates by name length in descending order
        templates_by_length.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));

        LiveChecker {
            registry,
            semconv_attributes,
            semconv_templates,
            advisors,
            templates_by_length,
        }
    }

    /// Add an advisor
    pub fn add_advisor(&mut self, advisor: Advisor) {
        self.advisors.push(advisor);
    }

    /// Find an attribute in the registry
    #[must_use]
    pub fn find_attribute(&self, name: &str) -> Option<&Attribute> {
        self.semconv_attributes.get(name)
    }

    /// Find a template in the registry
    #[must_use]
    pub fn find_template(&self, attribute_name: &str) -> Option<&Attribute> {
        // Use the pre-sorted list to find the first (longest) matching template
        for (template_name, attribute) in &self.templates_by_length {
            if attribute_name.starts_with(template_name) {
                return Some(attribute);
            }
        }
        None
    }

    fn attach_live_check_result_to_attribute(&mut self, sample_attribute: &mut SampleAttribute) {
        let mut result = LiveCheckResult::new();
        // find the attribute in the registry
        let semconv_attribute = {
            if let Some(attribute) = self.find_attribute(&sample_attribute.name) {
                Some(attribute.clone())
            } else {
                self.find_template(&sample_attribute.name).cloned()
            }
        };

        if semconv_attribute.is_none() {
            result.add_advice(Advice {
                advice_type: MISSING_ATTRIBUTE_ADVICE_TYPE.to_owned(),
                value: Value::String(sample_attribute.name.clone()),
                message: "Does not exist in the registry".to_owned(),
                advice_level: AdviceLevel::Violation,
            });
        } else {
            // Provide an info advice if the attribute is a template
            if let Some(attribute) = &semconv_attribute {
                if let AttributeType::Template(_) = attribute.r#type {
                    result.add_advice(Advice {
                        advice_type: TEMPLATE_ATTRIBUTE_ADVICE_TYPE.to_owned(),
                        value: Value::String(attribute.name.clone()),
                        message: "Is a template".to_owned(),
                        advice_level: AdviceLevel::Information,
                    });
                }
            }
        }

        // run advisors on the attribute
        for entity_advisor in self.advisors.iter_mut() {
            if let Advisor::Attribute(advisor) = entity_advisor {
                if let Ok(advices) = advisor.advise(sample_attribute, semconv_attribute.as_ref()) {
                    for advice in advices {
                        result.add_advice(advice);
                    }
                }
            }
        }
        sample_attribute.live_check_result = Some(result);
    }

    fn attach_live_check_result_to_span_event(&mut self, sample_span_event: &mut SampleSpanEvent) {
        let mut result = LiveCheckResult::new();
        for entity_advisor in self.advisors.iter_mut() {
            if let Advisor::SpanEvent(advisor) = entity_advisor {
                if let Ok(advices) = advisor.advise(sample_span_event, None) {
                    for advice in advices {
                        result.add_advice(advice);
                    }
                }
            }
        }
        for attribute in &mut sample_span_event.attributes {
            self.attach_live_check_result_to_attribute(attribute);
        }
        sample_span_event.live_check_result = Some(result);
    }

    fn attach_live_check_result_to_span_link(&mut self, sample_span_link: &mut SampleSpanLink) {
        let mut result = LiveCheckResult::new();
        for entity_advisor in self.advisors.iter_mut() {
            if let Advisor::SpanLink(advisor) = entity_advisor {
                if let Ok(advices) = advisor.advise(sample_span_link, None) {
                    for advice in advices {
                        result.add_advice(advice);
                    }
                }
            }
        }
        for attribute in &mut sample_span_link.attributes {
            self.attach_live_check_result_to_attribute(attribute);
        }
        sample_span_link.live_check_result = Some(result);
    }

    /// Attach live check result to the sample
    pub fn attach_live_check_result(&mut self, sample: &mut Sample) {
        let mut result = LiveCheckResult::new();

        match sample {
            Sample::Attribute(sample_attribute) => {
                self.attach_live_check_result_to_attribute(sample_attribute);
            }
            Sample::Span(sample_span) => {
                // TODO Remove this:
                let span_advice = Advice {
                    advice_type: "span_info".to_owned(),
                    value: Value::String(sample_span.name.clone()),
                    message: format!("Has span kind: `{}`", sample_span.kind),
                    advice_level: AdviceLevel::Information,
                };
                result.add_advice(span_advice);

                for entity_advisor in self.advisors.iter_mut() {
                    if let Advisor::Span(advisor) = entity_advisor {
                        if let Ok(advices) = advisor.advise(sample_span, None) {
                            for advice in advices {
                                result.add_advice(advice);
                            }
                        }
                    }
                }

                for attribute in &mut sample_span.attributes {
                    self.attach_live_check_result_to_attribute(attribute);
                }
                for span_event in &mut sample_span.span_events {
                    self.attach_live_check_result_to_span_event(span_event);
                }
                for span_link in &mut sample_span.span_links {
                    self.attach_live_check_result_to_span_link(span_link);
                }

                sample_span.live_check_result = Some(result);
            }
            Sample::SpanEvent(sample_span_event) => {
                self.attach_live_check_result_to_span_event(sample_span_event);
            }
            Sample::SpanLink(sample_span_link) => {
                self.attach_live_check_result_to_span_link(sample_span_link);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        advice::{DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor},
        sample_attribute::SampleAttribute,
        LiveCheckStatistics,
    };

    use super::*;
    use serde_json::Value;
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::{
        attribute::{
            AttributeType, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel,
            TemplateTypeSpec, ValueSpec,
        },
        group::{GroupType, SpanKindSpec},
        stability::Stability,
    };

    fn get_all_advice(sample: &mut Sample) -> &mut [Advice] {
        match sample {
            Sample::Attribute(sample_attribute) => sample_attribute
                .live_check_result
                .as_mut() // Change to as_mut() to get a mutable reference
                .map(|result| &mut result.all_advice)
                .map_or(&mut [], |v| v),
            _ => &mut [],
        }
    }

    #[test]
    fn test_attribute_live_checker() {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![ResolvedGroup {
                id: "test.comprehensive.internal".to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                entity_associations: vec![],
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![
                    Attribute {
                        name: "test.string".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "value1".to_owned(),
                            "value2".to_owned(),
                        ])),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                    },
                    Attribute {
                        name: "test.enum".to_owned(),
                        r#type: AttributeType::Enum {
                            allow_custom_values: None,
                            members: vec![
                                EnumEntriesSpec {
                                    id: "test_enum_member".to_owned(),
                                    value: ValueSpec::String("example_variant1".to_owned()),
                                    brief: None,
                                    note: None,
                                    stability: Some(Stability::Stable),
                                    deprecated: None,
                                },
                                EnumEntriesSpec {
                                    id: "test_enum_member2".to_owned(),
                                    value: ValueSpec::String("example_variant2".to_owned()),
                                    brief: None,
                                    note: None,
                                    stability: Some(Stability::Stable),
                                    deprecated: None,
                                },
                            ],
                        },
                        examples: None,
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                    },
                    Attribute {
                        name: "test.deprecated".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "value1".to_owned(),
                            "value2".to_owned(),
                        ])),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Development),
                        deprecated: Some(weaver_semconv::deprecated::Deprecated::Uncategorized {
                            note: "note".to_owned(),
                        }),
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                    },
                    Attribute {
                        name: "test.template".to_owned(),
                        r#type: AttributeType::Template(TemplateTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "value1".to_owned(),
                            "value2".to_owned(),
                        ])),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                    },
                ],
                span_kind: Some(SpanKindSpec::Internal),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                lineage: None,
                display_name: None,
                body: None,
            }],
        };

        let mut samples = vec![
            Sample::Attribute(SampleAttribute::try_from("test.string=value").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("testString2").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.deprecated=42").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("aws.s3.bucket.name").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=foo").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=example_variant1").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=42.42").unwrap()),
            Sample::Attribute(
                SampleAttribute::try_from("test.string.not.allowed=example_value").unwrap(),
            ),
            Sample::Attribute(SampleAttribute::try_from("test.extends=new_value").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.template.my.key=42").unwrap()),
        ];

        let advisors: Vec<Advisor> = vec![
            Advisor::Attribute(Box::new(DeprecatedAdvisor)),
            Advisor::Attribute(Box::new(StabilityAdvisor)),
            Advisor::Attribute(Box::new(TypeAdvisor)),
            Advisor::Attribute(Box::new(EnumAdvisor)),
        ];

        let mut live_checker = LiveChecker::new(registry, advisors);
        let rego_advisor =
            RegoAdvisor::new(&live_checker, &None, &None).expect("Failed to create Rego advisor");
        live_checker.add_advisor(Advisor::Attribute(Box::new(rego_advisor)));

        let mut stats = LiveCheckStatistics::new(&live_checker.registry);
        for sample in &mut samples {
            live_checker.attach_live_check_result(sample);
            stats.update(sample);
        }
        stats.finalize();

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 3);
        // make a sort of the advice
        all_advice.sort_by(|a, b| a.advice_type.cmp(&b.advice_type));
        assert_eq!(all_advice[0].advice_type, "invalid_format");
        assert_eq!(all_advice[0].value, Value::String("testString2".to_owned()));
        assert_eq!(
            all_advice[0].message,
            "Does not match name formatting rules"
        );
        assert_eq!(all_advice[1].advice_type, "missing_attribute");
        assert_eq!(all_advice[1].value, Value::String("testString2".to_owned()));
        assert_eq!(all_advice[1].message, "Does not exist in the registry");
        assert_eq!(all_advice[2].advice_type, "missing_namespace");
        assert_eq!(all_advice[2].value, Value::String("testString2".to_owned()));
        assert_eq!(all_advice[2].message, "Does not have a namespace");

        let all_advice = get_all_advice(&mut samples[2]);
        assert_eq!(all_advice.len(), 3);
        assert_eq!(all_advice[0].advice_type, "deprecated");
        assert_eq!(
            all_advice[0].value,
            Value::String("uncategorized".to_owned())
        );
        assert_eq!(all_advice[0].message, "note");

        assert_eq!(all_advice[1].advice_type, "stability");
        assert_eq!(all_advice[1].value, Value::String("development".to_owned()));
        assert_eq!(all_advice[1].message, "Is not stable");

        assert_eq!(all_advice[2].advice_type, "type_mismatch");
        assert_eq!(all_advice[2].value, Value::String("int".to_owned()));
        assert_eq!(all_advice[2].message, "Type should be `string`");

        let all_advice = get_all_advice(&mut samples[3]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].advice_type, "missing_attribute");
        assert_eq!(
            all_advice[0].value,
            Value::String("aws.s3.bucket.name".to_owned())
        );
        assert_eq!(all_advice[0].message, "Does not exist in the registry");

        let all_advice = get_all_advice(&mut samples[4]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].advice_type, "undefined_enum_variant");
        assert_eq!(all_advice[0].value, Value::String("foo".to_owned()));
        assert_eq!(all_advice[0].message, "Is not a defined variant");

        let all_advice = get_all_advice(&mut samples[6]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].advice_type, "type_mismatch");
        assert_eq!(all_advice[0].value, Value::String("double".to_owned()));
        assert_eq!(all_advice[0].message, "Type should be `string` or `int`");

        let all_advice = get_all_advice(&mut samples[7]);

        // Make a sort of the advice
        all_advice.sort_by(|a, b| a.advice_type.cmp(&b.advice_type));
        assert_eq!(all_advice.len(), 3);

        assert_eq!(all_advice[0].advice_type, "extends_namespace");
        assert_eq!(all_advice[0].value, Value::String("test".to_owned()));
        assert_eq!(all_advice[0].message, "Extends existing namespace");
        assert_eq!(all_advice[1].advice_type, "illegal_namespace");
        assert_eq!(all_advice[1].value, Value::String("test.string".to_owned()));
        assert_eq!(
            all_advice[1].message,
            "Namespace matches existing attribute"
        );
        assert_eq!(all_advice[2].advice_type, "missing_attribute");
        assert_eq!(
            all_advice[2].value,
            Value::String("test.string.not.allowed".to_owned())
        );
        assert_eq!(all_advice[2].message, "Does not exist in the registry");

        let all_advice = get_all_advice(&mut samples[8]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].advice_type, "missing_attribute");
        assert_eq!(
            all_advice[0].value,
            Value::String("test.extends".to_owned())
        );
        assert_eq!(all_advice[0].message, "Does not exist in the registry");
        assert_eq!(all_advice[1].advice_type, "extends_namespace");
        assert_eq!(all_advice[1].value, Value::String("test".to_owned()));
        assert_eq!(all_advice[1].message, "Extends existing namespace");

        // test.template
        let all_advice = get_all_advice(&mut samples[9]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].advice_type, "template_attribute");
        assert_eq!(
            all_advice[0].value,
            Value::String("test.template".to_owned())
        );
        assert_eq!(all_advice[0].message, "Is a template");
        assert_eq!(all_advice[1].advice_type, "type_mismatch");
        assert_eq!(all_advice[1].value, Value::String("int".to_owned()));
        assert_eq!(all_advice[1].message, "Type should be `string`");

        // Check statistics
        assert_eq!(stats.total_attributes, 10);
        assert_eq!(stats.total_advisories, 16);
        assert_eq!(stats.advice_level_counts.len(), 3);
        assert_eq!(stats.advice_level_counts[&AdviceLevel::Violation], 10);
        assert_eq!(stats.advice_level_counts[&AdviceLevel::Information], 4);
        assert_eq!(stats.advice_level_counts[&AdviceLevel::Improvement], 2);
        assert_eq!(stats.highest_advice_level_counts.len(), 2);
        assert_eq!(
            stats.highest_advice_level_counts[&AdviceLevel::Violation],
            7
        );
        assert_eq!(
            stats.highest_advice_level_counts[&AdviceLevel::Information],
            1
        );
        assert_eq!(stats.no_advice_count, 2);
        assert_eq!(stats.seen_registry_attributes.len(), 3);
        assert_eq!(stats.seen_registry_attributes["test.enum"], 3);
        assert_eq!(stats.seen_non_registry_attributes.len(), 5);
        assert_eq!(stats.registry_coverage, 1.0);
    }

    #[test]
    fn test_custom_rego() {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![ResolvedGroup {
                id: "custom.comprehensive.internal".to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                entity_associations: vec![],
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![Attribute {
                    name: "custom.string".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: Some(Examples::Strings(vec![
                        "value1".to_owned(),
                        "value2".to_owned(),
                    ])),
                    brief: "".to_owned(),
                    tag: None,
                    requirement_level: RequirementLevel::Recommended {
                        text: "".to_owned(),
                    },
                    sampling_relevant: None,
                    note: "".to_owned(),
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    prefix: false,
                    tags: None,
                    value: None,
                    annotations: None,
                }],
                span_kind: Some(SpanKindSpec::Internal),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                lineage: None,
                display_name: None,
                body: None,
            }],
        };

        let mut samples = vec![
            Sample::Attribute(SampleAttribute::try_from("custom.string=hello").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.string").unwrap()),
        ];

        let advisors: Vec<Advisor> = vec![];

        let mut live_checker = LiveChecker::new(registry, advisors);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Advisor::Attribute(Box::new(rego_advisor)));

        let mut stats = LiveCheckStatistics::new(&live_checker.registry);
        for sample in &mut samples {
            live_checker.attach_live_check_result(sample);
            stats.update(sample);
        }
        stats.finalize();

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 2);

        assert_eq!(all_advice[0].advice_type, "missing_attribute");
        assert_eq!(all_advice[0].value, Value::String("test.string".to_owned()));
        assert_eq!(all_advice[0].message, "Does not exist in the registry");
        assert_eq!(all_advice[1].advice_type, "contains_test");
        assert_eq!(all_advice[1].value, Value::String("test.string".to_owned()));
        assert_eq!(all_advice[1].message, "Name must not contain 'test'");

        // Check statistics
        assert_eq!(stats.total_attributes, 2);
        assert_eq!(stats.total_advisories, 2);
        assert_eq!(stats.advice_level_counts.len(), 1);
        assert_eq!(stats.advice_level_counts[&AdviceLevel::Violation], 2);
        assert_eq!(stats.highest_advice_level_counts.len(), 1);
        assert_eq!(
            stats.highest_advice_level_counts[&AdviceLevel::Violation],
            1
        );
        assert_eq!(stats.no_advice_count, 1);
    }
}
