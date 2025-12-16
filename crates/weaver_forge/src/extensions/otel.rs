// SPDX-License-Identifier: Apache-2.0

//! Set of filters, tests, and functions that are specific to the OpenTelemetry project.

use crate::config::CaseConvention;
use crate::extensions::case::{
    camel_case, kebab_case, pascal_case, screaming_snake_case, snake_case,
};
use crate::extensions::prom;
use crate::extensions::prom::TranslationStrategy;
use itertools::Itertools;
use minijinja::filters::sort;
use minijinja::value::{Kwargs, ValueKind};
use minijinja::{ErrorKind, State, Value};
use serde::de::Error;
use std::borrow::Cow;

const TEMPLATE_PREFIX: &str = "template[";
const TEMPLATE_SUFFIX: &str = "]";

/// Add OpenTelemetry specific filters to the environment.
pub(crate) fn add_filters(env: &mut minijinja::Environment<'_>) {
    env.add_filter("attribute_namespace", attribute_namespace);
    env.add_filter("attribute_id", attribute_id);
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
    env.add_filter("prometheus_metric_names", prom_names);
    env.add_filter("prometheus_metric_name", prom_name);
    env.add_filter("prometheus_unit_name", prom_unit);
    env.add_filter("print_member_value", print_member_value);
    env.add_filter("body_fields", body_fields);
}

/// Add OpenTelemetry specific tests to the environment.
pub(crate) fn add_tests(env: &mut minijinja::Environment<'_>) {
    env.add_test("stable", is_stable);
    env.add_test("experimental", is_experimental);
    env.add_test("deprecated", is_deprecated);
    env.add_test("enum", is_enum);
    env.add_test("simple_type", is_simple_type);
    env.add_test("template_type", is_template_type);
    env.add_test("enum_type", is_enum_type);
    env.add_test("array", is_array);
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
            format!("This attribute registry id `{input}` is invalid"),
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
            format!("This attribute registry id `{input}` is invalid"),
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
            format!("This attribute registry id `{input}` is invalid"),
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
            format!("This metric id `{input}` is invalid"),
        ));
    }
    Ok(parts[1].to_owned())
}

/// Converts {namespace}.{attribute_id} to ({namespace}, {id}).
/// Falls back to "other" if no namespace present.
///
/// A [`minijinja::Error`] is returned if the input or any part of it is empty.
fn attribute_split(input: &str) -> Result<(&str, &str), minijinja::Error> {
    let bad = |msg| {
        Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("`{input}`: {msg}"),
        ))
    };
    if input.is_empty() {
        return bad("must not be empty");
    }
    match input.split_once(".") {
        Some((_, "")) => bad("id must not be empty"), // "namespace."
        Some(("", _)) => bad("namespace must not be empty"), // ".id"
        Some(parts) => Ok(parts),                     // "namespace.id"
        None => Ok(("other", input)),                 // "id"
    }
}

/// Converts {namespace}.{attribute_id} to {namespace}. Falls back to "other" if no namespace present.
///
/// A [`minijinja::Error`] is returned if the input or any part of it is empty.
pub(crate) fn attribute_namespace(input: &str) -> Result<String, minijinja::Error> {
    attribute_split(input).map(|(namespace, _)| namespace.to_owned())
}

