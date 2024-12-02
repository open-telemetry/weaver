// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to facilitate the generation of code.

use crate::config::{RenderFormat, WeaverConfig};
use crate::error::Error;
use crate::formats::html::HtmlRenderer;
use crate::formats::markdown::MarkdownRenderer;
use minijinja::value::{Kwargs, ValueKind};
use minijinja::{Environment, ErrorKind, Value};
use std::collections::HashMap;

/// Add code-oriented filters to the environment.
///
/// Arguments:
/// * `env` - The environment to add the filters to.
/// * `config` - The configuration to use for the filters.
/// * `comment_flag` - Whether to add comment filters.
pub(crate) fn add_filters(
    env: &mut Environment<'_>,
    config: &WeaverConfig,
    comment_flag: bool,
) -> Result<(), Error> {
    env.add_filter(
        "type_mapping",
        type_mapping(config.type_mapping.clone().unwrap_or_default()),
    );
    env.add_filter(
        "map_text",
        map_text(config.text_maps.clone().unwrap_or_default()),
    );
    if comment_flag {
        env.add_filter("comment", comment(config)?);
    }
    // This filter is deprecated
    env.add_filter("comment_with_prefix", comment_with_prefix);
    env.add_filter("markdown_to_html", markdown_to_html);
    Ok(())
}

/// Converts the input string into a string comment with a prefix.
/// Note: This filter is deprecated, please use the `comment` filter instead.
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

