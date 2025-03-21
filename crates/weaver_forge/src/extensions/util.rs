// SPDX-License-Identifier: Apache-2.0

//! Set of utility filters and tests used by the Weaver project.

use crate::config::WeaverConfig;
use minijinja::value::Rest;
use minijinja::{Environment, ErrorKind, Value};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Add utility filters and tests to the environment.
pub(crate) fn add_filters(env: &mut Environment<'_>, target_config: &WeaverConfig) {
    env.add_filter(
        "acronym",
        acronym(target_config.acronyms.clone().unwrap_or_default()),
    );
    env.add_filter("flatten", flatten);
    env.add_filter("split_id", split_id);
    env.add_filter("regex_replace", regex_replace);
    env.add_filter("to_yaml", to_yaml);
}

/// Add utility functions to the environment.
pub(crate) fn add_functions(env: &mut Environment<'_>) {
    env.add_function("concat_if", concat_if);
}

/// Concatenate a list of values into a single string IF all values are defined.
/// If any value is undefined, the filter will return an undefined value.
fn concat_if(args: Rest<Value>) -> Value {
    let mut result = String::new();
    for arg in args.iter() {
        if arg.is_undefined() {
            return Value::default();
        }
        result.push_str(arg.to_string().as_str());
    }
    Value::from(result)
}

// Helper filter to work around lack of `list.append()` support in minijinja.
// Will take a list of lists and return a new list containing only elements of sublists.
fn flatten(value: Value) -> Result<Value, minijinja::Error> {
    let mut result = Vec::new();
    for sublist in value.try_iter()? {
        for item in sublist.try_iter()? {
            result.push(item);
        }
    }
    Ok(Value::from(result))
}

// Helper function to take an "id" and split it by '.' into namespaces.
fn split_id(value: Value) -> Result<Vec<Value>, minijinja::Error> {
    match value.as_str() {
        Some(id) => {
            let values: Vec<Value> = id
                .split('.')
                .map(|s| Value::from_safe_string(s.to_owned()))
                .collect();
            Ok(values)
        }
        None => Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Expected string, found: {value}"),
        )),
    }
}

/// Replace all occurrences of a regex pattern (1st parameter) in the input string with the
/// replacement string (2nd parameter).
fn regex_replace(
    input: Cow<'_, str>,
    pattern: Cow<'_, str>,
    replacement: Cow<'_, str>,
) -> Result<String, minijinja::Error> {
    let re = Regex::new(pattern.as_ref()).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Invalid regex pattern: {}", e),
        )
    })?;
    Ok(re
        .replace_all(input.as_ref(), replacement.as_ref())
        .to_string())
}

/// Create a filter that replaces acronyms in the input string with the full
/// name defined in the `acronyms` list.
///
/// Note: Whitespace and punctuation are preserved.
///
/// # Arguments
///
/// * `acronyms` - A list of acronyms to replace in the input string.
///
/// # Example
///
/// ```rust
/// use weaver_forge::extensions::util;
///
/// let acronyms = vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()];
/// let filter = util::acronym(acronyms);
///
/// assert_eq!(filter("This is an - IOS - device!"), "This is an - iOS - device!");
/// assert_eq!(filter("This is another type of api with the following url!   "), "This is another type of API with the following URL!   ");
/// ```
///
/// # Returns
///
/// A function that takes an input string and returns a new string with the
/// acronyms replaced.
pub fn acronym(acronyms: Vec<String>) -> impl Fn(&str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let acronym_map = acronyms
        .iter()
        .map(|acronym| (acronym.to_lowercase(), acronym.clone()))
        .collect::<HashMap<String, String>>();

    move |input: &str| -> String {
        // Pattern to match sequences of whitespace (\s+), non-whitespace
        // non-punctuation (\w+), or any punctuation ([^\w\s]+)
        let re = RE.get_or_init(|| Regex::new(r"(\s+|\w+|[^\w\s]+)").expect("Invalid regex"));
        re.find_iter(input)
            .map(|mat| match acronym_map.get(&mat.as_str().to_lowercase()) {
                Some(acronym) => acronym.clone(),
                None => mat.as_str().to_owned(),
            })
            .collect()
    }
}

fn to_yaml(value: Value) -> Result<String, minijinja::Error> {
    let value = serde_json::to_value(value)
        .map_err(|error| minijinja::Error::new(ErrorKind::BadSerialization, error.to_string()))?;

    let yaml = serde_yaml::to_string(&value)
        .map_err(|error| minijinja::Error::new(ErrorKind::BadSerialization, error.to_string()))?;

    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use crate::extensions::util::add_filters;
    use minijinja::Environment;

    #[test]
    fn test_regex_replace() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;
        let config = crate::config::WeaverConfig::default();

        add_filters(&mut env, &config);

        assert_eq!(
            env.render_str("{{ 'Hello World!' | regex_replace('!','?') }}", &ctx)
                .unwrap(),
            "Hello World?"
        );

        assert_eq!(
            env.render_str(
                "{{ \"This a test with multiple a's\" | regex_replace('a','A') }}",
                &ctx
            )
            .unwrap(),
            "This A test with multiple A's"
        );
    }

    #[test]
    fn test_to_yaml() {
        let mut env = Environment::new();
        let ctx = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30,
                "is_active": true,
                "skills": ["Rust", "JavaScript"],
                "details": {
                    "city": "Wonderland",
                    "email": "alice@example.com"
                }
            }
        });
        let config = crate::config::WeaverConfig::default();

        add_filters(&mut env, &config);

        let expected_yaml = "\
age: 30
details:
  city: Wonderland
  email: alice@example.com
is_active: true
name: Alice
skills:
- Rust
- JavaScript
";

        assert_eq!(
            env.render_str("{{ user | to_yaml }}", &ctx).unwrap(),
            expected_yaml
        );
    }
}
