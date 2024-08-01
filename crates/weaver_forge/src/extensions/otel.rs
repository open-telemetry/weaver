// SPDX-License-Identifier: Apache-2.0

//! Set of filters, tests, and functions that are specific to the OpenTelemetry project.

use itertools::Itertools;
use minijinja::value::ValueKind;
use minijinja::{ErrorKind, Value};
use serde::de::Error;

use crate::config::CaseConvention;
use crate::extensions::case::{
    camel_case, kebab_case, pascal_case, screaming_snake_case, snake_case,
};

const TEMPLATE_PREFIX: &str = "template[";
const TEMPLATE_SUFFIX: &str = "]";

/// Add OpenTelemetry specific tests and filters to the environment.
pub(crate) fn add_tests_and_filters(env: &mut minijinja::Environment<'_>) {
    env.add_filter("attribute_namespace", attribute_namespace);
    env.add_filter("attribute_registry_namespace", attribute_registry_namespace);
    env.add_filter("attribute_registry_title", attribute_registry_title);
    env.add_filter("attribute_registry_file", attribute_registry_file);
    env.add_filter("attribute_sort", attribute_sort);
    env.add_filter("metric_namespace", metric_namespace);
    env.add_filter("required", required);
    env.add_filter("not_required", not_required);
    env.add_filter("instantiated_type", instantiated_type);
    env.add_filter("enum_type", enum_type);
    env.add_filter("kebab_case_const", kebab_case_const);
    env.add_filter("pascal_case_const", pascal_case_const);
    env.add_filter("camel_case_const", camel_case_const);
    env.add_filter("snake_case_const", snake_case_const);
    env.add_filter("screaming_snake_case_const", screaming_snake_case_const);
    env.add_filter("print_member_value", print_member_value);

    env.add_test("stable", is_stable);
    env.add_test("experimental", is_experimental);
    env.add_test("deprecated", is_deprecated);
    env.add_test("enum", is_enum);
    env.add_test("simple_type", is_simple_type);
    env.add_test("template_type", is_template_type);
    env.add_test("enum_type", is_enum_type);
}

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

/// Converts a semconv id into semconv constant following the namespacing rules and the
/// kebab case convention.
pub(crate) fn kebab_case_const(input: &str) -> String {
    // Remove all _ and convert to the kebab case
    kebab_case(&input.replace('_', ""))
}

/// Converts a semconv id into semconv constant following the namespacing rules and the
/// pascal case convention.
pub(crate) fn pascal_case_const(input: &str) -> String {
    // Remove all _ and convert to the pascal case
    pascal_case(&input.replace('_', ""))
}

/// Converts a semconv id into semconv constant following the namespacing rules and the
/// camel case convention.
pub(crate) fn camel_case_const(input: &str) -> String {
    // Remove all _ and convert to the camel case
    camel_case(&input.replace('_', ""))
}

/// Converts a semconv id into semconv constant following the namespacing rules and the
/// snake case convention.
pub(crate) fn snake_case_const(input: &str) -> String {
    // Remove all _ and convert to the snake case
    snake_case(&input.replace('_', ""))
}

