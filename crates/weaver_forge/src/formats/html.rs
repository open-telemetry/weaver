use super::{WordWrapConfig, WordWrapContext};
use crate::config::{RenderFormat, WeaverConfig};
use crate::error::Error;
use crate::error::Error::InvalidCodeSnippet;
use crate::install_weaver_extensions;
use markdown::mdast::{Delete, Emphasis, Node, Strong};
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const INLINE_CODE_SNIPPET_MODE: &str = "inline_code";
const BLOCK_CODE_SNIPPET_MODE: &str = "block_code";

/// Options for rendering markdown to HTML.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HtmlRenderOptions {
    /// Use old-style HTML paragraphs (i.e. single \<p\> tag).
    /// Default is false.
    #[serde(default)]
    pub(crate) old_style_paragraph: bool,
    /// Omit closing \</li\> tags in lists.
    /// Default is false.
    #[serde(default)]
    pub(crate) omit_closing_li: bool,
    /// Jinja expression to render inline code. Default is "<c>{{code}}</c>".
    #[serde(default = "default_inline_code_snippet")]
    pub(crate) inline_code_snippet: String,
    /// Jinja expression to render block code. Default is "<code>{{code}}</code>".
    #[serde(default = "default_block_code_snippet")]
    pub(crate) block_code_snippet: String,
}

fn default_inline_code_snippet() -> String {
    "<c>{{code}}</c>".to_owned()
}

fn default_block_code_snippet() -> String {
    "<pre>\n{{code}}\n</pre>".to_owned()
}

impl Default for HtmlRenderOptions {
    fn default() -> Self {
        HtmlRenderOptions {
            old_style_paragraph: false,
            omit_closing_li: false,
            inline_code_snippet: default_inline_code_snippet(),
            block_code_snippet: default_block_code_snippet(),
        }
    }
}

#[derive(Serialize)]
struct CodeContext {
    code: String,
}

pub(crate) struct HtmlRenderer<'source> {
    options_by_format: HashMap<String, HtmlRenderOptions>,
    word_wrap_by_format: HashMap<String, WordWrapConfig>,
    env: Environment<'source>,
}

struct RenderContext {
    // The rendered HTML.
    html: String,

    // The rendering process traverses the AST tree in a depth-first manner.
    // In certain circumstances, a tag should only be rendered if there is a
    // node following the current one in the AST traversal. This field contains
    // such a tag left by the previous node, which must be added by the current
    // node during rendering, if it exists.
    add_old_style_paragraph: bool,

    // Context for wrapping words.
    word_wrap: WordWrapContext,
}

impl RenderContext {
    fn new(cfg: &WordWrapConfig) -> Self {
        Self {
            html: Default::default(),
            add_old_style_paragraph: Default::default(),
            word_wrap: WordWrapContext::new(cfg),
        }
    }
    // Pushes a string without splitting it into words.
    // This will wrap lines if the string is too long for the current line.
    fn push_unbroken(&mut self, input: &str, indent: &str) -> std::fmt::Result {
        self.word_wrap.write_unbroken(&mut self.html, input, indent)
    }

    fn push_unbroken_ln(&mut self, input: &str, indent: &str) -> std::fmt::Result {
        self.word_wrap
            .write_unbroken_ln(&mut self.html, input, indent)
    }

    fn pushln(&mut self, indent: &str) -> std::fmt::Result {
        self.word_wrap.write_ln(&mut self.html, indent)
    }

    // Pushes a string after splitting it into words.
    // This may alter end-of-line splits.
    fn push_words(&mut self, input: &str, indent: &str) -> std::fmt::Result {
        self.word_wrap.write_words(&mut self.html, input, indent)
    }
}

