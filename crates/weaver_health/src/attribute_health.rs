use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use weaver_semconv::attribute::AttributeType;

use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;

use crate::{
    attribute_advice::{Advice, Advisor, Advisory},
    sample::SampleAttribute,
};

/// Checks the health of attributes
pub struct AttributeHealthChecker {
    _registry: ResolvedRegistry, // Keeping this here for future use
    semconv_attributes: HashMap<String, Attribute>,
    semconv_templates: HashMap<String, Attribute>,
    semconv_namespaces: HashSet<String>,
    sample_attributes: Vec<SampleAttribute>,
    advisors: Vec<Box<dyn Advisor>>,
}

impl AttributeHealthChecker {
    #[must_use]
    /// Create a new AttributeHealthChecker
    pub fn new(
        attributes: Vec<SampleAttribute>,
        registry: ResolvedRegistry,
        advisors: Vec<Box<dyn Advisor>>,
    ) -> Self {
        // Create a hashmap of attributes for quick lookup
        let mut semconv_attributes = HashMap::new();
        let mut semconv_templates = HashMap::new();
        let mut semconv_namespaces = HashSet::new();

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
                // Extract namespace (everything to the left of the last dot)
                // repeat until the last dot is found
                let mut name = attribute.name.clone();
                while let Some(last_dot_pos) = name.rfind('.') {
                    let namespace = name[..last_dot_pos].to_string();
                    let _ = semconv_namespaces.insert(namespace);
                    name = name[..last_dot_pos].to_string();
                }
            }
        }
        AttributeHealthChecker {
            _registry: registry,
            semconv_attributes,
            semconv_templates,
            semconv_namespaces,
            sample_attributes: attributes,
            advisors,
        }
    }

    /// Find an attribute in the registry
    #[must_use]
    pub fn find_attribute(&self, name: &str) -> Option<&Attribute> {
        self.semconv_attributes.get(name)
    }

    /// Find a template in the registry
    #[must_use]
    pub fn find_template(&self, attribute_name: &str) -> Option<&Attribute> {
        if let Some(last_dot_pos) = attribute_name.rfind('.') {
            let new_name = &attribute_name[..last_dot_pos];
            if let Some(attribute) = self.semconv_templates.get(new_name) {
                Some(attribute)
            } else {
                self.find_template(new_name)
            }
        } else {
            None
        }
    }

    /// Find a namespace in the registry
    #[must_use]
    pub fn find_namespace(&self, namespace: &str) -> Option<String> {
        let mut namespace = namespace.to_owned();
        while !self.semconv_namespaces.contains(&namespace) {
            if let Some(last_dot_pos) = namespace.rfind('.') {
                namespace = namespace[..last_dot_pos].to_string();
            } else {
                return None;
            }
        }
        Some(namespace)
    }

    /// Find an attribute from a namespace search
    #[must_use]
    pub fn find_attribute_from_namespace(&self, namespace: &str) -> Option<&Attribute> {
        if let Some(attribute) = self.find_attribute(namespace) {
            Some(attribute)
        } else if let Some(last_dot_pos) = namespace.rfind('.') {
            let new_namespace = &namespace[..last_dot_pos];
            self.find_attribute_from_namespace(new_namespace)
        } else {
            None
        }
    }

    /// Run advisors on every attribute in the list
    #[must_use]
    pub fn check_attributes(&self) -> Vec<HealthAttribute> {
        let mut results = Vec::new();
        for sample_attribute in self.sample_attributes.iter() {
            // clone the sample attribute into the result
            let mut attribute_result = HealthAttribute::new(sample_attribute.clone());

            // find the attribute in the registry
            let semconv_attribute = {
                if let Some(attribute) = self.find_attribute(&sample_attribute.name) {
                    Some(attribute)
                } else {
                    self.find_template(&sample_attribute.name)
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
                if let Some(attribute) = semconv_attribute {
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
            for advisor in self.advisors.iter() {
                if let Some(advice) = advisor.advise(sample_attribute, self, semconv_attribute) {
                    attribute_result.add_advice(advice);
                }
            }

            results.push(attribute_result);
        }
        results
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

#[cfg(test)]
mod tests {
    use crate::attribute_advice::{
        CorrectCaseAdvisor, DeprecatedAdvisor, EnumAdvisor, NamespaceAdvisor, StabilityAdvisor,
        TypeAdvisor,
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
            Box::new(CorrectCaseAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(NamespaceAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let health_checker = AttributeHealthChecker::new(attributes, registry, advisors);

        let results = health_checker.check_attributes();

        assert_eq!(results.len(), 10);

        assert!(results[0].all_advice.is_empty());

        assert_eq!(results[1].all_advice.len(), 3);
        assert_eq!(results[1].all_advice[0].key, "missing_attribute");
        assert_eq!(
            results[1].all_advice[0].value,
            Value::String("testString2".to_owned())
        );
        assert_eq!(
            results[1].all_advice[0].message,
            "Does not exist in the registry"
        );
        assert_eq!(results[1].all_advice[1].key, "correct_case");
        assert_eq!(results[1].all_advice[1].value, Value::Bool(false));
        assert_eq!(results[1].all_advice[1].message, "Is not in snake case");
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

        assert_eq!(results[7].all_advice.len(), 2);
        assert_eq!(results[7].all_advice[0].key, "missing_attribute");
        assert_eq!(
            results[7].all_advice[0].value,
            Value::String("test.string.not.allowed".to_owned())
        );
        assert_eq!(
            results[7].all_advice[0].message,
            "Does not exist in the registry"
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
    }
}
