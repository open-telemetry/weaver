use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use weaver_checker::violation::{Advice, Advisory};
use weaver_semconv::attribute::AttributeType;

use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;

use crate::{attribute_advice::Advisor, sample::SampleAttribute};

/// Checks the health of attributes
#[derive(Serialize)]
pub struct AttributeHealthChecker {
    /// The resolved registry
    pub registry: ResolvedRegistry,
    semconv_attributes: HashMap<String, Attribute>,
    semconv_templates: HashMap<String, Attribute>,
    #[serde(skip)]
    advisors: Vec<Box<dyn Advisor>>,
}

impl AttributeHealthChecker {
    #[must_use]
    /// Create a new AttributeHealthChecker
    pub fn new(registry: ResolvedRegistry, advisors: Vec<Box<dyn Advisor>>) -> Self {
        // Create a hashmap of attributes for quick lookup
        let mut semconv_attributes = HashMap::new();
        let mut semconv_templates = HashMap::new();

        for group in &registry.groups {
            for attribute in &group.attributes {
                match attribute.r#type {
                    AttributeType::Template(_) => {
                        let _ = semconv_templates.insert(attribute.name.clone(), attribute.clone());
                    }
                    _ => {
                        let _ =
                            semconv_attributes.insert(attribute.name.clone(), attribute.clone());
                    }
                }
            }
        }
        AttributeHealthChecker {
            registry,
            semconv_attributes,
            semconv_templates,
            advisors,
        }
    }

    /// Add an advisor
    pub fn add_advisor(&mut self, advisor: Box<dyn Advisor>) {
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
        for (template_name, attribute) in &self.semconv_templates {
            if attribute_name.starts_with(template_name) {
                return Some(attribute);
            }
        }
        None
    }

    /// Create a health attribute from a sample attribute
    #[must_use]
    pub fn create_health_attribute(
        &mut self,
        sample_attribute: &SampleAttribute,
    ) -> HealthAttribute {
        // clone the sample attribute into the result
        let mut attribute_result = HealthAttribute::new(sample_attribute.clone());

        // find the attribute in the registry
        let semconv_attribute = {
            if let Some(attribute) = self.find_attribute(&sample_attribute.name) {
                Some(attribute.clone())
            } else {
                self.find_template(&sample_attribute.name).cloned()
            }
        };

        if semconv_attribute.is_none() {
            attribute_result.add_advice(Advice {
                key: "missing_attribute".to_owned(),
                value: Value::String(sample_attribute.name.clone()),
                message: "Does not exist in the registry".to_owned(),
                advisory: Advisory::Violation,
            });
        } else {
            // Provide an info advice if the attribute is a template
            if let Some(attribute) = &semconv_attribute {
                if let AttributeType::Template(_) = attribute.r#type {
                    attribute_result.add_advice(Advice {
                        key: "template_attribute".to_owned(),
                        value: Value::String(attribute.name.clone()),
                        message: "Is a template".to_owned(),
                        advisory: Advisory::Information,
                    });
                }
            }
        }

        // run advisors on the attribute
        for advisor in self.advisors.iter_mut() {
            if let Ok(advices) = advisor.advise(sample_attribute, semconv_attribute.as_ref()) {
                for advice in advices {
                    attribute_result.add_advice(advice);
                }
            }
        }

        attribute_result
    }

    /// Run advisors on every attribute in the list
    #[must_use]
    pub fn check_attributes(&mut self, sample_attributes: Vec<SampleAttribute>) -> HealthReport {
        let mut health_report = HealthReport {
            attributes: Vec::new(),
            statistics: HealthStatistics::new(),
        };

        for sample_attribute in sample_attributes.iter() {
            let attribute_result = self.create_health_attribute(sample_attribute);

            // Update statistics
            health_report.statistics.update(&attribute_result);
            health_report.attributes.push(attribute_result);
        }
        health_report
    }
}

/// Represents a health attribute parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthAttribute {
    /// The sample attribute
    pub sample_attribute: SampleAttribute,
    /// Advice on the attribute
    pub all_advice: Vec<Advice>,
    /// The highest advisory level
    pub highest_advisory: Option<Advisory>,
}

impl HealthAttribute {
    /// Create a new HealthAttribute
    #[must_use]
    pub fn new(sample_attribute: SampleAttribute) -> Self {
        HealthAttribute {
            sample_attribute,
            all_advice: Vec::new(),
            highest_advisory: None,
        }
    }

