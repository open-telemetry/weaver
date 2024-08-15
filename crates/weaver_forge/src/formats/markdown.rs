// SPDX-License-Identifier: Apache-2.0

use crate::config::{RenderFormat, WeaverConfig};
use crate::error::Error;
use crate::install_weaver_extensions;
use markdown::mdast::Node;
use markdown::Constructs;
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Options for rendering markdown.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct MarkdownRenderOptions {
    /// A shortcut reference link consists of a link label that matches a link reference
    /// definition elsewhere in the document and is not followed by [] or a link label.
    /// Default is false.
    #[serde(default)]
    pub(crate) shortcut_reference_link: bool,
}

pub(crate) struct ShortcutReferenceLink {
    pub(crate) label: String,
    pub(crate) url: String,
}

pub(crate) struct MarkdownRenderer {
    options_by_format: HashMap<String, MarkdownRenderOptions>,
}

#[derive(Default)]
struct RenderContext {
    // The rendered markdown.
    markdown: String,
    // The shortcut reference links.
    shortcut_reference_links: Vec<ShortcutReferenceLink>,
}

impl MarkdownRenderer {
    pub(crate) fn try_new(config: &WeaverConfig) -> Result<MarkdownRenderer, Error> {
        let mut env = Environment::new();
        minijinja_contrib::add_to_environment(&mut env);
        // Add minijinja py-compat support to improve compatibility with Python Jinja2
        env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);

        // Add all Weaver filters and tests, except the comment filter
        // (in code extension), to avoid infinite recursion
        install_weaver_extensions(&mut env, config, false)?;

