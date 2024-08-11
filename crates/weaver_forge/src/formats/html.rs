use crate::config::{RenderOptions, WeaverConfig};
use crate::error::Error;
use crate::error::Error::InvalidCodeSnippet;
use crate::install_weaver_extensions;
use markdown::mdast::Node;
use markdown::Constructs;
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const INLINE_CODE_SNIPPET: &str = "inline";
const BLOCK_CODE_SNIPPET: &str = "block";

/// Options for rendering markdown to HTML.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HtmlRenderOptions {
    /// Use old-style HTML paragraphs (i.e. single <p> tag).
    /// Default is false.
    #[serde(default)]
    pub(crate) old_style_paragraph: bool,
    /// Omit closing </li> tags in lists.
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
    html_options_by_format: HashMap<String, HtmlRenderOptions>,
    env: Environment<'source>,
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
            html_options_by_format: config
                .comment_formats
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(name, format)| match format.render_options {
                    RenderOptions::Html(html_options) => Some((name, html_options)),
                    RenderOptions::Markdown => None,
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
    pub fn render(&self, markdown: &str, format: &str) -> Result<String, Error> {
        let html_render_options = if let Some(options) = self.html_options_by_format.get(format) {
            options
        } else {
            return Err(Error::CommentFormatNotFound {
                format: format.to_owned(),
                formats: self.html_options_by_format.keys().cloned().collect(),
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
        let mut html = String::new();
        self.write_html_to(&mut html, "", &md_node, format, html_render_options)?;
        Ok(html)
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
            .render_str(&options.inline_code_snippet, &ctx)
            .map_err(|e| InvalidCodeSnippet {
                format: format.to_owned(),
                mode: INLINE_CODE_SNIPPET.to_owned(),
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
            .render_str(&options.block_code_snippet, &ctx)
            .map_err(|e| InvalidCodeSnippet {
                format: format.to_owned(),
                mode: BLOCK_CODE_SNIPPET.to_owned(),
                error: e.to_string(),
            })
    }

    /// Render HTML from a markdown AST tree into a buffer.
    fn write_html_to(
        &self,
        buffer: &mut String,
        indent: &str,
        md_node: &Node,
        format: &str,
        options: &HtmlRenderOptions,
    ) -> Result<(), Error> {
        match md_node {
            Node::Root(root) => {
                for child in &root.children {
                    self.write_html_to(buffer, indent, child, format, options)?;
                }
            }
            Node::Text(text) => {
                buffer.push_str(&text.value);
            }
            Node::Paragraph(p) => {
                if !options.old_style_paragraph {
                    buffer.push_str("<p>");
                }
                for child in &p.children {
                    self.write_html_to(buffer, indent, child, format, options)?;
                }
                if options.old_style_paragraph {
                    buffer.push_str("\n<p>\n");
                } else {
                    buffer.push_str("</p>\n");
                }
            }
            Node::List(list) => {
                let tag = if list.ordered { "ol" } else { "ul" };
                buffer.push_str(&format!("<{}>\n", tag));
                let li_indent = format!("{}  ", indent);
                for item in &list.children {
                    buffer.push_str(&format!("{}<li>", li_indent));
                    self.write_html_to(buffer, indent, item, format, options)?;
                    if options.omit_closing_li {
                        buffer.push('\n');
                    } else {
                        buffer.push_str("</li>\n");
                    }
                }
                buffer.push_str(&format!("</{}>\n", tag));
            }
            Node::ListItem(item) => {
                for child in &item.children {
                    match child {
                        Node::Paragraph(paragraph) => {
                            for child in &paragraph.children {
                                self.write_html_to(buffer, indent, child, format, options)?;
                            }
                        }
                        _ => {
                            self.write_html_to(buffer, indent, child, format, options)?;
                        }
                    }
                }
            }
            Node::Html(html) => {
                buffer.push_str(&html.value);
            }
            Node::InlineCode(code) => {
                buffer.push_str(
                    self.render_inline_code(code.value.as_str(), format, options)?
                        .as_str(),
                );
            }
            Node::Code(code) => {
                buffer.push_str(
                    self.render_block_code(code.value.as_str(), format, options)?
                        .as_str(),
                );
            }
            Node::BlockQuote(block_quote) => {
                buffer.push_str("<blockquote>\n");
                for child in &block_quote.children {
                    self.write_html_to(buffer, indent, child, format, options)?;
                }
                buffer.push_str("</blockquote>\n");
            }
            Node::Toml(_) => {}
            Node::Yaml(_) => {}
            Node::Break(_) => {}
            Node::Delete(_) => {}
            Node::Emphasis(_) => {}
            Node::Image(_) => {}
            Node::ImageReference(_) => {}
            Node::Link(link) => {
                buffer.push_str(&format!("<a href=\"{}\">", link.url));
                for child in &link.children {
                    self.write_html_to(buffer, indent, child, format, options)?;
                }
                buffer.push_str("</a>");
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
    use crate::config::{CommentFormat, RenderOptions, WeaverConfig};
    use crate::error::Error;
    use crate::formats::html::{HtmlRenderOptions, HtmlRenderer};

    #[test]
    fn test_html_renderer() -> Result<(), Error> {
        let markdown = r##"
Graphics is the abstract base class for all graphics contexts
which allow an application to draw onto components realized on
various devices or onto off-screen images.
A Graphics object encapsulates the state information needed
for the various rendering operations that Java supports.  This
state information includes:
- The Component to draw on
- A translation origin for rendering and clipping coordinates
- The current clip
- The current color
- The current font
- The current logical pixel operation function (XOR or Paint)
- The current XOR alternation color
  (see <a href="#setXORMode">setXORMode</a>)

Coordinates are infinitely thin and lie between the pixels of the
output device.
Operations which draw the outline of a figure operate by traversing
along the infinitely thin path with a pixel-sized pen that hangs
down and to the right of the anchor point on the path.
Operations which fill a figure operate by filling the interior
of the infinitely thin path.
Operations which render horizontal text render the ascending
portion of the characters entirely above the baseline coordinate.

Some important points to consider are that drawing a figure that
covers a given rectangle will occupy one extra row of pixels on
the right and bottom edges compared to filling a figure that is
bounded by that same rectangle.
Also, drawing a horizontal line along the same y coordinate as
the baseline of a line of text will draw the line entirely below
the text except for any descenders.
Both of these properties are due to the pen hanging down and to
the right from the path that it traverses.
This is now an inline code `let x = 5;` and this is a block code:

```rust
fn main() {
    println!("Hello, world!");
}
```
"##;
        let config = WeaverConfig {
            comment_formats: Some(
                vec![(
                    "javadoc".to_owned(),
                    CommentFormat {
                        render_options: RenderOptions::Html(HtmlRenderOptions {
                            old_style_paragraph: true,
                            omit_closing_li: false,
                            ..HtmlRenderOptions::default()
                        }),
                        transform_options: Default::default(),
                    },
                )]
                .into_iter()
                .collect(),
            ),
            default_comment_format: Some("javadoc".to_owned()),
            ..WeaverConfig::default()
        };
        let renderer = HtmlRenderer::try_new(&config)?;
        let html = renderer.render(markdown, "javadoc")?;
        println!("{}", html);

        Ok(())
    }
}
