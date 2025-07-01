// SPDX-License-Identifier: Apache-2.0

use crate::config::default_bool;
use crate::config::{RenderFormat, WeaverConfig};
use crate::error::Error;
use crate::install_weaver_extensions;
use markdown::mdast::{Delete, Emphasis, Node, Strong};
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{WordWrapConfig, WordWrapContext};

/// Options for rendering markdown.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct MarkdownRenderOptions {
    /// Whether to escape backslashes in the Markdown.
    /// Default is false.
    #[serde(default)]
    pub(crate) escape_backslashes: bool,
    /// Whether to escape square brackets in the Markdown text. Valid links are not affected.
    /// Default is false.
    #[serde(default)]
    pub(crate) escape_square_brackets: bool,
    /// Whether to indent the first level of list items in the markdown.
    /// Default is false.
    #[serde(default)]
    pub(crate) indent_first_level_list_items: bool,
    /// A shortcut reference link consists of a link label that matches a link reference
    /// definition elsewhere in the document and is not followed by [] or a link label.
    /// Default is false.
    #[serde(default)]
    pub(crate) shortcut_reference_link: bool,
    /// The default language for code blocks.
    /// Default is None.
    pub(crate) default_block_code_language: Option<String>,
    #[serde(default = "default_bool::<false>")]
    pub(crate) use_go_style_list_indent: bool,
}

pub(crate) struct ShortcutReferenceLink {
    pub(crate) label: String,
    pub(crate) url: String,
}

pub(crate) struct MarkdownRenderer {
    options_by_format: HashMap<String, MarkdownRenderOptions>,
    word_wrap_by_format: HashMap<String, WordWrapConfig>,
}

struct RenderContext {
    // The rendered markdown.
    markdown: String,
    // List level
    list_level: usize,
    // List item number
    list_item_number: usize,
    // The shortcut reference links.
    shortcut_reference_links: Vec<ShortcutReferenceLink>,
    // The rendering process traverses the AST tree in a depth-first manner.
    // In certain circumstances, newlines should only be rendered if there is a
    // node following the current one in the AST traversal. This field contains
    // the number of such newlines left by the previous node, which must be added
    // by the current node during rendering, if it exists.
    leftover_newlines: usize,
    // A line prefix to add in front of each new line.
    line_prefix: String,
    // Whether to skip the line prefix on the first line.
    skip_line_prefix_on_first_line: bool,
    // Word wrapping helper.
    word_wrap: WordWrapContext,
    // A buffer of text we cannot break apart when dealing with links, emphasis, etc.
    unbreakable_buffer: Option<String>,
}

impl RenderContext {
    fn new(cfg: &WordWrapConfig) -> Self {
        Self {
            markdown: Default::default(),
            list_level: Default::default(),
            list_item_number: Default::default(),
            shortcut_reference_links: Default::default(),
            leftover_newlines: Default::default(),
            line_prefix: Default::default(),
            skip_line_prefix_on_first_line: Default::default(),
            word_wrap: WordWrapContext::new(cfg),
            unbreakable_buffer: Default::default(),
        }
    }

    /// Return the number of leftover newlines and reset the count.
    fn take_leftover_newlines(&mut self) -> usize {
        let leftover_newlines = self.leftover_newlines;
        self.leftover_newlines = 0;
        leftover_newlines
    }

    /// Add the number of leftover newlines.
    fn add_leftover_newlines(&mut self, count: usize) {
        self.leftover_newlines += count;
    }

    /// Add a blank line if the current markdown buffer
    /// does not end already with a double newline.
    fn add_cond_blank_line(&mut self) {
        // TODO - This is a workaround for not truly
        // refactoring word-wrap vs. regular add-text.
        if !self.markdown.ends_with("\n\n") && !self.markdown.is_empty() {
            let _ = self
                .word_wrap
                .write_ln(&mut self.markdown, &self.line_prefix);
        }
    }
    fn add_blank_line(&mut self) {
        let _ = self
            .word_wrap
            .write_ln(&mut self.markdown, &self.line_prefix);
    }

    /// Set the line prefix to add in front of each new line.
    fn set_line_prefix(&mut self, prefix: &str) {
        prefix.clone_into(&mut self.line_prefix);
    }

    /// Skip the line prefix on the first line.
    fn skip_line_prefix_on_first_line(&mut self) {
        self.skip_line_prefix_on_first_line = true;
    }

    /// Reset the line prefix.
    fn reset_line_prefix(&mut self) {
        "".clone_into(&mut self.line_prefix);
        self.skip_line_prefix_on_first_line = false;
    }

