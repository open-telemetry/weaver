use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;

use crate::{
    attribute_advice::{Advice, Advisor, Severity},
    sample::SampleAttribute,
};

/// Checks the health of attributes
pub struct AttributeHealthChecker {
    _registry: ResolvedRegistry, // Keeping this here for future use
    semconv_attributes: HashMap<String, Attribute>,
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
        let semconv_attributes = registry
            .groups
            .iter()
            .flat_map(|group| group.attributes.iter())
            .map(|attribute| (attribute.name.clone(), attribute.clone()))
            .collect();
        AttributeHealthChecker {
            sample_attributes: attributes,
            _registry: registry,
            semconv_attributes,
            advisors,
        }
    }

    fn find_attribute(&self, name: &str) -> Option<&Attribute> {
        self.semconv_attributes.get(name)
    }

    /// Run advisors on every attribute in the list
    #[must_use]
    pub fn check_attributes(&self) -> Vec<HealthAttribute> {
        let mut results = Vec::new();
        for sample_attribute in self.sample_attributes.iter() {
            // clone the sample attribute into the result
            let mut attribute_result = HealthAttribute::new(sample_attribute.clone());

            // find the attribute in the registry
            let semconv_attribute = self.find_attribute(&sample_attribute.name);
            if semconv_attribute.is_none() {
                attribute_result.all_advice.push(Advice {
                    key: "attribute_match".to_owned(),
                    value: Value::Bool(false),
                    message: "Does not exist in the registry".to_owned(),
                    severity: Severity::Error,
                });
            }

            // run advisors on the attribute
            for advisor in self.advisors.iter() {
                if let Some(advice) = advisor.advise(sample_attribute, self, semconv_attribute) {
                    attribute_result.all_advice.push(advice);
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
}

impl HealthAttribute {
    /// Create a new HealthAttribute
    #[must_use]
    pub fn new(sample_attribute: SampleAttribute) -> Self {
        HealthAttribute {
            sample_attribute,
            all_advice: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute_advice::{CorrectCaseAdvisor, DeprecatedAdvisor};

    use super::*;
    use serde_json::Value;
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::{
        attribute::{AttributeType, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel},
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
                        stability: Some(Stability::Stable),
                        deprecated: Some(weaver_semconv::deprecated::Deprecated::Uncategorized {
                            note: "".to_owned(),
                        }),
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

        let attributes = vec![
            SampleAttribute {
                name: "test.string".to_owned(),
            },
            SampleAttribute {
                name: "testString2".to_owned(),
            },
            SampleAttribute {
                name: "test.deprecated".to_owned(),
            },
            SampleAttribute {
                name: "aws.s3.bucket.name".to_owned(),
            },
        ];

        let advisors: Vec<Box<dyn Advisor>> =
            vec![Box::new(DeprecatedAdvisor), Box::new(CorrectCaseAdvisor)];

        let health_checker = AttributeHealthChecker::new(attributes, registry, advisors);

        let results = health_checker.check_attributes();

        assert_eq!(results.len(), 4);

        assert!(results[0].all_advice.is_empty());

        assert!(results[1].all_advice.len() == 2);
        assert_eq!(results[1].all_advice[0].key, "attribute_match");
        assert_eq!(results[1].all_advice[0].value, Value::Bool(false));
        assert_eq!(
            results[1].all_advice[0].message,
            "Does not exist in the registry"
        );
        assert_eq!(results[1].all_advice[1].key, "correct_case");
        assert_eq!(results[1].all_advice[1].value, Value::Bool(false));
        assert_eq!(results[1].all_advice[1].message, "Is not in snake case");

        assert!(results[2].all_advice.len() == 1);
        assert_eq!(results[2].all_advice[0].key, "is_deprecated");
        assert_eq!(results[2].all_advice[0].value, Value::Bool(true));
        assert_eq!(results[2].all_advice[0].message, "Is deprecated");

        assert!(results[3].all_advice.len() == 1);
        assert_eq!(results[3].all_advice[0].key, "attribute_match");
        assert_eq!(results[3].all_advice[0].value, Value::Bool(false));
        assert_eq!(
            results[3].all_advice[0].message,
            "Does not exist in the registry"
        );
    }
}
