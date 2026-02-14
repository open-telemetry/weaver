// SPDX-License-Identifier: Apache-2.0

//! Holds the registry, helper structs, and the advisors for the live check

use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use weaver_semconv::{attribute::AttributeType, group::GroupType};

use crate::{
    advice::Advisor, otlp_logger::OtlpEmitter, VersionedAttribute, VersionedRegistry,
    VersionedSignal,
};

#[cfg(test)]
use crate::CumulativeStatistics;

/// Holds the registry, helper structs, and the advisors for the live check
#[derive(Serialize)]
pub struct LiveChecker {
    /// The resolved registry
    pub registry: Arc<VersionedRegistry>,
    semconv_attributes: HashMap<String, Rc<VersionedAttribute>>,
    semconv_templates: HashMap<String, Rc<VersionedAttribute>>,
    semconv_metrics: HashMap<String, Rc<VersionedSignal>>,
    semconv_events: HashMap<String, Rc<VersionedSignal>>,
    /// The advisors to run
    #[serde(skip)]
    pub advisors: Vec<Box<dyn Advisor>>,
    #[serde(skip)]
    templates_by_length: Vec<(String, Rc<VersionedAttribute>)>,
    /// Optional OTLP emitter for emitting findings as log records
    #[serde(skip)]
    pub otlp_emitter: Option<Rc<OtlpEmitter>>,
}

