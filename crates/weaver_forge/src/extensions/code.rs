// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to facilitate the generation of code.

use std::collections::HashMap;

use minijinja::Value;

/// Converts the input into a string comment with a prefix.
#[must_use]
pub(crate) fn comment_with_prefix(input: &Value, prefix: &str) -> String {
    let mut comment = String::new();

    for line in input.to_string().lines() {
        if !comment.is_empty() {
            comment.push('\n');
        }
        comment.push_str(&format!("{}{}", prefix, line));
    }
    comment
}

/// Create a filter that uses the type mapping defined in `weaver.yaml` to replace
/// the input value (i.e. OTel type) with the target type.
///
/// # Returns
///
/// A function that takes an input value and returns a new string value with the
/// data type replaced. If the input value is not found in the type mapping or is
/// not a string, the input value is returned as is.
pub(crate) fn type_mapping(type_mapping: HashMap<String, String>) -> impl Fn(&Value) -> Value {
    move |input: &Value| -> Value {
        if let Some(input_as_str) = input.as_str() {
            if let Some(target_type) = type_mapping.get(input_as_str) {
                Value::from(target_type.as_str())
            } else {
                input.to_owned()
            }
        } else {
            input.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::extensions::code;

    use super::*;

    #[test]
    fn test_comment() {
        assert_eq!(comment_with_prefix(&Value::from("test"), "// "), "// test");
        assert_eq!(comment_with_prefix(&Value::from(12), "// "), "// 12");

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

        assert_eq!(comment_with_prefix(&Value::from(brief), "/// "), expected_brief);


    }

    #[test]
    fn test_mapping() {
        let type_mapping = vec![
            ("string".to_owned(), "String".to_owned()),
            ("int".to_owned(), "i64".to_owned()),
            ("double".to_owned(), "f64".to_owned()),
            ("boolean".to_owned(), "bool".to_owned()),
        ];

        let filter = code::type_mapping(type_mapping.into_iter().collect());

        assert_eq!(filter(&Value::from("int")), Value::from("i64"));
        assert_eq!(filter(&Value::from("double")), Value::from("f64"));
        assert_eq!(filter(&Value::from("string")), Value::from("String"));
        assert_eq!(filter(&Value::from("boolean")), Value::from("bool"));
        assert_eq!(
            filter(&Value::from("something else")),
            Value::from("something else")
        );
        assert_eq!(filter(&Value::from(12)), Value::from(12));
    }
}
