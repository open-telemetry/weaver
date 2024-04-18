// SPDX-License-Identifier: Apache-2.0

//! Acronym filter.

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

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
/// use weaver_forge::extensions::acronym;
///
/// let acronyms = vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()];
/// let filter = acronym::acronym(acronyms);
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