        Ok(Self {
            options_by_format: config
                .comment_formats
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(name, format)| match format.render_options.format {
                    RenderFormat::Html(..) => None,
                    RenderFormat::Markdown(markdown_options) => Some((name, markdown_options)),
                })
                .collect(),
        })
    }

    /// Render markdown to custom markdown.
    ///
    /// # Arguments
    ///
    /// * `markdown` - The markdown text to render.
    /// * `format` - The comment format to use.
    pub fn render(&self, markdown: &str, format: &str) -> Result<String, Error> {
        let render_options = if let Some(options) = self.options_by_format.get(format) {
            options
        } else {
            return Err(Error::CommentFormatNotFound {
                format: format.to_owned(),
                formats: self.options_by_format.keys().cloned().collect(),
            });
        };

        let md_options = markdown::ParseOptions {
            constructs: Constructs {
                attention: true,
                autolink: true,
                block_quote: true,
                character_escape: true,
                character_reference: true,
                code_indented: true,
                code_fenced: true,
                code_text: true,
                definition: true,
                frontmatter: false,
                hard_break_escape: true,
                hard_break_trailing: true,
                heading_atx: true,
                heading_setext: true,
                html_flow: true,
                html_text: true,
                label_start_image: true,
                label_start_link: true,
                label_end: true,
                list_item: true,
                thematic_break: true,
                ..Constructs::default()
            },
            ..markdown::ParseOptions::default()
        };
        let md_node =
            markdown::to_mdast(markdown, &md_options).map_err(|e| Error::InvalidMarkdown {
                error: e.to_string(),
            })?;
        let mut render_context = RenderContext::default();
        self.write_markdown_to(
            &mut render_context,
            "",
            &md_node,
            format,
            render_options,
        )?;

        if !render_context.shortcut_reference_links.is_empty() {
            render_context.markdown.push_str("\n");
            for link in &render_context.shortcut_reference_links {
                render_context.markdown.push_str(&format!("[{}]: {}\n", link.label, link.url));
            }
        }

        Ok(render_context.markdown)
    }

    /// Render custom markdown from a markdown AST tree into a buffer.
    fn write_markdown_to(
        &self,
        ctx: &mut RenderContext,
        indent: &str,
        md_node: &Node,
        format: &str,
        options: &MarkdownRenderOptions,
    ) -> Result<(), Error> {
        match md_node {
            Node::Root(root) => {
                for child in &root.children {
                    self.write_markdown_to(ctx, indent, child, format, options)?;
                }
            }
            Node::Text(text) => {
                ctx.markdown.push_str(&text.value);
            }
            Node::Paragraph(p) => {
                for child in &p.children {
                    self.write_markdown_to(ctx, indent, child, format, options)?;
                }
                ctx.markdown.push('\n');
            }
            Node::List(list) => {
                let list_prefix = if list.ordered { "1. " } else { "- " };
                for item in &list.children {
                    ctx.markdown.push_str(&format!("{}{}", indent, list_prefix));
                    self.write_markdown_to(ctx, indent, item, format, options)?;
                    ctx.markdown.push('\n');
                }
            }
            Node::ListItem(item) => {
                for child in &item.children {
                    match child {
                        Node::Paragraph(paragraph) => {
                            for child in &paragraph.children {
                                self.write_markdown_to(ctx, indent, child, format, options)?;
                            }
                        }
                        _ => {
                            self.write_markdown_to(ctx, indent, child, format, options)?;
                        }
                    }
                }
            }
            Node::Html(html) => {
                ctx.markdown.push_str(&html.value);
            }
            Node::InlineCode(code) => {
                ctx.markdown.push_str(&format!("`{}`", code.value));
            }
            Node::Code(code) => {
                match &code.lang {
                    Some(lang) => {
                        ctx.markdown.push_str(&format!("```{}\n{}\n```\n", lang, code.value));
                    }
                    None => {
                        ctx.markdown.push_str(&format!("```\n{}\n```\n", code.value));
                    }
                }
            }
            Node::BlockQuote(block_quote) => {
                let indent = format!("{}> ", indent);
                for child in &block_quote.children {
                    self.write_markdown_to(ctx, &indent, child, format, options)?;
                }
            }
            Node::Toml(_) => {}
            Node::Yaml(_) => {}
            Node::Break(_) => {}
            Node::Delete(_) => {}
            Node::Emphasis(_) => {}
            Node::Image(_) => {}
            Node::ImageReference(_) => {}
            Node::Link(link) => {
                ctx.markdown.push('[');
                let start = ctx.markdown.len();
                for child in &link.children {
                    self.write_markdown_to(ctx, indent, child, format, options)?;
                }
                let label = ctx.markdown[start..].to_string();
                ctx.markdown.push(']');
                if options.shortcut_reference_link && !link.url.is_empty() {
                    let url = link.url.clone();
                    ctx.shortcut_reference_links.push(ShortcutReferenceLink { label, url });
                } else {
                    ctx.markdown.push_str(&format!("({})", link.url));
                }
            }
            Node::LinkReference(_) => {}
            Node::Strong(_) => {}
            Node::Heading(_) => {}
            Node::Table(_) => {}
            Node::ThematicBreak(_) => {}
            Node::TableRow(_) => {}
            Node::TableCell(_) => {}
            Node::Definition(_) => {}
            _ => { /* Unhandled node */ }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{CommentFormat, RenderFormat, RenderOptions, TransformOptions, WeaverConfig};
    use crate::error::Error;
    use crate::formats::markdown::{MarkdownRenderOptions, MarkdownRenderer};

    #[test]
    fn test_html_renderer() -> Result<(), Error> {
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        render_options: RenderOptions {
                            header: None,
                            prefix: Some("// ".to_owned()),
                            footer: None,
                            format: RenderFormat::Markdown(MarkdownRenderOptions {
                                shortcut_reference_link: true
                            })
                        },
                        transform_options: TransformOptions {
                            trim: true,
                            remove_trailing_dots: true,
                            strong_words: vec![],
                            strong_word_style: None,
                        },
                    },
                )]
                    .into_iter()
                    .collect(),
            ),
            default_comment_format: Some("go".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
          The file extension extracted from the `url.full`, excluding the leading dot."##;
        let html = renderer.render(markdown, "go")?;
        assert_eq!(
            html,
            r##"In some cases a URL may refer to an IP and/or port directly,
The file extension extracted from the `url.full`, excluding the leading dot.
"##     // ToDo why a new line at the end?
        );

        let markdown = r##"Follows
[OCI Image Manifest Specification](https://github.com/opencontainers/image-spec/blob/main/manifest.md),
and specifically the
[Digest property](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests).

An example can be found in
[Example Image Manifest](https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest)."##;
        let html = renderer.render(markdown, "go")?;
        assert_eq!(
            html,
            r##"Follows
[OCI Image Manifest Specification],
and specifically the
[Digest property].
An example can be found in
[Example Image Manifest].

[OCI Image Manifest Specification]: https://github.com/opencontainers/image-spec/blob/main/manifest.md
[Digest property]: https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests
[Example Image Manifest]: https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest
"## // ToDo why a new line at the end?
        );

        let markdown = r##"The `error.type` SHOULD be predictable, and SHOULD have low cardinality.

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
* Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not."##;
        let html = renderer.render(markdown, "go")?;
        assert_eq!(
            html,
            r##"The `error.type` SHOULD be predictable, and SHOULD have low cardinality.
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
- Use a domain-specific attribute
- Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not.
"## // ToDo why a new line at the end?
        );
        Ok(())
    }
}