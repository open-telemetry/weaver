// SPDX-License-Identifier: Apache-2.0

//! Custom Tera filters

use std::collections::{BTreeMap, HashMap};

use tera::{try_get_value, Filter, Result, Value};
use textwrap::{wrap, Options};

use crate::config::CaseConvention;

/// Case converter filter.
pub struct CaseConverter {
    filter_name: &'static str,
    case: CaseConvention,
}

impl CaseConverter {
    /// Create a new case converter filter.
    pub fn new(case: CaseConvention, filter_name: &'static str) -> Self {
        CaseConverter { filter_name, case }
    }
}

/// Filter to convert a string to a specific case.
impl Filter for CaseConverter {
    /// Convert a string to a specific case.
    fn filter(&self, value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
        let text = try_get_value!(self.filter_name, "value", String, value);
        Ok(Value::String(self.case.convert(&text)))
    }
}

/// Filter to normalize instrument name.
pub fn instrument(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    if let Value::String(metric_type) = value {
        match metric_type.as_str() {
            "counter" | "gauge" | "histogram" => Ok(Value::String(metric_type.clone())),
            "updowncounter" => Ok(Value::String("up_down_counter".to_owned())),
            _ => Err(tera::Error::msg(format!(
                "Filter instrument: unknown metric instrument {}",
                metric_type
            ))),
        }
    } else {
        Err(tera::Error::msg(format!(
            "Filter instrument: expected a string, got {:?}",
            value
        )))
    }
}

/// Filter to deduplicate attributes from a list of values containing attributes.
/// The optional parameter `recursive` can be set to `true` to recursively search for attributes.
/// The default value is `false`.
///
/// The result is a list of unique attributes sorted by their id or an empty list if no attributes
/// are found.
pub fn unique_attributes(value: &Value, ctx: &HashMap<String, Value>) -> Result<Value> {
    let mut unique_attributes = BTreeMap::new();

    let recursive = match ctx.get("recursive") {
        Some(Value::Bool(v)) => *v,
        _ => false,
    };

    fn visit_attributes(
        value: &Value,
        unique_attributes: &mut BTreeMap<String, Value>,
        levels_to_visit: usize,
    ) {
        match value {
            Value::Array(values) => {
                if levels_to_visit == 0 {
                    return;
                }
                for value in values {
                    visit_attributes(value, unique_attributes, levels_to_visit - 1);
                }
            }
            Value::Object(obj) => {
                if levels_to_visit == 0 {
                    return;
                }
                for (field, value) in obj.iter() {
                    if field.eq("attributes") {
                        if let Value::Array(attrs) = value {
                            for attr in attrs {
                                if let Value::Object(map) = attr {
                                    let id = map.get("id");
                                    if let Some(Value::String(id)) = id {
                                        if unique_attributes.contains_key(id) {
                                            // attribute already exists
                                            continue;
                                        }
                                        _ = unique_attributes.insert(id.clone(), attr.clone());
                                    }
                                }
                            }
                        }
                    }
                    visit_attributes(value, unique_attributes, levels_to_visit - 1);
                }
            }
            _ => {}
        }
    }

    visit_attributes(
        value,
        &mut unique_attributes,
        if recursive { usize::MAX } else { 1 },
    );
    let mut attributes = vec![];
    for attribute in unique_attributes.into_values() {
        attributes.push(attribute);
    }
    Ok(Value::Array(attributes))
}

