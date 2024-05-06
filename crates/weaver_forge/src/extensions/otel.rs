// SPDX-License-Identifier: Apache-2.0

//! Set of filters, tests, and functions that are specific to the OpenTelemetry project.

use itertools::Itertools;
use minijinja::{ErrorKind, Value};
use serde::de::Error;
use crate::config::CaseConvention;

/// Filters the input value to only include the required "object".
/// A required object is one that has a field named "requirement_level" with the value "required".
/// An object that is "conditionally_required" is not returned by this filter.
pub(crate) fn required(input: Value) -> Result<Vec<Value>, minijinja::Error> {
    let mut rv = vec![];

    for value in input.try_iter()? {
        let required = value.get_attr("requirement_level")?;
        if required.as_str() == Some("required") {
            rv.push(value);
        }
    }
    Ok(rv)
}

/// Filters the input value to only include the non-required "object".
/// A optional object is one that has a field named "requirement_level" which is not "required".
pub(crate) fn not_required(input: Value) -> Result<Vec<Value>, minijinja::Error> {
    let mut rv = vec![];

    for value in input.try_iter()? {
        let required = value.get_attr("requirement_level")?;
        if required.as_str() != Some("required") {
            rv.push(value);
        }
    }
    Ok(rv)
}

/// Converts registry.{namespace}.{other}.{components} to {namespace}.
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input).
pub(crate) fn attribute_registry_namespace(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(parts[1].to_owned())
}

/// Converts registry.{namespace}.{other}.{components} to {Namespace} (title case the namespace).
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input, title case).
pub(crate) fn attribute_registry_title(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(CaseConvention::TitleCase.convert(parts[1]))
}

/// attribute_registry_file: Converts registry.{namespace}.{other}.{components} to attributes-registry/{namespace}.md (kebab-case namespace).
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the file path (kebab-case namespace).
pub(crate) fn attribute_registry_file(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(format!(
        "attributes-registry/{}.md",
        CaseConvention::KebabCase.convert(parts[1])
    ))
}

/// Converts metric.{namespace}.{other}.{components} to {namespace}.
///
/// A [`minijinja::Error`] is returned if the input does not start with "metric" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input).
pub(crate) fn metric_namespace(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "metric" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This metric id `{}` is invalid", input),
        ));
    }
    Ok(parts[1].to_owned())
}

/// Converts {namespace}.{attribute_id} to {namespace}.
///
/// A [`minijinja::Error`] is returned if the input does not have
/// at least two parts. Otherwise, it returns the namespace (first part of the input).
pub(crate) fn attribute_namespace(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute name `{}` is invalid", input),
        ));
    }
    Ok(parts[0].to_owned())
}

/// Compares two attributes by their requirement_level, then name.
fn compare_requirement_level(
    lhs: &Value,
    rhs: &Value,
) -> Result<std::cmp::Ordering, minijinja::Error> {
    fn sort_ordinal_for_requirement(attribute: &Value) -> Result<i32, minijinja::Error> {
        let level = attribute.get_attr("requirement_level")?;
        if level
            .get_attr("conditionally_required")
            .is_ok_and(|v| !v.is_undefined())
        {
            Ok(2)
        } else if level
            .get_attr("recommended")
            .is_ok_and(|v| !v.is_undefined())
        {
            Ok(3)
        } else {
            match level.as_str() {
                Some("required") => Ok(1),
                Some("recommended") => Ok(3),
                Some("opt_in") => Ok(4),
                _ => Err(minijinja::Error::custom(format!(
                    "Expected requirement level, found {}",
                    level
                ))),
            }
        }
    }
    match sort_ordinal_for_requirement(lhs)?.cmp(&sort_ordinal_for_requirement(rhs)?) {
        std::cmp::Ordering::Equal => {
            let lhs_name = lhs.get_attr("name")?;
            let rhs_name = rhs.get_attr("name")?;
            if lhs_name.lt(&rhs_name) {
                Ok(std::cmp::Ordering::Less)
            } else if lhs_name.eq(&rhs_name) {
                Ok(std::cmp::Ordering::Equal)
            } else {
                Ok(std::cmp::Ordering::Greater)
            }
        }
        other => Ok(other),
    }
}

