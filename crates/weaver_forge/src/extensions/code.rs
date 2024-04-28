// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to facilitate the generation of code.

use std::collections::HashMap;

/// Converts the input string into a comment with a prefix.
#[must_use]
pub(crate) fn comment_with_prefix(input: &str, prefix: &str) -> String {
    let mut comment = String::new();
    for line in input.lines() {
        if !comment.is_empty() {
            comment.push('\n');
        }
        comment.push_str(&format!("{}{}", prefix, line));
    }
    comment
}

/// Create a filter that uses the type mapping defined in `weaver.yaml` to replace
/// the input string (i.e. OTel type) with the target type.
///
/// # Example
///
/// ```rust
/// use weaver_forge::extensions::code;
///
/// let type_mapping = vec![
///     ("string".to_owned(), "String".to_owned()),
///     ("int".to_owned(), "i64".to_owned()),
///     ("double".to_owned(), "f64".to_owned()),
///     ("boolean".to_owned(), "bool".to_owned()),
/// ];
///
/// let filter = code::type_mapping(type_mapping.into_iter().collect());;
///
/// assert_eq!(filter("int"), "i64");
/// assert_eq!(filter("double"), "f64");
/// assert_eq!(filter("string"), "String");
/// assert_eq!(filter("boolean"), "bool");
/// assert_eq!(filter("something else"), "something else");
/// ```
///
/// # Returns
///
/// A function that takes an input string and returns a new string with the
/// data type replaced.
pub fn type_mapping(type_mapping: HashMap<String, String>) -> impl Fn(&str) -> String {
    move |input: &str| -> String {
        if let Some(target_type) = type_mapping.get(input) {
            target_type.clone()
        } else {
            input.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment() {
        assert_eq!(comment_with_prefix("test", "// "), "// test");

        let brief = r#"These attributes may be used to describe the client in a connection-based network interaction
where there is one side that initiates the connection (the client is the side that initiates the connection).
This covers all TCP network interactions since TCP is connection-based and one side initiates the
connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the
protocol / API doesn't expose a clear notion of client and server).
This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS."#;

        let expected_brief = r#"/// These attributes may be used to describe the client in a connection-based network interaction
/// where there is one side that initiates the connection (the client is the side that initiates the connection).
/// This covers all TCP network interactions since TCP is connection-based and one side initiates the
/// connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the
/// protocol / API doesn't expose a clear notion of client and server).
/// This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS."#;

        assert_eq!(comment_with_prefix(brief, "/// "), expected_brief);
    }
}