    /// Add an advice to the attribute and update the highest advisory level
    pub fn add_advice(&mut self, advice: Advice) {
        let advisory = advice.advisory.clone();
        if let Some(previous_highest) = &self.highest_advisory {
            if previous_highest < &advisory {
                self.highest_advisory = Some(advisory);
            }
        } else {
            self.highest_advisory = Some(advisory);
        }
        self.all_advice.push(advice);
    }
}

/// A health report for a set of attributes
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthReport {
    /// The health attributes
    pub attributes: Vec<HealthAttribute>,
    /// The statistics for the report
    pub statistics: HealthStatistics,
}

impl HealthReport {
    /// Return true if there are any violations in the report
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.statistics
            .highest_advisory_counts
            .contains_key(&Advisory::Violation)
    }
}

/// The statistics for a health report
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthStatistics {
    /// The total number of attributes
    pub total_attributes: usize,
    /// The total number of advisories
    pub total_advisories: usize,
    /// The number of each advisory level
    pub advisory_counts: HashMap<Advisory, usize>,
    /// The number of attributes with each highest advisory level
    pub highest_advisory_counts: HashMap<Advisory, usize>,
    /// The number of attributes with no advice
    pub no_advice_count: usize,
}

impl Default for HealthStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthStatistics {
    /// Create a new empty HealthStatistics
    #[must_use]
    pub fn new() -> Self {
        HealthStatistics {
            total_attributes: 0,
            total_advisories: 0,
            advisory_counts: HashMap::new(),
            highest_advisory_counts: HashMap::new(),
            no_advice_count: 0,
        }
    }