impl<'source> HtmlRenderer<'source> {
    pub(crate) fn try_new(config: &WeaverConfig) -> Result<HtmlRenderer<'source>, Error> {
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
                    RenderFormat::Html(html_options) => Some((name, html_options)),
                    RenderFormat::Markdown(..) => None,
                })
                .collect(),
            word_wrap_by_format: config
                .comment_formats
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(name, format)| match format.format {
                    RenderFormat::Html(_) => Some((name, format.word_wrap)),
                    RenderFormat::Markdown(..) => None,
                })
                .collect(),
            env,
        })
    }

    /// Render markdown to HTML.
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
        let html_render_options = if let Some(options) = self.options_by_format.get(format) {
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
        self.write_html_to(
            &mut render_context,
            "",
            &md_node,
            format,
            html_render_options,
        )?;
        Ok(render_context.html)
    }

    /// Render inline code to HTML.
    ///
    /// # Arguments
    /// * `code` - The code to render.
    /// * `format` - The comment format to use.
    /// * `options` - The HTML render options.
    pub fn render_inline_code(
        &self,
        code: &str,
        format: &str,
        options: &HtmlRenderOptions,
    ) -> Result<String, Error> {
        let ctx = CodeContext {
            code: code.to_owned(),
        };
        self.env
            .render_str(&options.inline_code_snippet, ctx)
            .map_err(|e| InvalidCodeSnippet {
                format: format.to_owned(),
                mode: INLINE_CODE_SNIPPET_MODE.to_owned(),
                error: e.to_string(),
            })
    }

    /// Render block code to HTML.
    ///
    /// # Arguments
    /// * `code` - The code to render.
    /// * `format` - The comment format to use.
    /// * `options` - The HTML render options.
    pub fn render_block_code(
        &self,
        code: &str,
        format: &str,
        options: &HtmlRenderOptions,
    ) -> Result<String, Error> {
        let ctx = CodeContext {
            code: code.to_owned(),
        };
        self.env
            .render_str(&options.block_code_snippet, ctx)
            .map_err(|e| InvalidCodeSnippet {
                format: format.to_owned(),
                mode: BLOCK_CODE_SNIPPET_MODE.to_owned(),
                error: e.to_string(),
            })
    }

    /// Render HTML from a markdown AST tree into a buffer.
    fn write_html_to(
        &self,
        ctx: &mut RenderContext,
        indent: &str,
        md_node: &Node,
        format: &str,
        options: &HtmlRenderOptions,
    ) -> Result<(), Error> {
        if ctx.add_old_style_paragraph {
            ctx.pushln(indent)?;
            if !matches!(md_node, Node::List(_)) {
                ctx.push_unbroken_ln("<p>", indent)?;
            }
            ctx.add_old_style_paragraph = false;
        }

        match md_node {
            Node::Root(root) => {
                for child in &root.children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
            }
            Node::Text(text) => {
                ctx.push_words(&text.value, indent)?;
            }
            Node::Paragraph(p) => {
                if !options.old_style_paragraph {
                    ctx.push_unbroken("<p>", indent)?;
                }
                for child in &p.children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                if options.old_style_paragraph {
                    ctx.add_old_style_paragraph = true;
                } else {
                    ctx.push_unbroken_ln("</p>", indent)?;
                }
            }
            Node::List(list) => {
                let tag = if list.ordered { "ol" } else { "ul" };
                ctx.push_unbroken(&format!("<{}>", tag), indent)?;
                let li_indent = format!("{}  ", indent);
                for item in &list.children {
                    ctx.pushln(&li_indent)?;
                    ctx.push_unbroken("<li>", &li_indent)?;
                    self.write_html_to(ctx, &li_indent, item, format, options)?;
                    if !options.omit_closing_li {
                        ctx.push_unbroken("</li>", indent)?;
                    }
                }
                ctx.pushln(indent)?;
                ctx.push_unbroken_ln(&format!("</{}>", tag), indent)?;
            }
            Node::ListItem(item) => {
                for child in &item.children {
                    match child {
                        Node::Paragraph(paragraph) => {
                            for child in &paragraph.children {
                                self.write_html_to(ctx, indent, child, format, options)?;
                            }
                        }
                        _ => {
                            self.write_html_to(ctx, indent, child, format, options)?;
                        }
                    }
                }
            }
            Node::Html(html) => {
                // TODO Calculate line length
                ctx.html.push_str(&html.value);
            }
            Node::InlineCode(code) => {
                // TODO Calculate line length
                ctx.push_unbroken(
                    &self.render_inline_code(code.value.as_str(), format, options)?,
                    indent,
                )?;
            }
            Node::Code(code) => {
                ctx.push_unbroken(
                    &self.render_block_code(code.value.as_str(), format, options)?,
                    indent,
                )?;
            }
            Node::Blockquote(block_quote) => {
                // Should we enforce line length on block quotes?
                ctx.push_unbroken_ln("<blockquote>", indent)?;
                for child in &block_quote.children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken_ln("</blockquote>", indent)?;
            }
            Node::Link(link) => {
                ctx.push_unbroken(&format!("<a href=\"{}\">", link.url), indent)?;
                for child in &link.children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken("</a>", indent)?;
            }
            Node::Strong(Strong { children, .. }) => {
                ctx.push_unbroken("<strong>", indent)?;
                for child in children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken("</strong>", indent)?;
            }
            Node::Emphasis(Emphasis { children, .. }) => {
                ctx.push_unbroken("<em>", indent)?;
                for child in children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken("</em>", indent)?;
            }
            Node::Delete(Delete { children, .. }) => {
                // TODO Calculate line length
                ctx.push_unbroken("<s>", indent)?;
                for child in children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken("</s>", indent)?;
            }
            Node::Heading(heading) => {
                // TODO Calculate line length
                ctx.push_unbroken(&format!("<h{}>", heading.depth), indent)?;
                for child in &heading.children {
                    self.write_html_to(ctx, indent, child, format, options)?;
                }
                ctx.push_unbroken(&format!("</h{}>\n", heading.depth), indent)?;
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
    use crate::config::{CommentFormat, IndentType, RenderFormat, WeaverConfig};
    use crate::error::Error;
    use crate::formats::html::{HtmlRenderOptions, HtmlRenderer};
    use crate::formats::WordWrapConfig;
    use weaver_diff::assert_string_eq;

    #[test]
    fn test_html_renderer() -> Result<(), Error> {
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
                            ignore_newlines: false,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = HtmlRenderer::try_new(&config)?;
        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
          The file extension extracted from the `url.full`, excluding the leading dot."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"In some cases a URL may refer to an IP and/or port directly,
The file extension extracted from the {@code url.full}, excluding the leading dot."##
        );

        let markdown = r##"Follows
[OCI Image Manifest Specification](https://github.com/opencontainers/image-spec/blob/main/manifest.md),
and specifically the
[Digest property](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests).

An example can be found in
[Example Image Manifest](https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest)."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Follows
<a href="https://github.com/opencontainers/image-spec/blob/main/manifest.md">OCI Image Manifest Specification</a>,
and specifically the
<a href="https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests">Digest property</a>.
<p>
An example can be found in
<a href="https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest">Example Image Manifest</a>."##
        );

        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
without a domain name. In this case, the IP address would go to the domain field.
If the URL contains a [literal IPv6 address](https://www.rfc-editor.org/rfc/rfc2732#section-2)
enclosed by `[` and `]`, the `[` and `]` characters should also be captured in the domain field."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"In some cases a URL may refer to an IP and/or port directly,
without a domain name. In this case, the IP address would go to the domain field.
If the URL contains a <a href="https://www.rfc-editor.org/rfc/rfc2732#section-2">literal IPv6 address</a>
enclosed by {@code [} and {@code ]}, the {@code [} and {@code ]} characters should also be captured in the domain field."##
        );

        let markdown = r##"For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment
is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.

`url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`.
In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.

`url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed).
Sensitive content provided in `url.full` SHOULD be scrubbed when instrumentations can identify it."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"For network calls, URL usually has {@code scheme://host[:port][path][?query][#fragment]} format, where the fragment
is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.
<p>
{@code url.full} MUST NOT contain credentials passed via URL in form of {@code https://username:password@www.example.com/}.
In such case username and password SHOULD be redacted and attribute's value SHOULD be {@code https://REDACTED:REDACTED@www.example.com/}.
<p>
{@code url.full} SHOULD capture the absolute URL when it is available (or can be reconstructed).
Sensitive content provided in {@code url.full} SHOULD be scrubbed when instrumentations can identify it."##
        );

        let markdown = r##"Pool names are generally obtained via
[BufferPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/BufferPoolMXBean.html#getName())."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Pool names are generally obtained via
<a href="https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/BufferPoolMXBean.html#getName()">BufferPoolMXBean#getName()</a>."##
        );

        let markdown = r##"Value can be retrieved from value `space_name` of [`v8.getHeapSpaceStatistics()`](https://nodejs.org/api/v8.html#v8getheapspacestatistics)"##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Value can be retrieved from value {@code space_name} of <a href="https://nodejs.org/api/v8.html#v8getheapspacestatistics">{@code v8.getHeapSpaceStatistics()}</a>"##
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
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
<p>
When {@code error.type} is set to a type (e.g., an exception type), its
canonical class name identifying the type within the artifact SHOULD be used.
<p>
Instrumentations SHOULD document the list of errors they report.
<p>
The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
should be prepared for {@code error.type} to have high cardinality at query time when no
additional filters are applied.
<p>
If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
<p>
If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
it's RECOMMENDED to:
<ul>
  <li>Use a domain-specific attribute
  <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not.
</ul>
"##
        );
        Ok(())
    }

    #[test]
    fn test_html_renderer_word_wrap() -> Result<(), Error> {
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
                            line_length: Some(30),
                            ignore_newlines: true,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("java".to_owned()),
            ..WeaverConfig::default()
        };

        let renderer = HtmlRenderer::try_new(&config)?;
        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
          The file extension extracted from the `url.full`, excluding the leading dot."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"In some cases a URL may refer
to an IP and/or port directly,
The file extension extracted
from the {@code url.full},
excluding the leading dot."##
        );

        let markdown = r##"Follows
[OCI Image Manifest Specification](https://github.com/opencontainers/image-spec/blob/main/manifest.md),
and specifically the
[Digest property](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests).

An example can be found in
[Example Image Manifest](https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest)."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Follows
<a href="https://github.com/opencontainers/image-spec/blob/main/manifest.md">
OCI Image Manifest
Specification</a>, and
specifically the
<a href="https://github.com/opencontainers/image-spec/blob/main/descriptor.md#digests">
Digest property</a>.
<p>
An example can be found in
<a href="https://docs.docker.com/registry/spec/manifest-v2-2/#example-image-manifest">
Example Image Manifest</a>."##
        );

        let markdown = r##"In some cases a URL may refer to an IP and/or port directly,
without a domain name. In this case, the IP address would go to the domain field.
If the URL contains a [literal IPv6 address](https://www.rfc-editor.org/rfc/rfc2732#section-2)
enclosed by `[` and `]`, the `[` and `]` characters should also be captured in the domain field."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"In some cases a URL may refer
to an IP and/or port directly,
without a domain name. In this
case, the IP address would go
to the domain field. If the URL
contains a
<a href="https://www.rfc-editor.org/rfc/rfc2732#section-2">
literal IPv6 address</a>
enclosed by {@code [} and
{@code ]}, the {@code [} and
{@code ]} characters should
also be captured in the domain
field."##
        );

        let markdown = r##"For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment
is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.

`url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`.
In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.

`url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed).
Sensitive content provided in `url.full` SHOULD be scrubbed when instrumentations can identify it."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"For network calls, URL usually
has
{@code scheme://host[:port][path][?query][#fragment]}
 format, where the fragment is
not transmitted over HTTP, but
if it is known, it SHOULD be
included nevertheless.
<p>
{@code url.full} MUST NOT
contain credentials passed via
URL in form of
{@code https://username:password@www.example.com/}
. In such case username and
password SHOULD be redacted and
attribute's value SHOULD be
{@code https://REDACTED:REDACTED@www.example.com/}
.
<p>
{@code url.full} SHOULD capture
the absolute URL when it is
available (or can be
reconstructed). Sensitive
content provided in
{@code url.full} SHOULD be
scrubbed when instrumentations
can identify it."##
        );

        let markdown = r##"Pool names are generally obtained via
[BufferPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/BufferPoolMXBean.html#getName())."##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Pool names are generally
obtained via
<a href="https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/BufferPoolMXBean.html#getName()">
BufferPoolMXBean#getName()</a>
."##
        );

        let markdown = r##"Value can be retrieved from value `space_name` of [`v8.getHeapSpaceStatistics()`](https://nodejs.org/api/v8.html#v8getheapspacestatistics)"##;
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"Value can be retrieved from
value {@code space_name} of
<a href="https://nodejs.org/api/v8.html#v8getheapspacestatistics">
{@code v8.getHeapSpaceStatistics()}
</a>"##
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
        let html = renderer.render(markdown, "java", None)?;
        assert_string_eq!(
            &html,
            r##"The {@code error.type} SHOULD
be predictable, and SHOULD have
low cardinality.
<p>
When {@code error.type} is set
to a type (e.g., an exception
type), its canonical class name
identifying the type within the
artifact SHOULD be used.
<p>
Instrumentations SHOULD
document the list of errors
they report.
<p>
The cardinality of
{@code error.type} within one
instrumentation library SHOULD
be low. Telemetry consumers
that aggregate data from
multiple instrumentation
libraries and applications
should be prepared for
{@code error.type} to have high
cardinality at query time when
no additional filters are
applied.
<p>
If the operation has completed
successfully, instrumentations
SHOULD NOT set
{@code error.type}.
<p>
If a specific domain defines
its own set of error
identifiers (such as HTTP or
gRPC status codes), it's
RECOMMENDED to:
<ul>
  <li>Use a domain-specific
  attribute
  <li>Set {@code error.type} to
  capture all errors,
  regardless of whether they
  are defined within the
  domain-specific set or not.
</ul>
"##
        );
        Ok(())
    }
}