/// Sorts a sequence of attributes by their requirement_level, then name.
pub(crate) fn attribute_sort(input: Value) -> Result<Value, minijinja::Error> {
    let mut errors: Vec<minijinja::Error> = vec![];

    let opt_result = Value::from(
        input
            .try_iter()?
            .sorted_by(|lhs, rhs| {
                // Sorted doesn't allow us to keep errors, so we sneak them into
                // a mutable vector.
                match compare_requirement_level(lhs, rhs) {
                    Ok(result) => result,
                    Err(error) => {
                        errors.push(error);
                        std::cmp::Ordering::Less
                    }
                }
            })
            .to_owned()
            .collect::<Vec<_>>(),
    );

    // If we had an internal error, return the first.
    match errors.pop() {
        Some(err) => Err(err),
        None => Ok(opt_result),
    }
}

/// Checks if the input value is an object with a field named "stability" that has the value "stable".
/// Otherwise, it returns false.
#[must_use]
pub(crate) fn is_stable(input: Value) -> bool {
    let result = input.get_attr("stability");

    if let Ok(stability) = result {
        if let Some(stability) = stability.as_str() {
            return stability == "stable";
        }
    }
    false
}

/// Checks if the input value is an object with a field named "stability" that has the value
/// "experimental". Otherwise, it returns false.
#[must_use]
pub(crate) fn is_experimental(input: Value) -> bool {
    let result = input.get_attr("stability");

    if let Ok(stability) = result {
        if let Some(stability) = stability.as_str() {
            return stability == "experimental";
        }
    }
    false
}