impl LiveChecker {
    #[must_use]
    /// Create a new LiveChecker
    pub fn new(registry: Arc<VersionedRegistry>, advisors: Vec<Box<dyn Advisor>>) -> Self {
        // Create a hashmap of attributes for quick lookup
        let mut semconv_attributes = HashMap::new();
        let mut semconv_templates = HashMap::new();
        let mut templates_by_length = Vec::new();
        // Hashmap of metrics by name
        let mut semconv_metrics = HashMap::new();
        // Hashmap of events by name
        let mut semconv_events = HashMap::new();

        match registry.as_ref() {
            VersionedRegistry::V1(registry) => {
                for group in &registry.groups {
                    if group.r#type == GroupType::Metric {
                        if let Some(metric_name) = &group.metric_name {
                            let group_rc = Rc::new(VersionedSignal::Group(Box::new(group.clone())));
                            let _ = semconv_metrics.insert(metric_name.clone(), group_rc);
                        }
                    }
                    if group.r#type == GroupType::Event {
                        if let Some(event_name) = &group.name {
                            let group_rc = Rc::new(VersionedSignal::Group(Box::new(group.clone())));
                            let _ = semconv_events.insert(event_name.clone(), group_rc);
                        }
                    }
                    for attribute in &group.attributes {
                        let attribute_rc =
                            Rc::new(VersionedAttribute::V1(Box::new(attribute.clone())));
                        match attribute.r#type {
                            AttributeType::Template(_) => {
                                templates_by_length
                                    .push((attribute.name.clone(), attribute_rc.clone()));
                                let _ =
                                    semconv_templates.insert(attribute.name.clone(), attribute_rc);
                            }
                            _ => {
                                let _ =
                                    semconv_attributes.insert(attribute.name.clone(), attribute_rc);
                            }
                        }
                    }
                }
            }
            VersionedRegistry::V2(registry) => {
                for metric in &registry.registry.metrics {
                    let metric_name = metric.name.to_string();
                    let metric_rc = Rc::new(VersionedSignal::Metric(metric.clone()));
                    let _ = semconv_metrics.insert(metric_name, metric_rc);
                }
                for event in &registry.registry.events {
                    let event_name = event.name.to_string();
                    let event_rc = Rc::new(VersionedSignal::Event(event.clone()));
                    let _ = semconv_events.insert(event_name, event_rc);
                }
                for attribute in &registry.registry.attributes {
                    let attribute_rc = Rc::new(VersionedAttribute::V2(Box::new(attribute.clone())));
                    match &attribute.r#type {
                        AttributeType::Template(_) => {
                            templates_by_length.push((attribute.key.clone(), attribute_rc.clone()));
                            let _ = semconv_templates.insert(attribute.key.clone(), attribute_rc);
                        }
                        _ => {
                            let _ = semconv_attributes.insert(attribute.key.clone(), attribute_rc);
                        }
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
            semconv_metrics,
            semconv_events,
            advisors,
            templates_by_length,
            otlp_emitter: None,
        }
    }

    /// Add an advisor
    pub fn add_advisor(&mut self, advisor: Box<dyn Advisor>) {
        self.advisors.push(advisor);
    }

    /// Find an attribute in the registry
    #[must_use]
    pub fn find_attribute(&self, name: &str) -> Option<Rc<VersionedAttribute>> {
        self.semconv_attributes.get(name).map(Rc::clone)
    }

    /// Find a metric in the registry
    #[must_use]
    pub fn find_metric(&self, name: &str) -> Option<Rc<VersionedSignal>> {
        self.semconv_metrics.get(name).map(Rc::clone)
    }

    /// Find an event in the registry
    #[must_use]
    pub fn find_event(&self, name: &str) -> Option<Rc<VersionedSignal>> {
        self.semconv_events.get(name).map(Rc::clone)
    }

    /// Find a template in the registry
    #[must_use]
    pub fn find_template(&self, attribute_name: &str) -> Option<Rc<VersionedAttribute>> {
        // Use the pre-sorted list to find the first (longest) matching template
        for (template_name, attribute) in &self.templates_by_length {
            if attribute_name.starts_with(template_name) {
                return Some(Rc::clone(attribute));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::{
        advice::{DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor},
        sample_attribute::SampleAttribute,
        sample_metric::{
            DataPoints, SampleExemplar, SampleExponentialHistogramDataPoint, SampleInstrument,
            SampleMetric, SampleNumberDataPoint,
        },
        LiveCheckRunner, LiveCheckStatistics, Sample,
    };

    use super::*;
    use serde_json::json;
    use serde_yaml;
    use std::collections::BTreeMap;
    use weaver_checker::{FindingLevel, PolicyFinding};
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_forge::v2::{
        attribute::Attribute as V2Attribute,
        event::{Event as V2Event, EventAttribute},
        metric::{Metric as V2Metric, MetricAttribute},
        registry::{ForgeResolvedRegistry, Refinements, Registry},
        span::{Span as V2Span, SpanAttribute},
    };
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::{
        attribute::{
            AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, Examples,
            PrimitiveOrArrayTypeSpec, RequirementLevel, TemplateTypeSpec, ValueSpec,
        },
        group::{GroupType, InstrumentSpec, SpanKindSpec},
        stability::Stability,
        YamlValue,
    };
    use weaver_semconv::{
        schema_url::SchemaUrl,
        v2::{span::SpanName, CommonFields},
    };

    fn get_all_advice(sample: &mut Sample) -> &mut [PolicyFinding] {
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
        run_attribute_live_checker_test(false);
    }

    #[test]
    fn test_attribute_live_checker_v2() {
        run_attribute_live_checker_test(true);
    }

    fn run_attribute_live_checker_test(use_v2: bool) {
        let registry = make_registry(use_v2);

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
            Sample::Attribute(SampleAttribute::try_from("test.deprecated.allowed=42").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=17").unwrap()),
        ];

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor =
            RegoAdvisor::new(&live_checker, &None, &None).expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 3);
        // make a sort of the advice
        all_advice.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all_advice[0].id, "invalid_format");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "testString2" })
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'testString2' does not match name formatting rules."
        );
        assert_eq!(all_advice[1].id, "missing_attribute");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "testString2"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'testString2' does not exist in the registry."
        );
        assert_eq!(all_advice[2].id, "missing_namespace");
        assert_eq!(
            all_advice[2].context,
            json!({"attribute_name": "testString2"})
        );
        assert_eq!(all_advice[2].message, "Attribute name 'testString2' must include a namespace (e.g. '{namespace}.{attribute_key}')");

        let all_advice = get_all_advice(&mut samples[2]);
        assert_eq!(all_advice.len(), 3);
        assert_eq!(all_advice[0].id, "deprecated");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.deprecated", "deprecation_reason": "uncategorized", "deprecation_note": "note"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.deprecated' is deprecated; reason = 'uncategorized', note = 'note'."
        );

        assert_eq!(all_advice[1].id, "not_stable");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.deprecated", "stability": "development"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'test.deprecated' is not stable; stability = development."
        );

        assert_eq!(all_advice[2].id, "type_mismatch");
        assert_eq!(
            all_advice[2].context,
            json!({"attribute_name": "test.deprecated", "attribute_type": "int", "expected": "string"})
        );
        assert_eq!(
            all_advice[2].message,
            "Attribute 'test.deprecated' has type 'int'. Type should be 'string'."
        );

        let all_advice = get_all_advice(&mut samples[3]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "aws.s3.bucket.name"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'aws.s3.bucket.name' does not exist in the registry."
        );

        let all_advice = get_all_advice(&mut samples[4]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "undefined_enum_variant");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.enum", "attribute_value": "foo"})
        );
        assert_eq!(
            all_advice[0].message,
            "Enum attribute 'test.enum' has value 'foo' which is not documented."
        );

        let all_advice = get_all_advice(&mut samples[6]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "type_mismatch");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.enum", "attribute_type": "double"})
        );
        assert_eq!(all_advice[0].message, "Enum attribute 'test.enum' has type 'double'. Enum value type should be 'string' or 'int'.");

        let all_advice = get_all_advice(&mut samples[7]);

        // Make a sort of the advice
        all_advice.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all_advice.len(), 3);

        assert_eq!(all_advice[0].id, "extends_namespace");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.string.not.allowed", "namespace": "test"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute name 'test.string.not.allowed' collides with existing namespace 'test'"
        );
        assert_eq!(all_advice[1].id, "illegal_namespace");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.string.not.allowed", "namespace": "test.string"})
        );
        assert_eq!(
            all_advice[1].message,
            "Namespace 'test.string' collides with existing attribute 'test.string.not.allowed'"
        );
        assert_eq!(all_advice[2].id, "missing_attribute");
        assert_eq!(
            all_advice[2].context,
            json!({
                "attribute_name": "test.string.not.allowed"
            })
        );
        assert_eq!(
            all_advice[2].message,
            "Attribute 'test.string.not.allowed' does not exist in the registry."
        );

        let all_advice = get_all_advice(&mut samples[8]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.extends"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.extends' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "extends_namespace");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.extends", "namespace": "test"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute name 'test.extends' collides with existing namespace 'test'"
        );

        // test.template
        let all_advice = get_all_advice(&mut samples[9]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "template_attribute");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.template.my.key", "template_name": "test.template"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.template.my.key' is a template"
        );
        assert_eq!(all_advice[1].id, "type_mismatch");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.template.my.key", "attribute_type": "int", "expected": "string"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'test.template.my.key' has type 'int'. Type should be 'string'."
        );

        // test.deprecated.allowed
        // Should not get illegal_namespace for extending a deprecated attribute
        let all_advice = get_all_advice(&mut samples[10]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.deprecated.allowed"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.deprecated.allowed' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "extends_namespace");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.deprecated.allowed", "namespace": "test"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute name 'test.deprecated.allowed' collides with existing namespace 'test'"
        );

        let all_advice = get_all_advice(&mut samples[11]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "undefined_enum_variant");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.enum", "attribute_value": 17})
        );
        assert_eq!(
            all_advice[0].message,
            "Enum attribute 'test.enum' has value '17' which is not documented."
        );

        // Check statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 12);
            assert_eq!(cumulative_stats.total_advisories, 19);
            assert_eq!(cumulative_stats.advice_level_counts.len(), 3);
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Violation],
                11
            );
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Information],
                6
            );
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Improvement],
                2
            );
            assert_eq!(cumulative_stats.highest_advice_level_counts.len(), 2);
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Violation],
                8
            );
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Information],
                2
            );
            assert_eq!(cumulative_stats.no_advice_count, 2);
            assert_eq!(cumulative_stats.seen_registry_attributes.len(), 3);
            assert_eq!(cumulative_stats.seen_registry_attributes["test.enum"], 4);
            assert_eq!(cumulative_stats.seen_non_registry_attributes.len(), 6);
            assert_eq!(cumulative_stats.registry_coverage, 1.0);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    fn make_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: SchemaUrl::new("https://example.com/schemas/1.2.3".to_owned()),
                registry: Registry {
                    attributes: vec![
                        V2Attribute {
                            key: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                        V2Attribute {
                            key: "test.enum".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "test_enum_member".to_owned(),
                                        value: ValueSpec::String("example_variant1".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "test_enum_member2".to_owned(),
                                        value: ValueSpec::String("example_variant2".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                ],
                            },
                            examples: None,
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                        V2Attribute {
                            key: "test.deprecated".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: Some(
                                    weaver_semconv::deprecated::Deprecated::Uncategorized {
                                        note: "note".to_owned(),
                                    },
                                ),
                                annotations: BTreeMap::new(),
                            },
                        },
                        V2Attribute {
                            key: "test.template".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::String),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                    ],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                },
            }))
        } else {
            VersionedRegistry::V1(ResolvedRegistry {
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
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
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
                            role: Default::default(),
                        },
                        Attribute {
                            name: "test.enum".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "test_enum_member".to_owned(),
                                        value: ValueSpec::String("example_variant1".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "test_enum_member2".to_owned(),
                                        value: ValueSpec::String("example_variant2".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
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
                            role: Default::default(),
                        },
                        Attribute {
                            name: "test.deprecated".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
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
                            deprecated: Some(
                                weaver_semconv::deprecated::Deprecated::Uncategorized {
                                    note: "note".to_owned(),
                                },
                            ),
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
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
                            role: Default::default(),
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
                    annotations: None,
                }],
            })
        }
    }

    fn make_metrics_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let memory_state_attr = V2Attribute {
                key: "system.memory.state".to_owned(),
                r#type: AttributeType::Enum {
                    members: vec![
                        EnumEntriesSpec {
                            id: "used".to_owned(),
                            value: ValueSpec::String("used".to_owned()),
                            brief: None,
                            note: None,
                            stability: Some(Stability::Development),
                            deprecated: None,
                            annotations: None,
                        },
                        EnumEntriesSpec {
                            id: "free".to_owned(),
                            value: ValueSpec::String("free".to_owned()),
                            brief: None,
                            note: None,
                            stability: Some(Stability::Development),
                            deprecated: None,
                            annotations: None,
                        },
                    ],
                },
                examples: Some(Examples::Strings(vec![
                    "free".to_owned(),
                    "cached".to_owned(),
                ])),
                common: CommonFields {
                    brief: "The memory state".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: SchemaUrl::new("https://example.com/schemas/1.2.3".to_owned()),
                registry: Registry {
                    attributes: vec![memory_state_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![
                        V2Metric {
                            name: "system.uptime".to_owned().into(),
                            instrument: InstrumentSpec::Gauge,
                            unit: "s".to_owned(),
                            attributes: vec![],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "The time the system has been running".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                        V2Metric {
                            name: "system.memory.usage".to_owned().into(),
                            instrument: InstrumentSpec::UpDownCounter,
                            unit: "By".to_owned(),
                            attributes: vec![MetricAttribute {
                                base: memory_state_attr.clone(),
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                            }],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "Reports memory in use by state.".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                    ],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                },
            }))
        } else {
            VersionedRegistry::V1(ResolvedRegistry {
                registry_url: "TEST_METRICS".to_owned(),
                groups: vec![
                    // Attribute group for system memory
                    ResolvedGroup {
                        id: "registry.system.memory".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        brief: "Describes System Memory attributes".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: None,
                        deprecated: None,
                        attributes: vec![Attribute {
                            name: "system.memory.state".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "used".to_owned(),
                                        value: ValueSpec::String("used".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Development),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "free".to_owned(),
                                        value: ValueSpec::String("free".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Development),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                ],
                            },
                            examples: Some(Examples::Strings(vec![
                                "free".to_owned(),
                                "cached".to_owned(),
                            ])),
                            brief: "The memory state".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Development),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        }],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        name: None,
                        lineage: None,
                        display_name: Some("System Memory Attributes".to_owned()),
                        body: None,
                        annotations: None,
                    },
                    // System uptime metric
                    ResolvedGroup {
                        id: "metric.system.uptime".to_owned(),
                        r#type: GroupType::Metric,
                        brief: "The time the system has been running".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: Some("system.uptime".to_owned()),
                        instrument: Some(InstrumentSpec::Gauge),
                        unit: Some("s".to_owned()),
                        name: None,
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                    // System memory usage metric
                    ResolvedGroup {
                        id: "metric.system.memory.usage".to_owned(),
                        r#type: GroupType::Metric,
                        brief: "Reports memory in use by state.".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: None,
                        attributes: vec![Attribute {
                            name: "system.memory.state".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: None,
                            brief: "The memory state".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Development),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        }],
                        span_kind: None,
                        events: vec![],
                        metric_name: Some("system.memory.usage".to_owned()),
                        instrument: Some(InstrumentSpec::UpDownCounter),
                        unit: Some("By".to_owned()),
                        name: None,
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                ],
            })
        }
    }

    fn make_custom_rego_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let custom_string_attr = V2Attribute {
                key: "custom.string".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "value1".to_owned(),
                    "value2".to_owned(),
                ])),
                common: CommonFields {
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: SchemaUrl::new("https://example.com/schemas/1.2.3".to_owned()),
                registry: Registry {
                    attributes: vec![custom_string_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![V2Span {
                        r#type: "custom.comprehensive.internal".to_owned().into(),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "custom.comprehensive.internal".to_owned(),
                        },
                        attributes: vec![SpanAttribute {
                            base: custom_string_attr.clone(),
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                        }],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                    }],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                },
            }))
        } else {
            VersionedRegistry::V1(ResolvedRegistry {
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
                        role: Default::default(),
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
                    annotations: None,
                }],
            })
        }
    }

    #[test]
    fn test_custom_rego() {
        run_custom_rego_test(false);
    }

    #[test]
    fn test_custom_rego_v2() {
        run_custom_rego_test(true);
    }

    fn run_custom_rego_test(use_v2: bool) {
        let registry = make_custom_rego_registry(use_v2);

        let mut samples = vec![
            Sample::Attribute(SampleAttribute::try_from("custom.string=hello").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.string").unwrap()),
        ];

        let advisors: Vec<Box<dyn Advisor>> = vec![];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 2);

        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            json!({"attribute_name": "test.string"})
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.string' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "contains_test");
        assert_eq!(
            all_advice[1].context,
            json!({"attribute_name": "test.string"})
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute name must not contain 'test', but was 'test.string'"
        );

        // Check statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 2);
            assert_eq!(cumulative_stats.total_advisories, 2);
            assert_eq!(cumulative_stats.advice_level_counts.len(), 1);
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Violation],
                2
            );
            assert_eq!(cumulative_stats.highest_advice_level_counts.len(), 1);
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Violation],
                1
            );
            assert_eq!(cumulative_stats.no_advice_count, 1);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_input_output() {
        run_json_input_output_test(false);
    }

    #[test]
    fn test_json_input_output_v2() {
        run_json_input_output_test(true);
    }

    fn run_json_input_output_test(use_v2: bool) {
        let registry = make_registry(use_v2);

        // Load samples from JSON file
        let path = "data/span.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor =
            RegoAdvisor::new(&live_checker, &None, &None).expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 14);
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&10)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span_event"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span_link"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("resource"),
                Some(&1)
            );
            assert_eq!(cumulative_stats.total_advisories, 14);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_span_rego() {
        run_json_span_rego_test(false);
    }

    #[test]
    fn test_json_span_rego_v2() {
        run_json_span_rego_test(true);
    }

    fn run_json_span_rego_test(use_v2: bool) {
        let registry = make_registry(use_v2);

        // Load samples from JSON file
        let path = "data/span.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("contains_test_in_status"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_metric() {
        run_json_metric_test(false);
    }

    #[test]
    fn test_json_metric_v2() {
        run_json_metric_test(true);
    }

    fn run_json_metric_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // Load samples from JSON file
        let path = "data/metrics.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor =
            RegoAdvisor::new(&live_checker, &None, &None).expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("data_point"),
                Some(&6)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("metric"),
                Some(&4)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&3)
            );
            assert_eq!(cumulative_stats.no_advice_count, 4);
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("recommended_attribute_not_present"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_attribute"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("not_stable"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_metric"),
                Some(&3)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_namespace"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats
                    .seen_registry_metrics
                    .get("system.memory.usage"),
                Some(&1)
            );
            assert_eq!(cumulative_stats.seen_non_registry_metrics.len(), 3);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_metric_custom_rego() {
        run_json_metric_custom_rego_test(false);
    }

    #[test]
    fn test_json_metric_custom_rego_v2() {
        run_json_metric_custom_rego_test(true);
    }

    fn run_json_metric_custom_rego_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // Load samples from JSON file
        let path = "data/metrics.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

            assert!(result.is_ok());
        }
        stats.finalize();
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("invalid_data_point_value"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_log_custom_rego() {
        run_json_log_custom_rego_test(false);
    }

    #[test]
    fn test_json_log_custom_rego_v2() {
        run_json_log_custom_rego_test(true);
    }

    fn run_json_log_custom_rego_test(use_v2: bool) {
        let registry = make_events_registry(use_v2);

        // Load samples from JSON file
        let path = "data/logs.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

            assert!(result.is_ok());
        }
        stats.finalize();

        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.advice_type_counts.get("empty_body"),
                Some(&1),
                "Expected 1 empty_body advice for event with empty name and empty body"
            );

            assert_eq!(
                cumulative_stats.advice_type_counts.get("required_phrase_missing"),
                Some(&1),
                "Expected 1 required_phrase_missing advice for session.start event missing 'hello world'"
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    fn make_events_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let session_id_attr = V2Attribute {
                key: "session.id".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                ])),
                common: CommonFields {
                    brief: "A unique session identifier".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            };

            let session_previous_id_attr = V2Attribute {
                key: "session.previous_id".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                ])),
                common: CommonFields {
                    brief: "The previous session identifier".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: SchemaUrl::new("https://example.com/schemas/1.2.3".to_owned()),
                registry: Registry {
                    attributes: vec![session_id_attr.clone(), session_previous_id_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![],
                    events: vec![
                        V2Event {
                            name: "session.start".to_owned().into(),
                            attributes: vec![
                                EventAttribute {
                                    base: session_id_attr.clone(),
                                    requirement_level: RequirementLevel::Basic(
                                        BasicRequirementLevelSpec::Required,
                                    ),
                                },
                                EventAttribute {
                                    base: session_previous_id_attr.clone(),
                                    requirement_level: RequirementLevel::Recommended {
                                        text: "".to_owned(),
                                    },
                                },
                            ],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "This event represents a session start".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: Some(
                                    weaver_semconv::deprecated::Deprecated::Uncategorized {
                                        note: "Use session.initialized event instead".to_owned(),
                                    },
                                ),
                                annotations: {
                                    let mut annotations = BTreeMap::new();
                                    let _ = annotations.insert(
                                        "required_phrase".to_owned(),
                                        YamlValue(serde_yaml::Value::String(
                                            "hello world".to_owned(),
                                        )),
                                    );
                                    annotations
                                },
                            },
                        },
                        V2Event {
                            name: "example.event".to_owned().into(),
                            attributes: vec![],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "An example event".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: {
                                    let mut annotations = BTreeMap::new();
                                    let _ = annotations.insert(
                                        "required_phrase".to_owned(),
                                        YamlValue(serde_yaml::Value::String(
                                            "hello world".to_owned(),
                                        )),
                                    );
                                    annotations
                                },
                            },
                        },
                    ],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                },
            }))
        } else {
            VersionedRegistry::V1(ResolvedRegistry {
                registry_url: "TEST_EVENTS".to_owned(),
                groups: vec![
                    ResolvedGroup {
                        id: "event.session.start".to_owned(),
                        r#type: GroupType::Event,
                        brief: "This event represents a session start".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: Some(weaver_semconv::deprecated::Deprecated::Uncategorized {
                            note: "Use session.initialized event instead".to_owned(),
                        }),
                        attributes: vec![
                            Attribute {
                                name: "session.id".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: Some(Examples::Strings(vec![
                                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                                ])),
                                brief: "A unique session identifier".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Basic(
                                    BasicRequirementLevelSpec::Required,
                                ),
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Development),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                            Attribute {
                                name: "session.previous_id".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: Some(Examples::Strings(vec![
                                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                                ])),
                                brief: "The previous session identifier".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Development),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                        ],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        name: Some("session.start".to_owned()),
                        lineage: None,
                        display_name: Some("Session Start Event".to_owned()),
                        body: None,
                        annotations: Some({
                            let mut annotations = BTreeMap::new();
                            let _ = annotations.insert(
                                "required_phrase".to_owned(),
                                YamlValue(serde_yaml::Value::String("hello world".to_owned())),
                            );
                            annotations
                        }),
                    },
                    ResolvedGroup {
                        id: "event.example.event".to_owned(),
                        r#type: GroupType::Event,
                        brief: "An example event".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        name: Some("example.event".to_owned()),
                        lineage: None,
                        display_name: Some("Example Event".to_owned()),
                        body: None,
                        annotations: Some({
                            let mut annotations = BTreeMap::new();
                            let _ = annotations.insert(
                                "required_phrase".to_owned(),
                                YamlValue(serde_yaml::Value::String("hello world".to_owned())),
                            );
                            annotations
                        }),
                    },
                ],
            })
        }
    }

    #[test]
    fn test_json_log() {
        run_json_log_test(false);
    }

    #[test]
    fn test_json_log_v2() {
        run_json_log_test(true);
    }

    fn run_json_log_test(use_v2: bool) {
        let registry = make_events_registry(use_v2);

        // Load samples from JSON file
        let path = "data/logs.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor =
            RegoAdvisor::new(&live_checker, &None, &None).expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 8);
            assert_eq!(cumulative_stats.total_entities_by_type.get("log"), Some(&4));
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&4)
            );

            // Check advisor advice types
            // Expected advice:
            // - missing_attribute: 1 (session.idd does not exist in registry)
            // - required_attribute_not_present: 1 (session.id is required but not present)
            // - not_stable: 2 (1 for session.start event, 1 for session.previous_id attribute)
            // - deprecated: 1 (session.start event is deprecated)
            // - missing_event: 1 (session.test event does not exist in registry)
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_attribute"),
                Some(&1),
                "Expected 1 missing_attribute advice for session.idd"
            );
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("required_attribute_not_present"),
                Some(&1),
                "Expected 1 required_attribute_not_present advice for session.id"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("not_stable"),
                Some(&4),
                "Expected 4 not_stable advice (for session.start event, session.previous_id attribute, and 2 session.id attribute)"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("deprecated"),
                Some(&1),
                "Expected 1 deprecated advice for session.start event"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_event"),
                Some(&1),
                "Expected 1 missing_event advice for session.test"
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_bad_custom_rego() {
        run_bad_custom_rego_test(false);
    }

    #[test]
    fn test_bad_custom_rego_v2() {
        run_bad_custom_rego_test(true);
    }

    fn run_bad_custom_rego_test(use_v2: bool) {
        let registry = make_custom_rego_registry(use_v2);

        let mut samples = vec![Sample::Attribute(
            SampleAttribute::try_from("custom.string=hello").unwrap(),
        )];

        let advisors: Vec<Box<dyn Advisor>> = vec![];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/bad_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            // This should fail with: "error: use of undefined variable `attribu1te_name` is unsafe"

            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("use of undefined variable"));
        }
    }

    #[test]
    fn test_exponential_histogram() {
        run_exponential_histogram_test(false);
    }

    #[test]
    fn test_exponential_histogram_v2() {
        run_exponential_histogram_test(true);
    }

    fn run_exponential_histogram_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // A sample with exponential histogram data points
        let sample = Sample::Metric(SampleMetric {
            name: "system.memory.usage".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "By".to_owned(),
            data_points: Some(DataPoints::ExponentialHistogram(vec![
                SampleExponentialHistogramDataPoint {
                    attributes: vec![],
                    count: 0,
                    sum: None,
                    min: None,
                    max: None,
                    live_check_result: None,
                    scale: 1,
                    zero_count: 0,
                    positive: None,
                    negative: None,
                    flags: 0,
                    zero_threshold: 0.0,
                    exemplars: vec![],
                },
            ])),
            live_check_result: None,
        });
        let mut samples = vec![sample];
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("unexpected_instrument"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
        // Check the live check result for the sample has the correct instrument mismatch message
        let sample = match &samples[0] {
            Sample::Metric(m) => m,
            _ => panic!("Expected a Metric sample"),
        };
        let live_check_result = sample.live_check_result.as_ref().unwrap();
        // Get the instrument_mismatch from all_advice
        let advice = live_check_result
            .all_advice
            .iter()
            .find(|a| a.id == "unexpected_instrument")
            .expect("Expected unexpected_instrument advice");
        assert_eq!(
            advice.message,
            "Instrument should be 'updowncounter', but found 'histogram'."
        );
        assert_eq!(advice.signal_name, Some("system.memory.usage".to_owned()));
        assert_eq!(advice.signal_type, Some("metric".to_owned()));
    }

    #[test]
    fn test_gauge_exemplar_rego() {
        run_gauge_exemplar_rego_test(false);
    }

    #[test]
    fn test_gauge_exemplar_rego_v2() {
        run_gauge_exemplar_rego_test(true);
    }

    fn run_gauge_exemplar_rego_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // A gauge sample with an exemplar
        let mut sample = Sample::Metric(SampleMetric {
            name: "system.uptime".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Gauge),
            unit: "s".to_owned(),
            data_points: Some(DataPoints::Number(vec![SampleNumberDataPoint {
                attributes: vec![],
                value: json!(0.0),
                flags: 0,
                live_check_result: None,
                exemplars: vec![SampleExemplar {
                    timestamp: "".to_owned(),
                    value: json!(0.0),
                    filtered_attributes: vec![],
                    span_id: "".to_owned(),
                    trace_id: "".to_owned(),
                    live_check_result: None,
                }],
            }])),
            live_check_result: None,
        });
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        let result = sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

        assert!(result.is_ok());
        stats.finalize();
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.advice_type_counts.get("low_value"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_summary_unspecified() {
        run_summary_unspecified_test(false);
    }

    #[test]
    fn test_summary_unspecified_v2() {
        run_summary_unspecified_test(true);
    }

    fn run_summary_unspecified_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        let mut samples = vec![
            Sample::Metric(SampleMetric {
                name: "system.memory.usage".to_owned(),
                instrument: SampleInstrument::Unsupported("Summary".to_owned()),
                unit: "By".to_owned(),
                data_points: None,
                live_check_result: None,
            }),
            Sample::Metric(SampleMetric {
                name: "system.memory.usage".to_owned(),
                instrument: SampleInstrument::Unsupported("Unspecified".to_owned()),
                unit: "By".to_owned(),
                data_points: None,
                live_check_result: None,
            }),
        ];
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize();
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("unexpected_instrument"),
                Some(&2)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }
}