    /// Update statistics based on a health attribute
    pub fn update(&mut self, attribute_result: &HealthAttribute) {
        self.total_attributes += 1;

        // Count of advisories by type
        for advice in &attribute_result.all_advice {
            // Count of total advisories
            self.total_advisories += 1;

            let advisory_count = self
                .advisory_counts
                .entry(advice.advisory.clone())
                .or_insert(0);
            *advisory_count += 1;
        }

        // Count of attributes with the highest advisory level
        if let Some(highest_advisory) = &attribute_result.highest_advisory {
            let highest_advisory_count = self
                .highest_advisory_counts
                .entry(highest_advisory.clone())
                .or_insert(0);
            *highest_advisory_count += 1;
        }

        // Count of attributes with no advice
        if attribute_result.all_advice.is_empty() {
            self.no_advice_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute_advice::{
        DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
    };

    use super::*;
    use serde_json::{json, Value};
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

    #[test]
    fn test_attribute_health_checker() {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![ResolvedGroup {
                id: "test.comprehensive.internal".to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                constraints: vec![],
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

        let mut attributes = vec![
            SampleAttribute {
                name: "test.string".to_owned(),
                r#type: None,
                value: Some(Value::String("hello".to_owned())),
            },
            SampleAttribute {
                name: "testString2".to_owned(),
                r#type: None,
                value: None,
            },
            SampleAttribute {
                name: "test.deprecated".to_owned(),
                r#type: None,
                value: Some(Value::Number(42.into())),
            },
            SampleAttribute {
                name: "aws.s3.bucket.name".to_owned(),
                r#type: None,
                value: None,
            },
            SampleAttribute {
                name: "test.enum".to_owned(),
                r#type: None,
                value: Some(Value::String("foo".to_owned())),
            },
            SampleAttribute {
                name: "test.enum".to_owned(),
                r#type: None,
                value: Some(Value::String("example_variant1".to_owned())),
            },
            SampleAttribute {
                name: "test.enum".to_owned(),
                r#type: None,
                value: Some(json!(42.42)),
            },
            SampleAttribute {
                name: "test.string.not.allowed".to_owned(),
                r#type: None,
                value: Some(Value::String("example_value".to_owned())),
            },
            SampleAttribute {
                name: "test.extends".to_owned(),
                r#type: None,
                value: Some(Value::String("new_value".to_owned())),
            },
            SampleAttribute {
                name: "test.template.my.key".to_owned(),
                r#type: None,
                value: Some(Value::Number(42.into())),
            },
        ];

        for attribute in attributes.iter_mut() {
            attribute.infer_type();
        }

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut health_checker = AttributeHealthChecker::new(registry, advisors);
        let rego_advisor =
            RegoAdvisor::new(&health_checker, &None, &None).expect("Failed to create Rego advisor");
        health_checker.add_advisor(Box::new(rego_advisor));

        let report = health_checker.check_attributes(attributes);
        let mut results = report.attributes;

        assert_eq!(results.len(), 10);

        assert!(results[0].all_advice.is_empty());

        assert_eq!(results[1].all_advice.len(), 3);
        // make a sort of the advice
        results[1].all_advice.sort_by(|a, b| a.key.cmp(&b.key));
        assert_eq!(results[1].all_advice[0].key, "invalid_format");
        assert_eq!(
            results[1].all_advice[0].value,
            Value::String("testString2".to_owned())
        );
        assert_eq!(
            results[1].all_advice[0].message,
            "Does not match name formatting rules"
        );
        assert_eq!(results[1].all_advice[1].key, "missing_attribute");
        assert_eq!(
            results[1].all_advice[1].value,
            Value::String("testString2".to_owned())
        );
        assert_eq!(
            results[1].all_advice[1].message,
            "Does not exist in the registry"
        );
        assert_eq!(results[1].all_advice[2].key, "missing_namespace");
        assert_eq!(
            results[1].all_advice[2].value,
            Value::String("testString2".to_owned())
        );
        assert_eq!(
            results[1].all_advice[2].message,
            "Does not have a namespace"
        );

        assert_eq!(results[2].all_advice.len(), 3);
        assert_eq!(results[2].all_advice[0].key, "deprecated");
        assert_eq!(
            results[2].all_advice[0].value,
            Value::String("uncategorized".to_owned())
        );
        assert_eq!(results[2].all_advice[0].message, "note");

        assert_eq!(results[2].all_advice[1].key, "stability");
        assert_eq!(
            results[2].all_advice[1].value,
            Value::String("development".to_owned())
        );
        assert_eq!(results[2].all_advice[1].message, "Is not stable");

        assert_eq!(results[2].all_advice[2].key, "type_mismatch");
        assert_eq!(
            results[2].all_advice[2].value,
            Value::String("int".to_owned())
        );
        assert_eq!(results[2].all_advice[2].message, "Type should be `string`");

        assert_eq!(results[2].highest_advisory, Some(Advisory::Violation));

        assert_eq!(results[3].all_advice.len(), 1);
        assert_eq!(results[3].all_advice[0].key, "missing_attribute");
        assert_eq!(
            results[3].all_advice[0].value,
            Value::String("aws.s3.bucket.name".to_owned())
        );
        assert_eq!(
            results[3].all_advice[0].message,
            "Does not exist in the registry"
        );

        assert_eq!(results[4].all_advice.len(), 1);
        assert_eq!(results[4].all_advice[0].key, "undefined_enum_variant");
        assert_eq!(
            results[4].all_advice[0].value,
            Value::String("foo".to_owned())
        );
        assert_eq!(results[4].all_advice[0].message, "Is not a defined variant");
        assert_eq!(results[4].highest_advisory, Some(Advisory::Information));

        assert_eq!(results[6].all_advice.len(), 1);
        assert_eq!(results[6].all_advice[0].key, "type_mismatch");
        assert_eq!(
            results[6].all_advice[0].value,
            Value::String("double".to_owned())
        );
        assert_eq!(
            results[6].all_advice[0].message,
            "Type should be `string` or `int`"
        );

        // Make a sort of the advice
        results[7].all_advice.sort_by(|a, b| a.key.cmp(&b.key));
        assert_eq!(results[7].all_advice.len(), 3);

        assert_eq!(results[7].all_advice[0].key, "extends_namespace");
        assert_eq!(
            results[7].all_advice[0].value,
            Value::String("test".to_owned())
        );
        assert_eq!(
            results[7].all_advice[0].message,
            "Extends existing namespace"
        );
        assert_eq!(results[7].all_advice[1].key, "illegal_namespace");
        assert_eq!(
            results[7].all_advice[1].value,
            Value::String("test.string".to_owned())
        );
        assert_eq!(
            results[7].all_advice[1].message,
            "Namespace matches existing attribute"
        );
        assert_eq!(results[7].all_advice[2].key, "missing_attribute");
        assert_eq!(
            results[7].all_advice[2].value,
            Value::String("test.string.not.allowed".to_owned())
        );
        assert_eq!(
            results[7].all_advice[2].message,
            "Does not exist in the registry"
        );

        assert_eq!(results[8].all_advice.len(), 2);
        assert_eq!(results[8].all_advice[0].key, "missing_attribute");
        assert_eq!(
            results[8].all_advice[0].value,
            Value::String("test.extends".to_owned())
        );
        assert_eq!(
            results[8].all_advice[0].message,
            "Does not exist in the registry"
        );
        assert_eq!(results[8].all_advice[1].key, "extends_namespace");
        assert_eq!(
            results[8].all_advice[1].value,
            Value::String("test".to_owned())
        );
        assert_eq!(
            results[8].all_advice[1].message,
            "Extends existing namespace"
        );

        // test.template
        assert_eq!(results[9].all_advice.len(), 2);
        assert_eq!(results[9].all_advice[0].key, "template_attribute");
        assert_eq!(
            results[9].all_advice[0].value,
            Value::String("test.template".to_owned())
        );
        assert_eq!(results[9].all_advice[0].message, "Is a template");
        assert_eq!(results[9].all_advice[1].key, "type_mismatch");
        assert_eq!(
            results[9].all_advice[1].value,
            Value::String("int".to_owned())
        );
        assert_eq!(results[9].all_advice[1].message, "Type should be `string`");

        // Check statistics
        let stats = report.statistics;
        assert_eq!(stats.total_attributes, 10);
        assert_eq!(stats.total_advisories, 16);
        assert_eq!(stats.advisory_counts.len(), 3);
        assert_eq!(stats.advisory_counts[&Advisory::Violation], 10);
        assert_eq!(stats.advisory_counts[&Advisory::Information], 4);
        assert_eq!(stats.advisory_counts[&Advisory::Improvement], 2);
        assert_eq!(stats.highest_advisory_counts.len(), 2);
        assert_eq!(stats.highest_advisory_counts[&Advisory::Violation], 7);
        assert_eq!(stats.highest_advisory_counts[&Advisory::Information], 1);
        assert_eq!(stats.no_advice_count, 2);
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
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                constraints: vec![],
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

        let mut attributes = vec![
            SampleAttribute {
                name: "custom.string".to_owned(),
                r#type: None,
                value: Some(Value::String("hello".to_owned())),
            },
            SampleAttribute {
                name: "test.string".to_owned(),
                r#type: None,
                value: None,
            },
        ];

        for attribute in attributes.iter_mut() {
            attribute.infer_type();
        }

        let advisors: Vec<Box<dyn Advisor>> = vec![];

        let mut health_checker = AttributeHealthChecker::new(registry, advisors);
        let rego_advisor = RegoAdvisor::new(
            &health_checker,
            &Some("data/policies/advice/".into()),
            &Some("data/jq/test.jq".into()),
        )
        .expect("Failed to create Rego advisor");
        health_checker.add_advisor(Box::new(rego_advisor));

        let report = health_checker.check_attributes(attributes);
        let results = report.attributes;

        assert_eq!(results.len(), 2);

        assert!(results[0].all_advice.is_empty());

        assert_eq!(results[1].all_advice.len(), 2);

        assert_eq!(results[1].all_advice[0].key, "missing_attribute");
        assert_eq!(
            results[1].all_advice[0].value,
            Value::String("test.string".to_owned())
        );
        assert_eq!(
            results[1].all_advice[0].message,
            "Does not exist in the registry"
        );
        assert_eq!(results[1].all_advice[1].key, "contains_test");
        assert_eq!(
            results[1].all_advice[1].value,
            Value::String("test.string".to_owned())
        );
        assert_eq!(
            results[1].all_advice[1].message,
            "Name must not contain 'test'"
        );

        // Check statistics
        let stats = report.statistics;
        assert_eq!(stats.total_attributes, 2);
        assert_eq!(stats.total_advisories, 2);
        assert_eq!(stats.advisory_counts.len(), 1);
        assert_eq!(stats.advisory_counts[&Advisory::Violation], 2);
        assert_eq!(stats.highest_advisory_counts.len(), 1);
        assert_eq!(stats.highest_advisory_counts[&Advisory::Violation], 1);
        assert_eq!(stats.no_advice_count, 1);
    }
}