/// Converts {namespace}.{attribute_id} to {attribute_id}.
///
/// A [`minijinja::Error`] is returned if the input or any part of it is empty.
pub(crate) fn attribute_id(input: &str) -> Result<String, minijinja::Error> {
    attribute_split(input).map(|(_, id)| id.to_owned())
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

fn get_name_or_key(input: &Value) -> Result<Value, minijinja::Error> {
    let mut name = input.get_attr("name")?;
    if name.is_undefined() {
        name = input.get_attr("key")?;
    }
    Ok(name)
}

fn parse_translation_strategy(s: &str) -> Result<TranslationStrategy, minijinja::Error> {
    match s {
        "NoTranslation" => Ok(TranslationStrategy::NoTranslation),
        "UnderscoreEscapingWithoutSuffixes" => Ok(TranslationStrategy::UnderscoreEscapingWithoutSuffixes),
        "NoUTF8EscapingWithSuffixes" => Ok(TranslationStrategy::NoUTF8EscapingWithSuffixes),
        "UnderscoreEscapingWithSuffixes" => Ok(TranslationStrategy::UnderscoreEscapingWithSuffixes),
        _ => Err(minijinja::Error::custom(format!(
            "Invalid translation_strategy: '{}'. Valid values are: NoTranslation, UnderscoreEscapingWithoutSuffixes, NoUTF8EscapingWithSuffixes, UnderscoreEscapingWithSuffixes",
            s
        ))),
    }
}

pub(crate) fn prom_names(input: Value, kwargs: Kwargs) -> Result<Vec<String>, minijinja::Error> {
    let translation_strategy = kwargs
        .get::<Option<&str>>("translation_strategy")?
        .map(parse_translation_strategy)
        .transpose()?;
    let expand_summary_and_histogram = kwargs
        .get::<Option<bool>>("expand_summary_and_histogram")?
        .unwrap_or(false);

    let metric_name = get_attr(&input, "metric_name")?;
    let unit = get_attr(&input, "unit")?;
    let instrument = get_attr(&input, "instrument")?;

    Ok(prom::get_names(
        &metric_name,
        &unit,
        &instrument,
        translation_strategy.as_ref(),
        expand_summary_and_histogram,
    )
    .into_iter()
    .map(|cow| cow.into_owned())
    .collect())
}

pub(crate) fn prom_name(input: Value, kwargs: Kwargs) -> Result<String, minijinja::Error> {
    let translation_strategy_str = kwargs
        .get::<Option<&str>>("translation_strategy")?
        .ok_or_else(|| {
            minijinja::Error::custom(
                "translation_strategy parameter is required for prometheus_metric_name filter",
            )
        })?;

    let translation_strategy = parse_translation_strategy(translation_strategy_str)?;

    let metric_name = get_attr(&input, "metric_name")?;
    let unit = get_attr(&input, "unit")?;
    let instrument = get_attr(&input, "instrument")?;

    Ok(prom::get_name(&metric_name, &unit, &instrument, &translation_strategy).into_owned())
}

fn get_attr<'a>(input: &'a Value, key: &str) -> Result<Cow<'a, str>, minijinja::Error> {
    let val = input.get_attr(key)?;
    let Some(s) = val.as_str() else {
        return Err(minijinja::Error::custom(format!(
            "Expected {key} to be a string"
        )));
    };
    Ok(s.to_owned().into())
}

pub(crate) fn prom_unit(input: Value) -> Result<String, minijinja::Error> {
    let unit_val = input.get_attr("unit")?;
    let Some(unit) = unit_val.as_str() else {
        return Err(minijinja::Error::custom("Expected unit to be a string"));
    };

    Ok(prom::get_prom_units(unit).unwrap_or(unit).to_owned())
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
                    "Expected requirement level, found {level}",
                ))),
            }
        }
    }
    match sort_ordinal_for_requirement(lhs)?.cmp(&sort_ordinal_for_requirement(rhs)?) {
        std::cmp::Ordering::Equal => {
            let lhs_name = get_name_or_key(lhs)?;
            let rhs_name = get_name_or_key(rhs)?;
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

/// Checks if the input value is an object with a field named "stability" that has any value
/// other than "stable". Otherwise, it returns true. This implies that undefined stability
/// is considered experimental for safety.
#[must_use]
pub(crate) fn is_experimental(input: &Value) -> bool {
    let result = input.get_attr("stability");

    if let Ok(stability) = result {
        if let Some(stability) = stability.as_str() {
            return stability != "stable";
        }
    }
    true
}

/// Returns `true` if the input object contains a populated `deprecated` field. If this field is
/// not present, the test returns `false`.
#[must_use]
pub(crate) fn is_deprecated(input: &Value) -> bool {
    let result = input.get_attr("deprecated");

    if let Ok(deprecated) = result {
        return deprecated.as_object().is_some();
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
        "Expected simple type, template type, or enum type, found {attr_type}"
    )))
}