    fn start_unbreakable_block(&mut self, text: &str) {
        // TODO - check for existing unbreakable.
        if let Some(buf) = self.unbreakable_buffer.as_ref() {
            // ToDo - we should error out here.
            // For now, we just FLUSH this to write to the buffer.
            let _ = self
                .word_wrap
                .write_unbroken(&mut self.markdown, buf, &self.line_prefix);
        }
        if self.word_wrap.line_length.is_some() {
            // Start a buffer
            self.unbreakable_buffer = Some(text.to_owned());
        } else {
            self.markdown.push_str(text);
        }
    }
    fn end_unbreakable_block(&mut self, text: &str) {
        let result = if let Some(buffer) = self.unbreakable_buffer.as_ref() {
            format!("{buffer}{text}")
        } else {
            text.to_owned()
        };
        self.unbreakable_buffer = None;
        self.add_unbreakable_text(&result);
    }
    /// Add text to the markdown buffer.
    fn add_text(&mut self, text: &str) {
        if let Some(buf) = self.unbreakable_buffer.as_mut() {
            buf.push_str(text);
        } else if self.word_wrap.line_length.is_some() {
            if !self.line_prefix.is_empty() && !self.skip_line_prefix_on_first_line {
                let prefix = self.line_prefix.to_owned();
                self.add_unbreakable_text(&prefix);
            }
            // Word wrap algorithm.
            if self.word_wrap.ignore_newlines {
                let _ = self
                    .word_wrap
                    .write_words(&mut self.markdown, text, &self.line_prefix);
            } else {
                // Now we need to deal with newlines.
                let lines = text.split('\n');
                for (i, line) in lines.enumerate() {
                    if i > 0 {
                        self.add_blank_line();
                    }
                    let _ = self
                        .word_wrap
                        .write_words(&mut self.markdown, line, &self.line_prefix);
                }
            }
        } else {
            // Preserve original lines
            let lines = text.split('\n');
            for (i, line) in lines.enumerate() {
                if i > 0 {
                    self.markdown.push('\n');
                }
                if !self.line_prefix.is_empty() && (!self.skip_line_prefix_on_first_line || i > 0) {
                    self.markdown.push_str(self.line_prefix.as_str());
                }
                self.markdown.push_str(line);
            }
        }
    }
    fn add_unbreakable_text(&mut self, text: &str) {
        if let Some(buf) = self.unbreakable_buffer.as_mut() {
            buf.push_str(text);
        } else {
            let _ = self
                .word_wrap
                .write_unbroken(&mut self.markdown, text, &self.line_prefix);
        }
    }
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
                .filter_map(|(name, format)| match format.format {
                    RenderFormat::Html(..) => None,
                    RenderFormat::Markdown(markdown_options) => Some((name, markdown_options)),
                })
                .collect(),
            word_wrap_by_format: config
                .comment_formats
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(name, format)| match format.format {
                    RenderFormat::Html(..) => None,
                    RenderFormat::Markdown(_) => Some((name, format.word_wrap)),
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
    pub fn render(
        &self,
        markdown: &str,
        format: &str,
        line_length_override: Option<usize>,
    ) -> Result<String, Error> {
        let render_options = if let Some(options) = self.options_by_format.get(format) {
            options
        } else {
            return Err(Error::CommentFormatNotFound {
                format: format.to_owned(),
                formats: self.options_by_format.keys().cloned().collect(),
            });
        };
        let word_wrap_options = if let Some(options) = self.word_wrap_by_format.get(format) {
            options.with_line_length_override(line_length_override)
        } else {
            return Err(Error::CommentFormatNotFound {
                format: format.to_owned(),
                formats: self.options_by_format.keys().cloned().collect(),
            });
        };

        let md_options = markdown::ParseOptions::default();
        let md_node =
            markdown::to_mdast(markdown, &md_options).map_err(|e| Error::InvalidMarkdown {
                error: e.to_string(),
            })?;
        let mut render_context = RenderContext::new(&word_wrap_options);
        Self::write_markdown_to(&mut render_context, "", &md_node, render_options)?;

        if !render_context.shortcut_reference_links.is_empty() {
            let blank_line_count = render_context.take_leftover_newlines();
            if blank_line_count > 0 {
                render_context
                    .markdown
                    .push_str(&"\n".repeat(blank_line_count));
            }
            for link in &render_context.shortcut_reference_links {
                render_context.markdown.push('\n');
                render_context
                    .markdown
                    .push_str(&format!("[{}]: {}", link.label, link.url));
            }
        }

        Ok(render_context.markdown)
    }

    /// Render custom markdown from a markdown AST tree into a buffer.
    fn write_markdown_to(
        ctx: &mut RenderContext,
        indent: &str,
        md_node: &Node,
        options: &MarkdownRenderOptions,
    ) -> Result<(), Error> {
        let leftover_newlines = ctx.take_leftover_newlines();
        if leftover_newlines > 0 {
            // Add the newlines left by the previous node only if the current node
            // is not a list.
            if !matches!(md_node, Node::List(..)) {
                for _ in 0..leftover_newlines {
                    ctx.add_blank_line();
                }
            }
        }
        match md_node {
            Node::Root(root) => {
                for child in &root.children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
            }
            Node::Text(text) => {
                fn escape_unescaped_chars(s: &str, chars_to_escape: &[char]) -> String {
                    let mut result = String::with_capacity(s.len());
                    let mut backslash_count = 0;

                    for c in s.chars() {
                        if c == '\\' {
                            backslash_count += 1;
                            result.push(c);
                        } else {
                            if chars_to_escape.contains(&c) && backslash_count % 2 == 0 {
                                // Even number of backslashes means the character is unescaped
                                result.push('\\');
                            }
                            result.push(c);
                            // Reset the backslash count after a non-backslash character
                            backslash_count = 0;
                        }
                    }

                    result
                }

                let mut text = text.value.clone();
                if options.escape_backslashes {
                    text = text.replace('\\', "\\\\");
                }
                if options.escape_square_brackets {
                    text = escape_unescaped_chars(&text, &['[', ']']);
                }
                ctx.add_text(&text);
            }
            Node::Paragraph(p) => {
                ctx.add_cond_blank_line();
                for child in &p.children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                ctx.add_blank_line();
            }
            Node::List(list) => {
                ctx.list_level += 1;
                let indent = if !options.indent_first_level_list_items && ctx.list_level == 1 {
                    indent.to_owned()
                } else if options.use_go_style_list_indent && list.ordered {
                    format!("{indent} ")
                } else {
                    format!("{indent}  ")
                };
                ctx.add_blank_line();
                for item in &list.children {
                    let leftover_newlines = ctx.take_leftover_newlines();
                    if leftover_newlines > 0 {
                        ctx.set_line_prefix("");
                        for _ in 0..leftover_newlines {
                            ctx.add_blank_line();
                        }
                    }
                    ctx.list_item_number += 1;
                    let line_prefix = if list.ordered {
                        format!("{}{}. ", indent, ctx.list_item_number)
                    } else {
                        format!("{indent}- ")
                    };
                    ctx.skip_line_prefix_on_first_line();
                    ctx.set_line_prefix(" ".repeat(line_prefix.len()).as_str());
                    // ctx.markdown.push_str(&line_prefix);
                    ctx.add_unbreakable_text(&line_prefix);
                    Self::write_markdown_to(ctx, &indent, item, options)?;
                    ctx.add_leftover_newlines(1);
                }
                ctx.list_level -= 1;
                ctx.list_item_number = 0;
                ctx.reset_line_prefix();
                ctx.add_leftover_newlines(1);
            }
            Node::ListItem(item) => {
                for child in &item.children {
                    match child {
                        Node::Paragraph(paragraph) => {
                            for child in &paragraph.children {
                                Self::write_markdown_to(ctx, indent, child, options)?;
                            }
                        }
                        _ => {
                            Self::write_markdown_to(ctx, indent, child, options)?;
                        }
                    }
                }
            }
            Node::Html(html) => {
                ctx.add_unbreakable_text(&html.value);
            }
            Node::InlineCode(code) => {
                ctx.add_unbreakable_text(&format!("`{}`", code.value));
            }
            Node::Code(code) => {
                // If the language is not specified, use the default language and if no default
                // language is specified, use an empty string.
                let lang = code
                    .lang
                    .as_deref()
                    .or(options.default_block_code_language.as_deref())
                    .unwrap_or("");

                ctx.add_unbreakable_text(&format!("```{}\n{}\n```", lang, code.value));
                ctx.add_blank_line();
            }
            Node::Blockquote(block_quote) => {
                // Somehow we're getting  end of lines from the block quote.
                ctx.add_cond_blank_line();
                ctx.set_line_prefix("> ");
                for child in &block_quote.children {
                    match child {
                        Node::Paragraph(paragraph) => {
                            for child in &paragraph.children {
                                Self::write_markdown_to(ctx, indent, child, options)?;
                            }
                        }
                        _ => {
                            Self::write_markdown_to(ctx, indent, child, options)?;
                        }
                    }
                }
                ctx.reset_line_prefix();
                ctx.add_blank_line();
            }
            Node::Link(link) => {
                ctx.start_unbreakable_block("[");
                let start = ctx.markdown.len();
                for child in &link.children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                let label = if let Some(buf) = ctx.unbreakable_buffer.as_ref() {
                    buf[1..].to_string()
                } else {
                    ctx.markdown[start..].to_string()
                };
                ctx.add_unbreakable_text("]");
                if options.shortcut_reference_link && !link.url.is_empty() {
                    let url = link.url.clone();
                    ctx.shortcut_reference_links
                        .push(ShortcutReferenceLink { label, url });
                } else {
                    ctx.add_unbreakable_text(&format!("({})", link.url));
                }
                ctx.end_unbreakable_block("");
            }
            Node::Strong(Strong { children, .. }) => {
                ctx.start_unbreakable_block("**");
                for child in children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                ctx.end_unbreakable_block("**");
            }
            Node::Emphasis(Emphasis { children, .. }) => {
                ctx.start_unbreakable_block("*");
                for child in children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                ctx.end_unbreakable_block("*");
            }
            Node::Delete(Delete { children, .. }) => {
                ctx.start_unbreakable_block("~~");
                for child in children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                ctx.end_unbreakable_block("~~");
            }
            Node::Heading(heading) => {
                // Heading nodes must surrounded by newlines.
                ctx.add_cond_blank_line();
                ctx.start_unbreakable_block(&format!(
                    "{}{} ",
                    indent,
                    "#".repeat(heading.depth as usize),
                ));
                for child in &heading.children {
                    Self::write_markdown_to(ctx, indent, child, options)?;
                }
                ctx.end_unbreakable_block("");
                ctx.add_blank_line();
                ctx.add_leftover_newlines(1);
            }
            // Not supported markdown node types.
            Node::Toml(_) => {}
            Node::Yaml(_) => {}
            Node::Break(_) => {}
            Node::Image(_) => {}
            Node::ImageReference(_) => {}
            Node::LinkReference(_) => {}
            Node::Table(_) => {}
            Node::ThematicBreak(_) => {}
            Node::TableRow(_) => {}
            Node::TableCell(_) => {}
            Node::Definition(_) => {}
            Node::FootnoteDefinition(_) => {}
            Node::MdxJsxFlowElement(_) => {}
            Node::MdxjsEsm(_) => {}
            Node::InlineMath(_) => {}
            Node::MdxTextExpression(_) => {}
            Node::FootnoteReference(_) => {}
            Node::MdxJsxTextElement(_) => {}
            Node::Math(_) => {}
            Node::MdxFlowExpression(_) => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use weaver_diff::assert_string_eq;

    use crate::config::{CommentFormat, IndentType, RenderFormat, WeaverConfig};
    use crate::error::Error;
    use crate::formats::markdown::{MarkdownRenderOptions, MarkdownRenderer};
    use crate::formats::WordWrapConfig;

    #[test]
    fn test_markdown_renderer() -> Result<(), Error> {
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        indent_type: IndentType::Space,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: false,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
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
            default_comment_format: Some("go".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
          The file extension extracted from the `url.full`, excluding the leading dot."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a URL may refer to an IP and/or port directly,
The file extension extracted from the `url.full`, excluding the leading dot.
"## // ToDo why a new line at the end?
        );

        let markdown = r##"Follows
[OCI Image Manifest Specification](https://github.com/opencontainers/image-spec/blob/main/manifest.md),
and specifically the
[Digest property](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests).

An example can be found in
[Example Image Manifest](https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest)."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"Follows
[OCI Image Manifest Specification],
and specifically the
[Digest property].

An example can be found in
[Example Image Manifest].

[OCI Image Manifest Specification]: https://github.com/opencontainers/image-spec/blob/main/manifest.md
[Digest property]: https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests
[Example Image Manifest]: https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest"##
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
* Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not

And something more."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
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
  - Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not

And something more.
"##
        );

        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: false,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
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
            default_comment_format: Some("go".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"In some cases a [URL] may refer to an [IP](http://ip.com) and/or port directly,
          The file \\[extension\\] extracted \\[from] the `url.full`, excluding the leading dot."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a [URL] may refer to an [IP] and/or port directly,
The file \[extension\] extracted \[from] the `url.full`, excluding the leading dot.

[IP]: http://ip.com"##
        );

        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: true,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
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
            default_comment_format: Some("go".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"In some cases a [URL] may refer to an [IP](http://ip.com) and/or port directly,
          The file \[extension\] extracted \[from] the `url.full`, excluding the leading dot."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a \[URL\] may refer to an [IP] and/or port directly,
The file \[extension\] extracted \[from\] the `url.full`, excluding the leading dot.

[IP]: http://ip.com"##
        );

        Ok(())
    }
    #[test]
    fn test_markdown_renderer_wrap() -> Result<(), Error> {
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        indent_type: IndentType::Space,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: false,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: Some(30),
                            ignore_newlines: true,
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
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a URL may refer
to an IP and/or port directly,
The file extension extracted
from the `url.full`, excluding
the leading dot.
"## // ToDo why a new line at the end?
        );

        let markdown = r##"Follows
[OCI Image Manifest Specification](https://github.com/opencontainers/image-spec/blob/main/manifest.md),
and specifically the
[Digest property](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests).

An example can be found in
[Example Image Manifest](https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest)."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"Follows
[OCI Image Manifest Specification]
, and specifically the
[Digest property].

An example can be found in
[Example Image Manifest].

[OCI Image Manifest Specification]: https://github.com/opencontainers/image-spec/blob/main/manifest.md
[Digest property]: https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests
[Example Image Manifest]: https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest"##
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
* Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not

And something more."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"The `error.type` SHOULD be
predictable, and SHOULD have
low cardinality.

When `error.type` is set to a
type (e.g., an exception type),
its canonical class name
identifying the type within the
artifact SHOULD be used.

Instrumentations SHOULD
document the list of errors
they report.

The cardinality of `error.type`
 within one instrumentation
library SHOULD be low.
Telemetry consumers that
aggregate data from multiple
instrumentation libraries and
applications should be prepared
for `error.type` to have high
cardinality at query time when
no additional filters are
applied.

If the operation has completed
successfully, instrumentations
SHOULD NOT set `error.type`.

If a specific domain defines
its own set of error
identifiers (such as HTTP or
gRPC status codes), it's
RECOMMENDED to:

  - Use a domain-specific
    attribute
  - Set `error.type` to capture
    all errors, regardless of
    whether they are defined
    within the domain-specific
    set or not

And something more.
"##
        );

        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: false,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: Some(30),
                            ignore_newlines: true,
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
        let markdown = r##"In some cases a [URL] may refer to an [IP](http://ip.com) and/or port directly,
          The file \\[extension\\] extracted \\[from] the `url.full`, excluding the leading dot."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a [URL] may refer
to an [IP] and/or port
directly, The file
\[extension\] extracted \[from]
the `url.full`, excluding the
leading dot.

[IP]: http://ip.com"##
        );

        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: true,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: Some(30),
                            ignore_newlines: true,
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
        let markdown = r##"In some cases a [URL] may refer to an [IP](http://ip.com) and/or port directly,
          The file \[extension\] extracted \[from] the `url.full`, excluding the leading dot."##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"In some cases a \[URL\] may
refer to an [IP] and/or port
directly, The file
\[extension\] extracted
\[from\] the `url.full`,
excluding the leading dot.

[IP]: http://ip.com"##
        );

        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"It should handle weirdly split lists.

## Unordered

- [Link 1](https://www.link1.com)
- [Link 2](https://www.link2.com)
- A very long item in the list with lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod
  tempor incididunt ut labore et dolore magna aliqua.

## Ordered

1. Example 1
2. [Example](https://loremipsum.com) with lorem ipsum dolor sit amet, consectetur adipiscing elit
   [sed](https://loremipsum.com) do eiusmod tempor incididunt ut
   [labore](https://loremipsum.com) et dolore magna aliqua.
3. Example 3
"##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"It should handle weirdly split
lists.

## Unordered

  - [Link 1]
  - [Link 2]
  - A very long item in the
    list with lorem ipsum dolor
    sit amet, consectetur
    adipiscing elit sed do
    eiusmod tempor incididunt
    ut labore et dolore magna
    aliqua.

## Ordered

  1. Example 1
  2. [Example] with lorem ipsum
     dolor sit amet,
     consectetur adipiscing
     elit [sed] do eiusmod
     tempor incididunt ut
     [labore] et dolore magna
     aliqua.
  3. Example 3


[Link 1]: https://www.link1.com
[Link 2]: https://www.link2.com
[Example]: https://loremipsum.com
[sed]: https://loremipsum.com
[labore]: https://loremipsum.com"##
        );

        Ok(())
    }
    #[test]
    fn test_markdown_render_keep_newlines() -> Result<(), Error> {
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: true,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: false,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: Some(30),
                            ignore_newlines: false,
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
        let markdown = r##"It should handle weirdly split lists.

## Unordered

- [Link 1](https://www.link1.com)
- [Link 2](https://www.link2.com)
- A very long item in the list with lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod
  tempor incididunt ut labore et dolore magna aliqua.

## Ordered

1. Example 1
2. [Example](https://loremipsum.com) with lorem ipsum dolor sit amet, consectetur adipiscing elit
   [sed](https://loremipsum.com) do eiusmod tempor incididunt ut
   [labore](https://loremipsum.com) et dolore magna aliqua.
3. Example 3
"##;
        let rendered_md = renderer.render(markdown, "go", None)?;
        assert_string_eq!(
            &rendered_md,
            r##"It should handle weirdly split
lists.

## Unordered

  - [Link 1]
  - [Link 2]
  - A very long item in the
    list with lorem ipsum dolor
    sit amet, consectetur
    adipiscing elit sed do
    eiusmod
    tempor incididunt ut labore
    et dolore magna aliqua.

## Ordered

  1. Example 1
  2. [Example] with lorem ipsum
     dolor sit amet,
     consectetur adipiscing
     elit
     [sed] do eiusmod tempor
     incididunt ut
     [labore] et dolore magna
     aliqua.
  3. Example 3


[Link 1]: https://www.link1.com
[Link 2]: https://www.link2.com
[Example]: https://loremipsum.com
[sed]: https://loremipsum.com
[labore]: https://loremipsum.com"##
        );

        // We do not want to split on punctuations like this, e.g.
        // `.`, `:`, etc.
        let renderer = MarkdownRenderer::try_new(&config)?;
        let markdown = r##"And an **inline code snippet**: `Attr.attr`."##;
        let rendered_md = renderer.render(markdown, "go", Some(80))?;
        assert_string_eq!(
            &rendered_md,
            r##"And an **inline code snippet**: `Attr.attr`.
"##
        );

        Ok(())
    }

    #[test]
    fn test_markdown_render_go_lists() -> Result<(), Error> {
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "go".to_owned(),
                    CommentFormat {
                        header: None,
                        prefix: Some("// ".to_owned()),
                        footer: None,
                        format: RenderFormat::Markdown(MarkdownRenderOptions {
                            escape_backslashes: false,
                            escape_square_brackets: true,
                            indent_first_level_list_items: true,
                            shortcut_reference_link: true,
                            default_block_code_language: None,
                            use_go_style_list_indent: true,
                        }),
                        trim: true,
                        remove_trailing_dots: true,
                        indent_type: Default::default(),
                        enforce_trailing_dots: false,
                        word_wrap: WordWrapConfig {
                            line_length: Some(30),
                            ignore_newlines: false,
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
        let markdown = r##"It should handle weirdly split lists for go.

## Unordered

  - [Link 1](https://www.link1.com)
  - [Link 2](https://www.link2.com)
  - A very long item in the list with lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod
    tempor incididunt ut labore et dolore magna aliqua.

## Ordered

 1. Example 1
 2. [Example](https://loremipsum.com) with lorem ipsum dolor sit amet, consectetur adipiscing elit
    [sed](https://loremipsum.com) do eiusmod tempor incididunt ut
    [labore](https://loremipsum.com) et dolore magna aliqua.
 3. Example 3
"##;
        let rendered_md = renderer.render(markdown, "go", Some(80))?;
        assert_string_eq!(
            &rendered_md,
            r##"It should handle weirdly split lists for go.

## Unordered

  - [Link 1]
  - [Link 2]
  - A very long item in the list with lorem ipsum dolor sit amet, consectetur
    adipiscing elit sed do eiusmod
    tempor incididunt ut labore et dolore magna aliqua.

## Ordered

 1. Example 1
 2. [Example] with lorem ipsum dolor sit amet, consectetur adipiscing elit
    [sed] do eiusmod tempor incididunt ut
    [labore] et dolore magna aliqua.
 3. Example 3


[Link 1]: https://www.link1.com
[Link 2]: https://www.link2.com
[Example]: https://loremipsum.com
[sed]: https://loremipsum.com
[labore]: https://loremipsum.com"##
        );

        Ok(())
    }
}
