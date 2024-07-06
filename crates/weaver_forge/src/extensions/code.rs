// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to facilitate the generation of code.

use std::collections::HashMap;

use crate::config::WeaverConfig;
use minijinja::{Environment, Value};

/// Add code-oriented filters to the environment.
pub(crate) fn add_filters(env: &mut Environment<'_>, config: &WeaverConfig) {
    env.add_filter("type_mapping", type_mapping(config.type_mapping.clone().unwrap_or_default()));
    env.add_filter("map_text", map_text(config.text_maps.clone().unwrap_or_default()));
    env.add_filter("comment_with_prefix", comment_with_prefix);
    env.add_filter("markdown_to_html", markdown_to_html);
}

/// Converts the input string into a string comment with a prefix.
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

/// Converts the input markdown string into an HTML string.
pub(crate) fn markdown_to_html(input: &Value) -> String {
    let markdown = input.to_string();
    markdown::to_html(&markdown)
}

/// Create a filter that uses the `text_maps` section defined in `weaver.yaml` to replace
/// the input value with the target value.
pub(crate) fn map_text(
    text_maps: HashMap<String, HashMap<String, String>>,
) -> impl Fn(&Value, &str, Option<&str>) -> Value {
    move |input: &Value, mapping_name: &str, default_value: Option<&str>| -> Value {
        if let Some(input_as_str) = input.as_str() {
            if let Some(target_text) = text_maps
                .get(mapping_name)
                .and_then(|mapping| mapping.get(input_as_str))
            {
                return Value::from(target_text.as_str());
            }
        }

        if let Some(default) = default_value {
            Value::from(default)
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

        assert_eq!(
            comment_with_prefix(&Value::from(brief), "/// "),
            expected_brief
        );
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

    #[test]
    fn test_markdown_to_html() {
        let markdown = r#"# Title"#;
        let expected_html = "<h1>Title</h1>";
        assert_eq!(markdown_to_html(&Value::from(markdown)), expected_html);

        let markdown = r#"## Subtitle"#;
        let expected_html = "<h2>Subtitle</h2>";
        assert_eq!(markdown_to_html(&Value::from(markdown)), expected_html);

        let markdown = "A paragraph with\n\na new line.";
        let expected_html = "<p>A paragraph with</p>\n<p>a new line.</p>";
        assert_eq!(markdown_to_html(&Value::from(markdown)), expected_html);

        let markdown = r#"A [link](https://example.com)"#;
        let expected_html = "<p>A <a href=\"https://example.com\">link</a></p>";
        assert_eq!(markdown_to_html(&Value::from(markdown)), expected_html);
    }

    #[test]
    fn test_map_text() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        let rust_mapping = vec![
            ("string".to_owned(), "String".to_owned()),
            ("int".to_owned(), "i64".to_owned()),
            ("double".to_owned(), "f64".to_owned()),
            ("boolean".to_owned(), "bool".to_owned()),
        ];
        let java_mapping = vec![
            ("string".to_owned(), "String".to_owned()),
            ("int".to_owned(), "int".to_owned()),
            ("double".to_owned(), "double".to_owned()),
            ("boolean".to_owned(), "boolean".to_owned()),
        ];
        let text_maps: HashMap<String, HashMap<String, String>> = vec![
            (
                "rust".to_owned(),
                rust_mapping
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
            ),
            (
                "java".to_owned(),
                java_mapping
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
            ),
        ]
        .into_iter()
        .collect();

        env.add_filter("map_text", map_text(text_maps));

        // Test with the `rust` mapping
        assert_eq!(
            env.render_str("{{ 'int' | map_text('rust') }}", &ctx)
                .unwrap(),
            "i64"
        );
        assert_eq!(
            env.render_str("{{ 'double' | map_text('rust') }}", &ctx)
                .unwrap(),
            "f64"
        );
        assert_eq!(
            env.render_str("{{ 'string' | map_text('rust') }}", &ctx)
                .unwrap(),
            "String"
        );
        assert_eq!(
            env.render_str("{{ 'boolean' | map_text('rust') }}", &ctx)
                .unwrap(),
            "bool"
        );
        assert_eq!(
            env.render_str("{{ 'something else' | map_text('rust') }}", &ctx)
                .unwrap(),
            "something else"
        );
        assert_eq!(
            env.render_str("{{ 12 | map_text('rust') }}", &ctx).unwrap(),
            "12"
        );

        // Test with the `java` mapping
        assert_eq!(
            env.render_str("{{ 'int' | map_text('java') }}", &ctx)
                .unwrap(),
            "int"
        );
        assert_eq!(
            env.render_str("{{ 'double' | map_text('java') }}", &ctx)
                .unwrap(),
            "double"
        );
        assert_eq!(
            env.render_str("{{ 'string' | map_text('java') }}", &ctx)
                .unwrap(),
            "String"
        );
        assert_eq!(
            env.render_str("{{ 'boolean' | map_text('java') }}", &ctx)
                .unwrap(),
            "boolean"
        );
        assert_eq!(
            env.render_str("{{ 'something else' | map_text('java') }}", &ctx)
                .unwrap(),
            "something else"
        );
        assert_eq!(
            env.render_str("{{ 12 | map_text('java') }}", &ctx).unwrap(),
            "12"
        );

        // Test default value
        assert_eq!(
            env.render_str("{{ 'int' | map_text('java', 'enum') }}", &ctx)
                .unwrap(),
            "int"
        );
        assert_eq!(
            env.render_str("{{ 'int' | map_text('unknown', 'enum') }}", &ctx)
                .unwrap(),
            "enum"
        );
        assert_eq!(
            env.render_str("{{ 'something else' | map_text('java', 'enum') }}", &ctx)
                .unwrap(),
            "enum"
        );
    }
}