/// Generic comment filter reading its configuration from the `weaver.yaml` file.
#[allow(clippy::assigning_clones)]
pub(crate) fn comment(
    config: &WeaverConfig,
) -> Result<impl Fn(&Value, Kwargs) -> Result<String, minijinja::Error>, Error> {
    let default_comment_format = config
        .default_comment_format
        .clone()
        .unwrap_or("default".to_owned());
    let markdown_snippet_renderer = MarkdownRenderer::try_new(config)?;
    let html_snippet_renderer = HtmlRenderer::try_new(config)?;
    let config = config.clone();

    Ok(
        move |input: &Value, args: Kwargs| -> Result<String, minijinja::Error> {
            // The `comment` filter doesn't fail on undefined inputs.
            // This eliminates the need to check for undefined values in the templates.
            if input.kind() == ValueKind::Undefined {
                return Ok("".to_owned());
            }

            // Get the comment format from the arguments or use the default format
            // defined in the configuration.
            let comment_format_name = args
                .get("format")
                .map(|v: String| v)
                .unwrap_or(default_comment_format.clone());
            let comment_format = config
                .comment_formats
                .as_ref()
                .and_then(|comment_formats| comment_formats.get(&comment_format_name).cloned())
                .unwrap_or_default();
            // Grab line length limit, custom option.
            let line_length_limit: Option<usize> = args
                .get("line_length")
                .map(|v: u32| v as usize)
                .ok()
                .or(comment_format.word_wrap.line_length);

            // If the input is an iterable (i.e. an array), join the values with a newline.
            let mut comment = if input.kind() == ValueKind::Seq {
                if let Ok(input_values) = input.try_iter() {
                    input_values
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    input.to_string()
                }
            } else {
                input.to_string()
            };
            if comment_format.trim {
                comment = comment.trim().to_owned();
            }

            // Process `remove_trailing_dots` and `enforce_trailing_dots`
            if comment_format.remove_trailing_dots && comment_format.enforce_trailing_dots {
                return Err(minijinja::Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "'remove_trailing_dots' and 'enforce_trailing_dots' can't be both set to true at the same time for format '{}'",
                        comment_format_name
                    ),
                ));
            }
            if comment_format.remove_trailing_dots {
                comment = comment.trim_end_matches('.').to_owned();
            }
            if comment_format.enforce_trailing_dots
                && !comment.is_empty()
                && !comment.ends_with('.')
            {
                comment.push('.');
            }
            let indent_arg = args.get("indent").map(|v: usize| v).unwrap_or(0);
            let indent_type_arg = args
                .get("indent_type")
                .map(|v: String| v)
                .unwrap_or(comment_format.indent_type.to_string());
            let indent = match indent_type_arg.as_str() {
                "space" => " ".repeat(indent_arg),
                "tab" => "\t".repeat(indent_arg),
                _ => {
                    return Err(minijinja::Error::new(
                        ErrorKind::InvalidOperation,
                        "Invalid indent type, must be 'space' or 'tab'",
                    ))
                }
            };
            let header = args
                .get("header")
                .map(|v: String| v)
                .unwrap_or_else(|_| comment_format.header.clone().unwrap_or("".to_owned()));
            let prefix = args
                .get("prefix")
                .map(|v: String| v)
                .unwrap_or_else(|_| comment_format.prefix.clone().unwrap_or("".to_owned()));
            let footer = args
                .get("footer")
                .map(|v: String| v)
                .unwrap_or_else(|_| comment_format.footer.clone().unwrap_or("".to_owned()));

            // TODO - reduce line limit by tabs.
            let actual_length_limit =
                line_length_limit.map(|limit| limit - (indent.len() + prefix.len()));
            comment = match &comment_format.format {
                RenderFormat::Markdown(..) => markdown_snippet_renderer
                    .render(&comment, &comment_format_name, actual_length_limit)
                    .map_err(|e| {
                        minijinja::Error::new(
                            ErrorKind::InvalidOperation,
                            format!(
                                "Comment Markdown rendering failed for format '{}': {}",
                                default_comment_format, e
                            ),
                        )
                    })?,
                RenderFormat::Html(..) => html_snippet_renderer
                    .render(&comment, &comment_format_name, actual_length_limit)
                    .map_err(|e| {
                        minijinja::Error::new(
                            ErrorKind::InvalidOperation,
                            format!(
                                "Comment HTML rendering failed for format '{}': {}",
                                default_comment_format, e
                            ),
                        )
                    })?,
            };

            // Expand all text with prefix.
            let mut new_comment = String::new();
            for line in comment.lines() {
                if !new_comment.is_empty() {
                    new_comment.push('\n');
                }
                // We apply "trim" to all split lines.
                if header.is_empty() && new_comment.is_empty() {
                    // For the first line we don't add the indentation
                    if comment_format.trim {
                        new_comment.push_str(format!("{}{}", prefix, line).trim_end());
                    } else {
                        new_comment.push_str(&format!("{}{}", prefix, line));
                    }
                } else if comment_format.trim {
                    new_comment.push_str(format!("{}{}{}", indent, prefix, line).trim_end());
                } else {
                    new_comment.push_str(&format!("{}{}{}", indent, prefix, line));
                }
            }
            comment = new_comment;

            // Add header + footer to the comment.
            if !header.is_empty() {
                comment = format!("{}\n{}", header, comment);
            }
            if !footer.is_empty() {
                comment = format!("{}\n{}{}", comment.trim_end(), indent, footer);
            }

            // Remove all trailing spaces from the comment
            comment = comment.trim_end().to_owned();

            Ok(comment)
        },
    )
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
    use weaver_diff::assert_string_eq;

    use super::*;
    use crate::config::{CommentFormat, IndentType};
    use crate::extensions::code;
    use crate::formats::html::HtmlRenderOptions;
    use crate::formats::WordWrapConfig;

    #[test]
    fn test_comment() -> Result<(), Error> {
        let mut env = Environment::new();
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "java".to_owned(),
                    CommentFormat {
                        header: Some("/**".to_owned()),
                        prefix: Some(" * ".to_owned()),
                        footer: Some(" */".to_owned()),
                        indent_type: IndentType::Space,
                        format: RenderFormat::Html(HtmlRenderOptions {
                            old_style_paragraph: true,
                            omit_closing_li: true,
                            inline_code_snippet: "{@code {{code}}}".to_owned(),
                            block_code_snippet: "<pre>{@code {{code}}}</pre>".to_owned(),
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: None,
                            ignore_newlines: true,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..Default::default()
        };
        let note = r#" The `error.type` SHOULD be predictable, and SHOULD have low cardinality.

When `error.type` is set to a type (e.g., an exception type), its
canonical class name identifying the type within the artifact SHOULD be used.

Instrumentations SHOULD document the list of errors they report.

The cardinality of `error.type` within one instrumentation library SHOULD be low.
Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
should be prepared for `error.type` to have high cardinality at query time when no
additional filters are applied.

If the operation has completed successfully, instrumentations SHOULD NOT set `error.type`.

If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
it's RECOMMENDED to:

* Use a domain-specific attribute
* Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not..  "#;
        let ctx = serde_json::json!({
            "note": note
        });

        add_filters(&mut env, &config, true)?;

        // Test with the optional parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(format='java', indent=2) }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
   * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
   * <p>
   * When {@code error.type} is set to a type (e.g., an exception type), its
   * canonical class name identifying the type within the artifact SHOULD be used.
   * <p>
   * Instrumentations SHOULD document the list of errors they report.
   * <p>
   * The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
   * Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
   * should be prepared for {@code error.type} to have high cardinality at query time when no
   * additional filters are applied.
   * <p>
   * If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
   * <p>
   * If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
   * it's RECOMMENDED to:
   * <ul>
   *   <li>Use a domain-specific attribute
   *   <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not
   * </ul>
   */"##
        );

        // Test without the parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(indent=2) }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
   * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
   * <p>
   * When {@code error.type} is set to a type (e.g., an exception type), its
   * canonical class name identifying the type within the artifact SHOULD be used.
   * <p>
   * Instrumentations SHOULD document the list of errors they report.
   * <p>
   * The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
   * Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
   * should be prepared for {@code error.type} to have high cardinality at query time when no
   * additional filters are applied.
   * <p>
   * If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
   * <p>
   * If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
   * it's RECOMMENDED to:
   * <ul>
   *   <li>Use a domain-specific attribute
   *   <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not
   * </ul>
   */"##
        );

        // Test with indent_type=`space`
        let observed_comment = env
            .render_str("{{ note | comment(indent=2, indent_type='space') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
   * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
   * <p>
   * When {@code error.type} is set to a type (e.g., an exception type), its
   * canonical class name identifying the type within the artifact SHOULD be used.
   * <p>
   * Instrumentations SHOULD document the list of errors they report.
   * <p>
   * The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
   * Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
   * should be prepared for {@code error.type} to have high cardinality at query time when no
   * additional filters are applied.
   * <p>
   * If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
   * <p>
   * If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
   * it's RECOMMENDED to:
   * <ul>
   *   <li>Use a domain-specific attribute
   *   <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not
   * </ul>
   */"##
        );

        // Test with indent_type=`tab`
        let observed_comment = env
            .render_str("{{ note | comment(indent=2, indent_type='tab') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
		 * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
		 * <p>
		 * When {@code error.type} is set to a type (e.g., an exception type), its
		 * canonical class name identifying the type within the artifact SHOULD be used.
		 * <p>
		 * Instrumentations SHOULD document the list of errors they report.
		 * <p>
		 * The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
		 * Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
		 * should be prepared for {@code error.type} to have high cardinality at query time when no
		 * additional filters are applied.
		 * <p>
		 * If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
		 * <p>
		 * If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
		 * it's RECOMMENDED to:
		 * <ul>
		 *   <li>Use a domain-specific attribute
		 *   <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not
		 * </ul>
		 */"##
        );

        // Test with the optional parameter `line_length=30`
        // TODO - Figure out where extra space is coming from li/code near bottom.
        let observed_comment = env
            .render_str("{{ note | comment(line_length=30) }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
 * The {@code error.type}
 * SHOULD be predictable, and
 * SHOULD have low cardinality.
 * <p>
 * When {@code error.type} is
 * set to a type (e.g., an
 * exception type), its
 * canonical class name
 * identifying the type within
 * the artifact SHOULD be used.
 * <p>
 * Instrumentations SHOULD
 * document the list of errors
 * they report.
 * <p>
 * The cardinality of
 * {@code error.type} within
 * one instrumentation library
 * SHOULD be low. Telemetry
 * consumers that aggregate
 * data from multiple
 * instrumentation libraries
 * and applications should be
 * prepared for
 * {@code error.type} to have
 * high cardinality at query
 * time when no additional
 * filters are applied.
 * <p>
 * If the operation has
 * completed successfully,
 * instrumentations SHOULD NOT
 * set {@code error.type}.
 * <p>
 * If a specific domain defines
 * its own set of error
 * identifiers (such as HTTP or
 * gRPC status codes), it's
 * RECOMMENDED to:
 * <ul>
 *   <li>Use a domain-specific
 *   attribute
 *   <li>Set {@code error.type}
 *    to capture all errors,
 *   regardless of whether they
 *   are defined within the
 *   domain-specific set or not
 * </ul>
 */"##
        );

        // New configuration with `indent_type='tab'`
        let mut env = Environment::new();
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "java".to_owned(),
                    CommentFormat {
                        header: Some("/**".to_owned()),
                        prefix: Some(" * ".to_owned()),
                        footer: Some(" */".to_owned()),
                        indent_type: IndentType::Tab,
                        format: RenderFormat::Html(HtmlRenderOptions {
                            old_style_paragraph: true,
                            omit_closing_li: true,
                            inline_code_snippet: "{@code {{code}}}".to_owned(),
                            block_code_snippet: "<pre>{@code {{code}}}</pre>".to_owned(),
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: None,
                            ignore_newlines: false,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..Default::default()
        };
        add_filters(&mut env, &config, true)?;

        // Test with an undefined field in the context
        let ctx = serde_json::json!({});
        let observed_comment = env
            .render_str("{{ note | comment(format='java', indent=2) }}", ctx)
            .unwrap();
        assert_eq!(observed_comment, "");

        // Test a multi-input comment
        let ctx = serde_json::json!({
            "brief": "This is a brief description.",
            "note": "This is a note."
        });
        let observed_comment = env
            .render_str(
                "{{ [brief,'Note: ', note, something_not_in_ctx] | comment(indent=2) }}",
                &ctx,
            )
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
		 * This is a brief description.
		 * Note:
		 * This is a note
		 */"##
        );

        Ok(())
    }

    #[test]
    fn test_comment_remove_trailing_dots() -> Result<(), Error> {
        let mut env = Environment::new();
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "java".to_owned(),
                    CommentFormat {
                        header: Some("/**".to_owned()),
                        prefix: Some(" * ".to_owned()),
                        footer: Some(" */".to_owned()),
                        format: RenderFormat::Html(HtmlRenderOptions {
                            old_style_paragraph: true,
                            omit_closing_li: true,
                            inline_code_snippet: "{@code {{code}}}".to_owned(),
                            block_code_snippet: "<pre>{@code {{code}}}</pre>".to_owned(),
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        enforce_trailing_dots: false,
                        indent_type: Default::default(),
                        word_wrap: WordWrapConfig {
                            line_length: None,
                            ignore_newlines: false,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..Default::default()
        };
        add_filters(&mut env, &config, true)?;

        let note = "The `error.type` SHOULD be predictable, and SHOULD have low cardinality.";
        let ctx = serde_json::json!({
            "note": note
        });

        // Test with the optional parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(format='java') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
 * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality
 */"##
        );

        let note = "The `error.type` SHOULD be predictable, and SHOULD have low cardinality";
        let ctx = serde_json::json!({
            "note": note
        });

        // Test with the optional parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(format='java') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
 * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality
 */"##
        );

        Ok(())
    }

    #[test]
    fn test_comment_enforce_trailing_dots() -> Result<(), Error> {
        let mut env = Environment::new();
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "java".to_owned(),
                    CommentFormat {
                        header: Some("/**".to_owned()),
                        prefix: Some(" * ".to_owned()),
                        footer: Some(" */".to_owned()),
                        format: RenderFormat::Html(HtmlRenderOptions {
                            old_style_paragraph: true,
                            omit_closing_li: true,
                            inline_code_snippet: "{@code {{code}}}".to_owned(),
                            block_code_snippet: "<pre>{@code {{code}}}</pre>".to_owned(),
                        }),
                        trim: true,
                        remove_trailing_dots: false,
                        enforce_trailing_dots: true,
                        indent_type: Default::default(),
                        word_wrap: WordWrapConfig {
                            line_length: None,
                            ignore_newlines: false,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..Default::default()
        };
        add_filters(&mut env, &config, true)?;

        let note = "The `error.type` SHOULD be predictable, and SHOULD have low cardinality.";
        let ctx = serde_json::json!({
            "note": note
        });

        // Test with the optional parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(format='java') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
 * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
 */"##
        );

        let note = "The `error.type` SHOULD be predictable, and SHOULD have low cardinality";
        let ctx = serde_json::json!({
            "note": note
        });

        // Test with the optional parameter `format='java'`
        let observed_comment = env
            .render_str("{{ note | comment(format='java') }}", &ctx)
            .unwrap();
        assert_string_eq!(
            &observed_comment,
            r##"/**
 * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
 */"##
        );

        Ok(())
    }

    #[test]
    fn test_comment_with_prefix() {
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

        assert_string_eq!(
            &comment_with_prefix(&Value::from(brief), "/// "),
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