/// Converts a semconv id into semconv constant following the namespacing rules and the
/// screaming snake case convention.
pub(crate) fn screaming_snake_case_const(input: &str) -> String {
    // Remove all _ and convert to the screaming snake case
    screaming_snake_case(&input.replace('_', ""))
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
pub(crate) fn is_stable(input: &Value) -> bool {
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
pub(crate) fn is_experimental(input: &Value) -> bool {
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
pub(crate) fn is_deprecated(input: &Value) -> bool {
    let result = input.get_attr("deprecated");

    if let Ok(deprecated) = result {
        if let Some(deprecated) = deprecated.as_str() {
            return !deprecated.is_empty();
        }
    }
    false
}

/// Returns the instantiated type of the input type.
pub(crate) fn instantiated_type(attr_type: &Value) -> Result<String, minijinja::Error> {
    if is_simple_type(attr_type) {
        return Ok(attr_type
            .as_str()
            .expect("should never happen, already tested in is_template_type")
            .to_owned());
    }
    if is_template_type(attr_type) {
        let attr_type = attr_type
            .as_str()
            .expect("should never happen, already tested in is_template_type");
        let end = attr_type.len() - TEMPLATE_SUFFIX.len();
        return Ok(attr_type[TEMPLATE_PREFIX.len()..end].to_owned());
    }
    if is_enum_type(attr_type) {
        return enum_type(attr_type);
    }
    Err(minijinja::Error::custom(format!(
        "Expected simple type, template type, or enum type, found {}",
        attr_type
    )))
}

/// Converts an enum member value into:
/// - A quoted string if the input is a string.
/// - A non-quoted string if the input is a number or a boolean.
/// - An empty string otherwise.
pub(crate) fn print_member_value(input: &Value) -> String {
    match input.kind() {
        ValueKind::String => format!("\"{}\"", input),
        ValueKind::Number => input.to_string(),
        ValueKind::Bool => input.to_string(),
        _ => "".to_owned(),
    }
}

/// Returns the inferred enum type of the input type or an error if the input type is not an enum.
pub(crate) fn enum_type(attr_type: &Value) -> Result<String, minijinja::Error> {
    if let Ok(members) = attr_type.get_attr("members") {
        // Infer the enum type from the members.
        let mut inferred_type: Option<String> = None;
        for member in members.try_iter()? {
            let value = member.get_attr("value")?;
            let member_type = match value.kind() {
                ValueKind::Number => {
                    if value.as_i64().is_some() {
                        "int"
                    } else {
                        "double"
                    }
                }
                ValueKind::String => "string",
                _ => {
                    return Err(minijinja::Error::custom(format!(
                        "Enum values are expected to be int, double, or string, found {}",
                        value
                    )));
                }
            };
            inferred_type = match inferred_type {
                Some(current_inferred_type) => {
                    if current_inferred_type != member_type {
                        // If the inferred type is different from the member type, then the enum
                        // type is "promoted" to a string.
                        Some("string".to_owned())
                    } else {
                        Some(current_inferred_type)
                    }
                }
                None => Some(member_type.to_owned()),
            };
        }

        return inferred_type.ok_or_else(|| minijinja::Error::custom("Empty enum type"));
    }
    Err(minijinja::Error::custom(format!(
        "Expected enum type, found {}",
        attr_type
    )))
}

/// Returns true if the input type is a simple type.
pub(crate) fn is_simple_type(attr_type: &Value) -> bool {
    if let Some(attr_type) = attr_type.as_str() {
        matches!(
            attr_type,
            "string"
                | "string[]"
                | "int"
                | "int[]"
                | "double"
                | "double[]"
                | "boolean"
                | "boolean[]"
        )
    } else {
        false
    }
}

/// Returns true if the input type is a template type.
pub(crate) fn is_template_type(attr_type: &Value) -> bool {
    if let Some(attr_type) = attr_type.as_str() {
        if attr_type.starts_with(TEMPLATE_PREFIX) && attr_type.ends_with(TEMPLATE_SUFFIX) {
            let end = attr_type.len() - TEMPLATE_SUFFIX.len();
            return is_simple_type(&Value::from(
                attr_type[TEMPLATE_PREFIX.len()..end].to_owned(),
            ));
        }
    }
    false
}

/// Returns true if the input type is an enum type.
pub(crate) fn is_enum_type(attr_type: &Value) -> bool {
    // Check the presence of the "members" field.
    if let Ok(v) = attr_type.get_attr("members") {
        // Returns true if the "members" field is defined.
        return !v.is_undefined();
    }
    false
}

/// Returns true if the input attribute has an enum type.
pub(crate) fn is_enum(attr: &Value) -> bool {
    // Check presence of the "type" field.
    let attr_type = attr.get_attr("type");
    if let Ok(attr_type) = attr_type {
        return is_enum_type(&attr_type);
    }
    false
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::sync::Arc;

    use minijinja::value::Object;
    use minijinja::{Environment, Value};
    use serde::Serialize;

    use crate::extensions::otel::{
        add_tests_and_filters, attribute_registry_file, attribute_registry_namespace,
        attribute_registry_title, attribute_sort, is_deprecated, is_experimental, is_stable,
        metric_namespace,
    };
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::BasicRequirementLevelSpec;
    use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;
    use weaver_semconv::attribute::RequirementLevel;
    use weaver_semconv::attribute::{AttributeType, EnumEntriesSpec, TemplateTypeSpec, ValueSpec};

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
        assert!(is_stable(&attr));

        // An attribute with stability "deprecated"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "deprecated".to_owned(),
            deprecated: None,
        });
        assert!(!is_stable(&attr));

        // An object without a stability field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_stable(&object));
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
        assert!(is_experimental(&attr));

        // An attribute with stability "stable"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_experimental(&attr));

        // An object without a stability field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_experimental(&object));
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
        assert!(is_deprecated(&attr));

        // An attribute with stability "stable" and a deprecated field with a value
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: Some("This is deprecated".to_owned()),
        });
        assert!(is_deprecated(&attr));

        // An object without a deprecated field
        let object = Value::from_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_deprecated(&object));

        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_deprecated(&attr));
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
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
                prefix: false,
            },
        ];

        let result = super::required(Value::from_serialize(&attrs)).unwrap();
        assert_eq!(result.len(), 2);

        let result = super::not_required(Value::from_serialize(&attrs)).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_instantiated_type() {
        #[derive(Serialize)]
        struct Ctx {
            attr_type: AttributeType,
        }

        fn eval(
            env: &Environment<'_>,
            expr: &str,
            attr_type: AttributeType,
        ) -> Result<String, minijinja::Error> {
            env.render_str(expr, Ctx { attr_type })
        }

        let mut env = Environment::new();

        add_tests_and_filters(&mut env);

        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int)
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints)
            )
            .unwrap(),
            "int[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Int)
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double)
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles)
            )
            .unwrap(),
            "double[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Double)
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean)
            )
            .unwrap(),
            "boolean"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans)
            )
            .unwrap(),
            "boolean[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Boolean)
            )
            .unwrap(),
            "boolean"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings)
            )
            .unwrap(),
            "string[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::String)
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), 2.into()])
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.1.into(), 2.1.into()])
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec!["value1".into(), "value2".into()])
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), 2.1.into()])
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), "two".into()])
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.0.into(), "two".into()])
            )
            .unwrap(),
            "string"
        );
        assert!(eval(
            &env,
            "{{ 'something else' | instantiated_type }}",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
        )
        .is_err());
    }

    #[test]
    fn test_is_simple_type() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str(
                "{% if 'int' is simple_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "true"
        );
        assert_eq!(
            env.render_str(
                "{% if 'int[]' is simple_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "true"
        );
        assert_eq!(
            env.render_str(
                "{% if 'template[double]' is simple_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "false"
        );
    }

    #[test]
    fn test_is_template_type() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str(
                "{% if 'int' is template_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "false"
        );
        assert_eq!(
            env.render_str(
                "{% if 'int[]' is template_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "false"
        );
        assert_eq!(
            env.render_str(
                "{% if 'template[double]' is template_type %}true{% else %}false{% endif %}",
                &ctx,
            )
            .unwrap(),
            "true"
        );
    }

    #[test]
    fn test_is_enum() {
        #[derive(Serialize)]
        struct Ctx {
            attr: Attribute,
        }

        let mut env = Environment::new();
        let attr = Attribute {
            name: "attr1".to_owned(),
            r#type: enum_type(vec!["value1".into(), "value2".into()]),
            brief: "A brief description".to_owned(),
            examples: None,
            tag: None,
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: None,
            note: "A note".to_owned(),
            stability: None,
            deprecated: None,
            tags: None,
            value: None,
            prefix: false,
        };

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str(
                "{% if attr is enum %}true{% else %}false{% endif %}",
                Ctx { attr },
            )
            .unwrap(),
            "true"
        );

        let attr = Attribute {
            name: "attr1".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "A brief description".to_owned(),
            examples: None,
            tag: None,
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: None,
            note: "A note".to_owned(),
            stability: None,
            deprecated: None,
            tags: None,
            value: None,
            prefix: false,
        };

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str(
                "{% if attr is enum %}true{% else %}false{% endif %}",
                Ctx { attr },
            )
            .unwrap(),
            "false"
        );
    }

    /// Utility function to create an enum type from a list of member values.
    fn enum_type(member_values: Vec<ValueSpec>) -> AttributeType {
        let members = member_values
            .into_iter()
            .enumerate()
            .map(|(i, value)| EnumEntriesSpec {
                id: format!("variant{}", i),
                value,
                brief: None,
                note: None,
                stability: None,
                deprecated: None,
            })
            .collect();

        AttributeType::Enum {
            allow_custom_values: true,
            members,
        }
    }

    #[test]
    fn test_semconv_const() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str(
                "{{ 'messaging.client_id' | screaming_snake_case_const }}",
                &ctx,
            )
            .unwrap(),
            "MESSAGING_CLIENTID"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | pascal_case_const }}", &ctx,)
                .unwrap(),
            "MessagingClientid"
        );

        assert_eq!(
            env.render_str(
                "{{ 'messaging.client.id' | screaming_snake_case_const }}",
                &ctx,
            )
            .unwrap(),
            "MESSAGING_CLIENT_ID"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | pascal_case_const }}", &ctx,)
                .unwrap(),
            "MessagingClientId"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | kebab_case_const }}", &ctx,)
                .unwrap(),
            "messaging-client-id"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | kebab_case_const }}", &ctx,)
                .unwrap(),
            "messaging-clientid"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | camel_case_const }}", &ctx,)
                .unwrap(),
            "messagingClientId"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | camel_case_const }}", &ctx,)
                .unwrap(),
            "messagingClientid"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | snake_case_const }}", &ctx,)
                .unwrap(),
            "messaging_client_id"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | snake_case_const }}", &ctx,)
                .unwrap(),
            "messaging_clientid"
        );

        assert!(env
            .render_str("{{ 'messaging.client.id' | invalid_case_const }}", &ctx,)
            .is_err());

        assert!(env
            .render_str("{{ 123 | pascal_case_const }}", &ctx,)
            .is_err());
    }

    #[test]
    fn test_member_print_value() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_tests_and_filters(&mut env);

        assert_eq!(
            env.render_str("{{ 1 | print_member_value }}", &ctx)
                .unwrap(),
            "1"
        );
        assert_eq!(
            env.render_str("{{ 1.1 | print_member_value }}", &ctx)
                .unwrap(),
            "1.1"
        );
        assert_eq!(
            env.render_str("{{ true | print_member_value }}", &ctx)
                .unwrap(),
            "true"
        );
        assert_eq!(
            env.render_str("{{ false | print_member_value }}", &ctx)
                .unwrap(),
            "false"
        );
        assert_eq!(
            env.render_str("{{ '1' | print_member_value }}", &ctx)
                .unwrap(),
            "\"1\""
        );
        assert_eq!(
            env.render_str("{{ 'test' | print_member_value }}", &ctx)
                .unwrap(),
            "\"test\""
        );
        assert_eq!(
            env.render_str("{{ [1,2] | print_member_value }}", &ctx)
                .unwrap(),
            ""
        );
    }
}