/// Converts an enum member value into:
/// - A quoted and escaped string if the input is a string. JSON escapes are used.
/// - A non-quoted string if the input is a number or a boolean.
/// - An empty string otherwise.
pub(crate) fn print_member_value(input: &Value) -> Result<String, minijinja::Error> {
    match input.kind() {
        ValueKind::String => {
            if let Some(input) = input.as_str() {
                // Escape the string and add quotes.
                // JSON escapes are used as they are very common for most languages.
                if let Ok(input) = serde_json::to_string(input) {
                    Ok(input)
                } else {
                    Err(minijinja::Error::custom(format!(
                        "`print_member_value` failed to convert {input} to a string"
                    )))
                }
            } else {
                Ok("".to_owned())
            }
        }
        ValueKind::Number => Ok(input.to_string()),
        ValueKind::Bool => Ok(input.to_string()),
        _ => Ok("".to_owned()),
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
                        "Enum values are expected to be int, double, or string, found {value}"
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
        "Expected enum type, found {attr_type}"
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

/// Returns true if the input type is array
pub(crate) fn is_array(attr_type: &Value) -> bool {
    let Some(attr_type) = attr_type.as_str() else {
        return false;
    };
    matches!(attr_type, "string[]" | "int[]" | "double[]" | "boolean[]")
}

/// Returns a list of pairs {field, depth} from a body field in depth-first order
/// by default.
///
/// This can be used to iterate over a tree of fields composing an
/// event body.
///
/// ```jinja
/// {% for path, field, depth in body|body_fields %}
/// Do something with {{ field }} at depth {{ depth }} with path {{ path }}
/// {% endfor %}
/// ```
pub(crate) fn body_fields(
    state: &State<'_, '_>,
    body: Value,
    kwargs: Kwargs,
) -> Result<Value, minijinja::Error> {
    fn traverse_body_fields(
        state: &State<'_, '_>,
        v: Value,
        rv: &mut Vec<Value>,
        path: String,
        depth: i64,
        sort_by: &str,
    ) -> Result<(), minijinja::Error> {
        if v.is_undefined() || v.is_none() {
            return Ok(());
        }

        let fields = v
            .get_attr("fields")
            .map_err(|_| minijinja::Error::custom("Invalid body field"))?;
        let id = v
            .get_attr("id")
            .map_err(|_| minijinja::Error::custom("Invalid body field"))?;
        let path = if path.is_empty() {
            id.to_string()
        } else {
            format!("{path}.{id}")
        };

        if fields.is_undefined() {
            rv.push(Value::from(vec![Value::from(path), v, Value::from(depth)]));
        } else {
            rv.push(Value::from(vec![
                Value::from(path.clone()),
                v,
                Value::from(depth),
            ]));
            let kwargs = Kwargs::from_iter([("attribute", Value::from(sort_by))]);
            for field in sort(state, fields, kwargs)?.try_iter()? {
                traverse_body_fields(state, field, rv, path.clone(), depth + 1, sort_by)?;
            }
        }

        Ok(())
    }

    let mut rv = Vec::new();
    let sort_by = kwargs.get::<Option<&str>>("sort_by")?.unwrap_or("id");

    traverse_body_fields(state, body, &mut rv, "".to_owned(), 0, sort_by)?;

    Ok(Value::from(rv))
}

#[cfg(test)]
mod tests {
    use minijinja::value::Object;
    use minijinja::{Environment, Value};
    use serde::Serialize;
    use serde_json::json;
    use std::fmt::Debug;
    use std::sync::Arc;

    use crate::extensions::otel::{self, attribute_id};
    use crate::extensions::otel::{
        attribute_namespace, attribute_registry_file, attribute_registry_namespace,
        attribute_registry_title, attribute_sort, is_experimental, is_stable, metric_namespace,
        print_member_value,
    };

    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::any_value::{AnyValueCommonSpec, AnyValueSpec};
    use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;
    use weaver_semconv::attribute::RequirementLevel;
    use weaver_semconv::attribute::{AttributeSpec, BasicRequirementLevelSpec};
    use weaver_semconv::attribute::{AttributeType, EnumEntriesSpec, TemplateTypeSpec, ValueSpec};
    use weaver_semconv::deprecated::Deprecated;

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
    fn test_attribute_namespace() {
        // Normal case with namespace
        let input = "namespace.attribute_id";
        assert_eq!(attribute_namespace(input).unwrap(), "namespace");

        // Multiple dots - should return first part
        let input = "namespace.sub.attribute_id";
        assert_eq!(attribute_namespace(input).unwrap(), "namespace");

        // No dots - should fallback to "other"
        let input = "attribute_id";
        assert_eq!(attribute_namespace(input).unwrap(), "other");

        // Empty string - should fail
        let input = "";
        assert!(attribute_namespace(input).is_err());

        // String starting with dot - should fail
        let input = ".attribute_id";
        assert!(attribute_namespace(input).is_err());

        // String ending with dot - should fail
        let input = "namespace.";
        assert!(attribute_namespace(input).is_err());
    }

    #[test]
    fn test_attribute_id() {
        // Normal case with namespace
        let input = "namespace.attribute_id";
        assert_eq!(attribute_id(input).unwrap(), "attribute_id");

        // Multiple dots - should return first part
        let input = "namespace.sub.attribute_id";
        assert_eq!(attribute_id(input).unwrap(), "sub.attribute_id");

        // No dots - should fallback to "other"
        let input = "attribute_id";
        assert_eq!(attribute_id(input).unwrap(), "attribute_id");

        // Empty string - should fail
        let input = "";
        assert!(attribute_id(input).is_err());

        // String starting with dot - should fail
        let input = ".attribute_id";
        assert!(attribute_id(input).is_err());

        // String ending with dot - should fail
        let input = "namespace.";
        assert!(attribute_id(input).is_err());
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

        // An attribute with stability "development"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "development".to_owned(),
            deprecated: None,
        });
        assert!(is_experimental(&attr));

        // An attribute with stability "alpha"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "alpha".to_owned(),
            deprecated: None,
        });
        assert!(is_experimental(&attr));

        // An attribute with stability "beta"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "beta".to_owned(),
            deprecated: None,
        });
        assert!(is_experimental(&attr));

        // An attribute with stability "release_candidate"
        let attr = Value::from_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "release_candidate".to_owned(),
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
        assert!(is_experimental(&object));
    }

    #[test]
    fn test_is_deprecated() {
        #[derive(Serialize)]
        struct Ctx {
            attr: Attribute,
        }

        let mut env = Environment::new();
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
            deprecated: Some(Deprecated::Renamed {
                renamed_to: "new_name".to_owned(),
                note: "".to_owned(),
            }),
            tags: None,
            value: None,
            prefix: false,
            annotations: None,
            role: Default::default(),
        };

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            env.render_str(
                "{% if attr is deprecated %}true{% else %}false{% endif %}",
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
            annotations: None,
            role: Default::default(),
        };

        assert_eq!(
            env.render_str(
                "{% if attr is deprecated %}true{% else %}false{% endif %}",
                Ctx { attr },
            )
            .unwrap(),
            "false"
        );
    }

    #[test]
    fn test_deprecated() {
        #[derive(Serialize)]
        struct Ctx {
            attr: AttributeSpec,
        }

        let mut env = Environment::new();
        let attr_yaml = r#"
            id: attr
            type: string
            brief: This attribute is replaced by new_name.
            note: A note.
            requirement_level: required
            deprecated: Replaced by new_name.
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "unspecified: Replaced by new_name."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: This attribute is replaced by new_name.
            note: A note.
            requirement_level: required
            deprecated:
                reason: renamed
                renamed_to: new_name
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "renamed: Replaced by `new_name`."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: This attribute is replaced by new_name.
            note: A note.
            requirement_level: required
            deprecated:
                reason: renamed
                renamed_to: new_name
                note: This attribute is deprecated and replaced by `new_name`.
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "renamed: This attribute is deprecated and replaced by `new_name`."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: A brief.
            note: A note.
            requirement_level: required
            deprecated:
                reason: obsoleted
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "obsoleted: Obsoleted."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: A brief.
            note: A note.
            requirement_level: required
            deprecated:
                reason: obsoleted
                note: This attribute is no longer used.
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "obsoleted: This attribute is no longer used."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: A brief.
            note: A note.
            requirement_level: required
            deprecated:
                reason: uncategorized
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "uncategorized: Uncategorized."
        );

        // ---------------------------------------------------------------------
        let attr_yaml = r#"
            id: attr
            type: string
            brief: A brief.
            note: A note.
            requirement_level: required
            deprecated:
                reason: uncategorized
                note: This attribute is deprecated for complex reasons.
        "#;
        let attr: AttributeSpec = serde_yaml::from_str(attr_yaml).unwrap();
        assert_eq!(
            env.render_str(
                "{{ attr.deprecated.reason }}: {{ attr.deprecated.note }}",
                Ctx { attr },
            )
            .unwrap(),
            "uncategorized: This attribute is deprecated for complex reasons."
        );
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
        assert_eq!(result_seq.len(), 10, "Expected 10 items, found {result}");
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
    fn test_attribute_sort_v2() {
        // Attributes in no particular order.
        let attributes: Vec<serde_json::Value> = vec![
            json!({"key": "rec.a", "requirement_level": "recommended"}),
            json!({"key": "rec.b", "requirement_level": "recommended"}),
            json!({"key": "crec.a", "requirement_level": {"conditionally_required": {"text": "hi"}}}),
            json!({"key": "crec.b", "requirement_level": {"conditionally_required": {"text": "hi"}}}),
            json!({"key": "rec.c", "requirement_level": {"recommended": {"text": "hi"}}}),
            json!({"key": "rec.d", "requirement_level": {"recommended": {"text": "hi"}}}),
            json!({"key": "opt.a", "requirement_level": "opt_in"}),
            json!({"key": "opt.b", "requirement_level": "opt_in"}),
            json!({"key": "req.a", "requirement_level": "required"}),
            json!({"key": "req.b", "requirement_level": "required"}),
        ];
        let json =
            serde_json::to_value(attributes).expect("Failed to serialize attributes to json.");
        let result =
            attribute_sort(Value::from_serialize(json)).expect("Failed to sort attributes");
        let result_seq = result
            .try_iter()
            .expect("Result was not a sequence!")
            .collect::<Vec<_>>();
        // Assert that requirement level takes precedence over anything else.
        assert_eq!(result_seq.len(), 10, "Expected 10 items, found {result}");
        let keys: Vec<String> = result_seq
            .iter()
            .map(|item| item.get_attr("key").unwrap().as_str().unwrap().to_owned())
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

        for (idx, (result, expected)) in keys.iter().zip(expected_names.iter()).enumerate() {
            assert_eq!(
                result, expected,
                "Expected item @ {idx} to have name {expected}, found {keys:?}"
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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
                annotations: None,
                role: Default::default(),
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

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
            )
            .unwrap(),
            "int[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Int),
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles),
            )
            .unwrap(),
            "double[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Double),
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean),
            )
            .unwrap(),
            "boolean"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans),
            )
            .unwrap(),
            "boolean[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::Boolean),
            )
            .unwrap(),
            "boolean"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            )
            .unwrap(),
            "string[]"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                AttributeType::Template(TemplateTypeSpec::String),
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), 2.into()]),
            )
            .unwrap(),
            "int"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.1.into(), 2.1.into()]),
            )
            .unwrap(),
            "double"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec!["value1".into(), "value2".into()]),
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), 2.1.into()]),
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.into(), "two".into()]),
            )
            .unwrap(),
            "string"
        );
        assert_eq!(
            eval(
                &env,
                "{{ attr_type | instantiated_type }}",
                enum_type(vec![1.0.into(), "two".into()]),
            )
            .unwrap(),
            "string"
        );
        assert!(eval(
            &env,
            "{{ 'something else' | instantiated_type }}",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
        )
        .is_err());
    }

    #[test]
    fn test_is_simple_type() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

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

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

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
            annotations: None,
            role: Default::default(),
        };

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

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
            annotations: None,
            role: Default::default(),
        };

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            env.render_str(
                "{% if attr is enum %}true{% else %}false{% endif %}",
                Ctx { attr },
            )
            .unwrap(),
            "false"
        );
    }

    #[test]
    fn test_is_array() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        // (assert, result)
        let test_cases = [
            ("string", "false"),
            ("string[]", "true"),
            ("int", "false"),
            ("int[]", "true"),
            ("double", "false"),
            ("double[]", "true"),
            ("boolean", "false"),
            ("boolean[]", "true"),
            ("template[string]", "false"),
            ("template[string[]]", "false"),
            ("template[int]", "false"),
            ("template[int[]]", "false"),
            ("template[double]", "false"),
            ("template[double[]]", "false"),
            ("template[boolean]", "false"),
            ("template[boolean[]]", "false"),
            ("enum {id}", "false"),
        ];

        for case in test_cases {
            assert_eq!(
                env.render_str(
                    &format!(
                        "{{% if '{}' is array %}}true{{% else %}}false{{% endif %}}",
                        case.0
                    ),
                    &ctx
                )
                .unwrap(),
                case.1
            );
        }

        // invalid value should return false
        assert!(!otel::is_array(&Value::from(())));
    }

    /// Utility function to create an enum type from a list of member values.
    fn enum_type(member_values: Vec<ValueSpec>) -> AttributeType {
        let members = member_values
            .into_iter()
            .enumerate()
            .map(|(i, value)| EnumEntriesSpec {
                id: format!("variant{i}"),
                value,
                brief: None,
                note: None,
                stability: None,
                deprecated: None,
                annotations: None,
            })
            .collect();

        AttributeType::Enum { members }
    }

    #[test]
    fn test_semconv_const() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            env.render_str(
                "{{ 'messaging.client_id' | screaming_snake_case_const }}",
                &ctx,
            )
            .unwrap(),
            "MESSAGING_CLIENTID"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | pascal_case_const }}", &ctx)
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
            env.render_str("{{ 'messaging.client.id' | pascal_case_const }}", &ctx)
                .unwrap(),
            "MessagingClientId"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | kebab_case_const }}", &ctx)
                .unwrap(),
            "messaging-client-id"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | kebab_case_const }}", &ctx)
                .unwrap(),
            "messaging-clientid"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | camel_case_const }}", &ctx)
                .unwrap(),
            "messagingClientId"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | camel_case_const }}", &ctx)
                .unwrap(),
            "messagingClientid"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client.id' | snake_case_const }}", &ctx)
                .unwrap(),
            "messaging_client_id"
        );

        assert_eq!(
            env.render_str("{{ 'messaging.client_id' | snake_case_const }}", &ctx)
                .unwrap(),
            "messaging_clientid"
        );

        assert!(env
            .render_str("{{ 'messaging.client.id' | invalid_case_const }}", &ctx)
            .is_err());

        assert!(env
            .render_str("{{ 123 | pascal_case_const }}", &ctx)
            .is_err());
    }

    #[test]
    fn test_print_member_value() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

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
            env.render_str("{{ 'test\\bla' | print_member_value }}", &ctx)
                .unwrap(),
            "\"test\\bla\""
        );
        assert_eq!(
            env.render_str("{{ [1,2] | print_member_value }}", &ctx)
                .unwrap(),
            ""
        );

        assert_eq!(
            print_member_value(&Value::from(r#"This is a test
        on multiple lines with characters like ',   , \, and /"#)).unwrap(),
            "\"This is a test\\n        on multiple lines with characters like ',   , \\\\, and /\"");
    }

    #[test]
    fn test_body_fields() {
        #[derive(Serialize)]
        struct Event {
            body: Option<AnyValueSpec>,
        }

        let mut env = Environment::new();

        otel::add_filters(&mut env);
        otel::add_tests(&mut env);

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields %}{{field.id}}:{{depth}}{% endfor %}", Event { body: None })
                .unwrap(),
            ""
        );

        let body = AnyValueSpec::Undefined {
            common: AnyValueCommonSpec {
                id: "id_undefined".to_owned(),
                brief: "a brief".to_owned(),
                note: "a note".to_owned(),
                stability: None,
                examples: None,
                requirement_level: Default::default(),
            },
        };

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields %}{{field.id}}:{{depth}}{% endfor %}", Event { body: Some(body) })
                .unwrap(),
            "id_undefined:0"
        );

        let body = AnyValueSpec::String {
            common: AnyValueCommonSpec {
                id: "id_string".to_owned(),
                brief: "a brief".to_owned(),
                note: "a note".to_owned(),
                stability: None,
                examples: None,
                requirement_level: Default::default(),
            },
        };

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields %}{{field.id}}:{{depth}}{% endfor %}", Event { body: Some(body) })
                .unwrap(),
            "id_string:0"
        );

        let body = AnyValueSpec::Int {
            common: AnyValueCommonSpec {
                id: "id_int".to_owned(),
                brief: "a brief".to_owned(),
                note: "a note".to_owned(),
                stability: None,
                examples: None,
                requirement_level: Default::default(),
            },
        };

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields %}{{field.id}}:{{depth}}{% endfor %}", Event { body: Some(body) })
                .unwrap(),
            "id_int:0"
        );

        let body = AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "id_map".to_owned(),
                brief: "0".to_owned(),
                note: Default::default(),
                stability: None,
                examples: None,
                requirement_level: Default::default(),
            },
            fields: vec![
                AnyValueSpec::String {
                    common: AnyValueCommonSpec {
                        id: "id_string".to_owned(),
                        brief: "0".to_owned(),
                        note: Default::default(),
                        stability: None,
                        examples: None,
                        requirement_level: Default::default(),
                    },
                },
                AnyValueSpec::Int {
                    common: AnyValueCommonSpec {
                        id: "id_int".to_owned(),
                        brief: "1".to_owned(),
                        note: Default::default(),
                        stability: None,
                        examples: None,
                        requirement_level: Default::default(),
                    },
                },
                AnyValueSpec::Ints {
                    common: AnyValueCommonSpec {
                        id: "id_ints".to_owned(),
                        brief: "2".to_owned(),
                        note: Default::default(),
                        stability: None,
                        examples: None,
                        requirement_level: Default::default(),
                    },
                },
                AnyValueSpec::Maps {
                    common: AnyValueCommonSpec {
                        id: "id_maps".to_owned(),
                        brief: "3".to_owned(),
                        note: Default::default(),
                        stability: None,
                        examples: None,
                        requirement_level: Default::default(),
                    },
                    fields: vec![
                        AnyValueSpec::Boolean {
                            common: AnyValueCommonSpec {
                                id: "id_boolean".to_owned(),
                                brief: "0".to_owned(),
                                note: Default::default(),
                                stability: None,
                                examples: None,
                                requirement_level: Default::default(),
                            },
                        },
                        AnyValueSpec::Enum {
                            common: AnyValueCommonSpec {
                                id: "id_enum".to_owned(),
                                brief: "1".to_owned(),
                                note: Default::default(),
                                stability: None,
                                examples: None,
                                requirement_level: Default::default(),
                            },
                            members: vec![],
                        },
                    ],
                },
            ],
        };

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields %}{{path}}:{{field.type}}:{{depth}}|{% endfor %}", Event { body: Some(body.clone()) })
                .unwrap(),
            "id_map:map:0|id_map.id_int:int:1|id_map.id_ints:int[]:1|id_map.id_maps:map[]:1|id_map.id_maps.id_boolean:boolean:2|id_map.id_maps.id_enum:enum:2|id_map.id_string:string:1|"
        );

        assert_eq!(
            env.render_str("{% for path, field, depth in body|body_fields(sort_by='brief') %}{{path}}:{{field.type}}:{{depth}}|{% endfor %}", Event { body: Some(body) })
                .unwrap(),
            "id_map:map:0|id_map.id_string:string:1|id_map.id_int:int:1|id_map.id_ints:int[]:1|id_map.id_maps:map[]:1|id_map.id_maps.id_boolean:boolean:2|id_map.id_maps.id_enum:enum:2|"
        );
    }
}