/// Filter out attributes that are not required.
pub fn required(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut required_values = vec![];
    match value {
        Value::Array(values) => {
            for value in values {
                match value {
                    Value::Object(map) => {
                        if let Some(Value::String(req_level)) = map.get("requirement_level") {
                            if req_level == "required" {
                                required_values.push(value.clone());
                            }
                        }
                    }
                    _ => required_values.push(value.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }
    Ok(Value::Array(required_values))
}

/// Filter out attributes that are required.
pub fn not_required(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut required_values = vec![];
    match value {
        Value::Array(values) => {
            for value in values {
                match value {
                    Value::Object(map) => {
                        if let Some(Value::String(req_level)) = map.get("requirement_level") {
                            if req_level != "required" {
                                required_values.push(value.clone());
                            }
                        } else {
                            required_values.push(value.clone());
                        }
                    }
                    _ => required_values.push(value.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }
    Ok(Value::Array(required_values))
}

/// Transform a value into a quoted string, a number, or a boolean depending on the value type.
pub fn value(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    Ok(match value {
        Value::Bool(v) => Value::String(v.to_string()),
        Value::Number(v) => Value::String(v.to_string()),
        Value::String(v) => Value::String(format!("\"{}\"", v)),
        _ => value.clone(),
    })
}

/// Filter out attributes without value.
pub fn with_value(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut with_values = vec![];
    match value {
        Value::Array(values) => {
            for value in values {
                match value {
                    Value::Object(map) => {
                        if map.get("value").is_some() {
                            with_values.push(value.clone());
                        }
                    }
                    _ => with_values.push(value.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }
    Ok(Value::Array(with_values))
}

/// Filter out attributes with value.
pub fn without_value(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut without_values = vec![];
    match value {
        Value::Array(values) => {
            for value in values {
                match value {
                    Value::Object(map) => {
                        if map.get("value").is_none() {
                            without_values.push(value.clone());
                        }
                    }
                    _ => without_values.push(value.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }
    Ok(Value::Array(without_values))
}

/// Retain only attributes with a valid enum type.
pub fn with_enum(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut with_enums = vec![];
    match value {
        Value::Array(attributes) => {
            for attr in attributes {
                match attr {
                    Value::Object(fields) => {
                        if let Some(Value::Object(type_value)) = fields.get("type") {
                            if type_value.get("members").is_some() {
                                with_enums.push(attr.clone());
                            }
                        }
                    }
                    _ => with_enums.push(attr.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }

    Ok(Value::Array(with_enums))
}

/// Retain only attributes without a enum type.
pub fn without_enum(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut without_enums = vec![];
    match value {
        Value::Array(attributes) => {
            for attr in attributes {
                match attr {
                    Value::Object(fields) => {
                        if let Some(Value::Object(type_value)) = fields.get("type") {
                            if type_value.get("members").is_none() {
                                without_enums.push(attr.clone());
                            }
                        } else {
                            without_enums.push(attr.clone());
                        }
                    }
                    _ => without_enums.push(attr.clone()),
                }
            }
        }
        _ => return Ok(value.clone()),
    }

    Ok(Value::Array(without_enums))
}

/// Filter to map an OTel type to a language type.
pub struct TypeMapping {
    pub type_mapping: HashMap<String, String>,
}

impl Filter for TypeMapping {
    /// Map an OTel type to a language type.
    fn filter(&self, value: &Value, ctx: &HashMap<String, Value>) -> Result<Value> {
        match value {
            Value::String(otel_type) => {
                match self.type_mapping.get(otel_type) {
                    Some(language_type) => Ok(Value::String(language_type.clone())),
                    None => Err(tera::Error::msg(format!("Filter type_mapping: could not find a conversion for {}. To resolve this, create or extend the type_mapping in the config.yaml file.", otel_type)))
                }
            }
            Value::Object(otel_enum) => {
                if !otel_enum.contains_key("members") {
                    return Err(tera::Error::msg(format!("Filter type_mapping: expected an enum with a members array, got {:?}", value)))
                }
                let enum_name = match ctx.get("enum") {
                    Some(Value::String(v)) => v.clone(),
                    Some(_) => return Err(tera::Error::msg(format!("Filter type_mapping: expected a string for the enum parameter, got {:?}", ctx.get("enum")))),
                    _ => return Err(tera::Error::msg("Filter type_mapping: expected an enum parameter".to_owned()))
                };
                Ok(Value::String(enum_name))
            }
            _ => Err(tera::Error::msg(format!("Filter type_mapping: expected a string or an object, got {:?}", value)))
        }
    }
}

/// Creates a multiline comment from a string.
/// The `value` parameter is a string.
/// The `prefix` parameter is a string.
pub fn comment(value: &Value, ctx: &HashMap<String, Value>) -> Result<Value> {
    fn wrap_comment(comment: &str, prefix: &str, lines: &mut Vec<String>) {
        wrap(comment.trim_end(), Options::new(80))
            .into_iter()
            .map(|s| format!("{}{}", prefix, s.trim_end()))
            .for_each(|s| lines.push(s));
    }

    let prefix = match ctx.get("prefix") {
        Some(Value::String(prefix)) => prefix.clone(),
        _ => "".to_owned(),
    };

    let mut lines = vec![];
    match value {
        Value::String(value) => wrap_comment(value, "", &mut lines),
        Value::Array(values) => {
            for value in values {
                match value {
                    Value::String(value) => wrap_comment(value, "", &mut lines),
                    Value::Array(values) => {
                        for value in values {
                            if let Value::String(value) = value {
                                wrap_comment(value, "- ", &mut lines)
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let mut comments = String::new();
    for (i, line) in lines.into_iter().enumerate() {
        if i > 0 {
            comments.push_str(format!("\n{}", prefix).as_ref());
        }
        comments.push_str(line.as_ref());
    }
    Ok(Value::String(comments))
}
