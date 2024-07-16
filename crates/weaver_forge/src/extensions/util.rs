// SPDX-License-Identifier: Apache-2.0

//! Set of utility filters and tests used by the Weaver project.

use crate::config::WeaverConfig;
use minijinja::{Environment, ErrorKind, Value};
use regex::Regex;
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