/// Checks if the input value is an object with a field named "stability" that has the value "deprecated".
/// Otherwise, it returns false.
#[must_use]
pub(crate) fn is_deprecated(input: Value) -> bool {
    let result = input.get_attr("deprecated");

    if let Ok(deprecated) = result {
        if let Some(deprecated) = deprecated.as_str() {
            return !deprecated.is_empty();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::sync::Arc;

    use minijinja::value::Object;
    use minijinja::Value;

    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::AttributeType;
    use weaver_semconv::attribute::BasicRequirementLevelSpec;
    use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;
    use weaver_semconv::attribute::RequirementLevel;

    use crate::extensions::otel::{
        attribute_registry_file, attribute_registry_namespace, attribute_registry_title,
        attribute_sort, is_deprecated, is_experimental, is_stable, metric_namespace,
    };

    #[derive(Debug)]
    struct DynAttr {
        id: String,
        r#type: String,
        stability: String,
        deprecated: Option<String>,
    }

    impl Object for DynAttr {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str() {
                Some("id") => Some(Value::from(self.id.as_str())),
                Some("type") => Some(Value::from(self.r#type.as_str())),
                Some("stability") => Some(Value::from(self.stability.as_str())),
                Some("deprecated") => self.deprecated.as_ref().map(|s| Value::from(s.as_str())),
                _ => None,
            }
        }
    }

    #[derive(Debug)]
    struct DynSomethingElse {
        id: String,
        r#type: String,
    }

    impl Object for DynSomethingElse {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str() {
                Some("id") => Some(Value::from(self.id.as_str())),
                Some("type") => Some(Value::from(self.r#type.as_str())),
                _ => None,
            }
        }
    }

    #[test]
    fn test_attribute_registry_namespace() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_namespace(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_namespace(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(attribute_registry_namespace(input).unwrap(), "namespace");

        // An empty string
        let input = "";
        assert!(attribute_registry_namespace(input).is_err());
    }

    #[test]
    fn test_attribute_registry_title() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_title(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_title(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(attribute_registry_title(input).unwrap(), "Namespace");

        // An empty string
        let input = "";
        assert!(attribute_registry_title(input).is_err());
    }

    #[test]
    fn test_attribute_registry_file() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_file(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_file(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(
            attribute_registry_file(input).unwrap(),
            "attributes-registry/namespace.md"
        );

        // An empty string
        let input = "";
        assert!(attribute_registry_file(input).is_err());
    }

    #[test]
    fn test_metric_namespace() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(metric_namespace(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "metric";
        assert!(metric_namespace(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "metric.namespace.other.components";
        assert_eq!(metric_namespace(input).unwrap(), "namespace");

        // An empty string
        let input = "";
        assert!(metric_namespace(input).is_err());
    }

    #[test]
    fn test_is_stable() {
        // An attribute with stability "stable"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(is_stable(attr));

        // An attribute with stability "deprecated"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "deprecated".to_owned(),
            deprecated: None,
        });
        assert!(!is_stable(attr));

        // An object without a stability field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_stable(object));
    }

    #[test]
    fn test_is_experimental() {
        // An attribute with stability "experimental"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "experimental".to_owned(),
            deprecated: None,
        });
        assert!(is_experimental(attr));

        // An attribute with stability "stable"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_experimental(attr));

        // An object without a stability field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_experimental(object));
    }

    #[test]
    fn test_is_deprecated() {
        // An attribute with stability "experimental" and a deprecated field with a value
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "experimental".to_owned(),
            deprecated: Some("This is deprecated".to_owned()),
        });
        assert!(is_deprecated(attr));

        // An attribute with stability "stable" and a deprecated field with a value
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: Some("This is deprecated".to_owned()),
        });
        assert!(is_deprecated(attr));

        // An object without a deprecated field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_deprecated(object));

        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_deprecated(attr));
    }

    #[test]
    fn test_attribute_sort() {
        // Attributes in no particular order.
        let attributes: Vec<Attribute> = vec![
            Attribute {
                name: "rec.a".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "rec.b".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "crec.a".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::ConditionallyRequired { text: "hi".into() },
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "crec.b".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::ConditionallyRequired { text: "hi".into() },
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "rec.c".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Recommended { text: "hi".into() },
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "rec.d".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Recommended { text: "hi".into() },
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "opt.a".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "opt.b".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "req.a".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "req.b".into(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".into(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".into(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
        ];
        let json =
            serde_json::to_value(attributes).expect("Failed to serialize attributes to json.");
        let value = Value::from_serialize(json);
        let result = attribute_sort(value).expect("Failed to sort attributes");
        let result_seq = result
            .try_iter()
            .expect("Result was not a sequence!")
            .collect::<Vec<_>>();
        // Assert that requirement level takes precedence over anything else.
        assert_eq!(result_seq.len(), 10, "Expected 10 items, found {}", result);
        let names: Vec<String> = result_seq
            .iter()
            .map(|item| item.get_attr("name").unwrap().as_str().unwrap().to_owned())
            .collect();
        let expected_names: Vec<String> = vec![
            // Required First
            "req.a".to_owned(),
            "req.b".to_owned(),
            // Conditionally Required Second
            "crec.a".to_owned(),
            "crec.b".to_owned(),
            // Conditionally Recommended + Recommended Third
            "rec.a".to_owned(),
            "rec.b".to_owned(),
            "rec.c".to_owned(),
            "rec.d".to_owned(),
            // OptIn last
            "opt.a".to_owned(),
            "opt.b".to_owned(),
        ];

        for (idx, (result, expected)) in names.iter().zip(expected_names.iter()).enumerate() {
            assert_eq!(
                result, expected,
                "Expected item @ {idx} to have name {expected}, found {names:?}"
            );
        }
    }

    #[test]
    fn test_required_and_not_required_filters() {
        let attrs = vec![
            Attribute {
                name: "attr1".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "attr2".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
            Attribute {
                name: "attr3".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: None,
                deprecated: None,
                tags: None,
                value: None,
            },
        ];

        let result = super::required(Value::from_serialize(&attrs)).unwrap();
        assert_eq!(result.len(), 2);

        let result = super::not_required(Value::from_serialize(&attrs)).unwrap();
        assert_eq!(result.len(), 1);
    }
}
