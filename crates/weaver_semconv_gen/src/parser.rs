// SPDX-License-Identifier: Apache-2.0

//! Parsing Utilities.

use crate::Error;
use nom::bytes::complete::take_until;
use nom::error::ErrorKind;
use nom::error::ParseError;
use nom::multi::many0_count;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, multispace0},
    combinator::{opt, recognize, value},
    multi::separated_list0,
    sequence::pair,
    IResult, Parser,
};
use weaver_semconv::v2::signal_id::SignalId;

/// Weaver-based snipper generation arguments.
pub struct WeaverGenerateMarkdownArgs {
    /// The JQ expression to execute against the repository before rendering a template.
    pub query: String,
    /// The template to use when rendering a snippet.
    pub template: Option<String>,
}

/// Markdown-snippet generation arguments.
pub struct GenerateMarkdownArgs {
    /// The id of the metric, event, span or attribute group to render.
    pub id: String,
    /// Arguments the user specified that we've parsed.
    pub args: Vec<MarkdownGenParameters>,
}

impl GenerateMarkdownArgs {
    /// Returns true if a metric table should be rendered.
    pub fn is_metric_table(&self) -> bool {
        self.args
            .iter()
            .any(|a| matches!(a, MarkdownGenParameters::MetricTable))
    }

    /// Returns all tag filters in a list.
    pub fn tag_filters(&self) -> Vec<&str> {
        self.args
            .iter()
            .find_map(|arg| match arg {
                MarkdownGenParameters::Tag(value) => Some(value.as_str()),
                _ => None,
            })
            .into_iter()
            .collect()
    }
}

// TODO - this is based on https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/src/opentelemetry/semconv/templating/markdown/__init__.py#L503
// We can likely model this much better.
/// Parameters users can specify for generating markdown.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownGenParameters {
    /// Filter attributes to those with a given tag.
    Tag(String),
    /// Display all metrics in a group?
    Full,
    /// Generate a metric table
    MetricTable,
    /// Omit the requirement level.
    OmitRequirementLevel,
}

/// exact string we expect for starting a semconv snippet.
const SEMCONV_HEADER: &str = "semconv";
/// exact string we expect for ending a semconv snippet.
const SEMCONV_TRAILER: &str = "endsemconv";

/// exact string we expect from the new weaver snippet.
const WEAVER_HEADER: &str = "weaver";
/// exact string we expect for ending a weaver snippet.
const WEAVER_TRAILER: &str = "endweaver";

/// nom parser for tag values.
fn parse_value(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
    ))
    .parse(input)
}

/// nom parser for tag={value}.
fn parse_markdown_gen_tag(input: &str) -> IResult<&str, MarkdownGenParameters> {
    let (input, _) = tag("tag=")(input)?;
    let (input, value) = parse_value(input)?;
    Ok((input, MarkdownGenParameters::Tag(value.to_owned())))
}

/// nom parser for full.
fn parse_markdown_full(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(MarkdownGenParameters::Full, tag("full")).parse(input)
}

/// nom parser for metric_table.
fn parse_markdown_metric_table(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(MarkdownGenParameters::MetricTable, tag("metric_table")).parse(input)
}

/// nom parser for omit_requirement_level.
fn parse_markdown_omit(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(
        MarkdownGenParameters::OmitRequirementLevel,
        tag("omit_requirement_level"),
    )
    .parse(input)
}

/// nom parser for single parameters to semconv generation.
fn parse_markdown_gen_parameter(input: &str) -> IResult<&str, MarkdownGenParameters> {
    alt((
        parse_markdown_full,
        parse_markdown_metric_table,
        parse_markdown_omit,
        parse_markdown_gen_tag,
    ))
    .parse(input)
}

/// nom parser for arguments to semconv generation. ({arg},{arg},..)
fn parse_markdown_gen_parameters(input: &str) -> IResult<&str, Vec<MarkdownGenParameters>> {
    let (input, _) = tag("(")(input)?;
    let (input, result) = separated_list0(tag(","), parse_markdown_gen_parameter).parse(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, result))
}

/// nom parser for semconv ids.
fn parse_id(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alpha1, // First character must be alpha, then anything is accepted.
        many0_count(alt((alphanumeric1, tag("."), tag("_"), tag("-")))),
    ))
    .parse(input)
}

/// nom parser for HTML comments: `<!--{comment}-->
fn parse_html_comment(input: &str) -> IResult<&str, &str> {
    // Comments must have the following format:
    // The string "<!--".
    let (input, _) = tag("<!--")(input)?;
    // Optionally, text, with the additional restriction that the text must not start with the string ">", nor start with the string "->", nor contain the strings "<!--", "-->", or "--!>", nor end with the string "<!-".
    let (input, result) = take_until("-->")(input)?;
    // The string "-->".
    let (input, _) = tag("-->")(input)?;
    Ok((input, result))
}

/// Parses the semantic convention header and directives for markdown generation.
fn parse_semconv_snippet_directive(input: &str) -> IResult<&str, GenerateMarkdownArgs> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag(SEMCONV_HEADER)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, id) = parse_id(input)?;
    let (input, opt_args) = opt(parse_markdown_gen_parameters).parse(input)?;
    let (input, _) = multispace0(input)?;
    Ok((
        input,
        GenerateMarkdownArgs {
            id: id.to_owned(),
            args: opt_args.unwrap_or(Vec::new()),
        },
    ))
}

/// Parses the weaver header and directives for markdown generation.
fn parse_weaver_snippet_args(input: &str) -> IResult<&str, WeaverGenerateMarkdownArgs> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag(WEAVER_HEADER)(input)?;
    let (input, _) = multispace0(input)?;
    // TODO - parse JQ expression and optional template to use.
    let (input, template) = opt(parse_weaver_template_directive).parse(input)?;
    let (input, _) = multispace0(input)?;

    // Remaining input is assumed to be the JQ expression.
    Ok((
        "",
        WeaverGenerateMarkdownArgs {
            query: input.trim().to_owned(),
            template,
        },
    ))
}

fn parse_weaver_template_directive(input: &str) -> IResult<&str, String> {
    let (input, _) = tag("template:")(input)?;
    // TODO - more flexible template string options.
    let (input, template) = parse_id(input)?;
    Ok((input, template.to_owned()))
}

/// nom parser for <!-- endweaver -->
fn parse_weaver_trailer(input: &str) -> IResult<&str, ()> {
    let (input, snippet) = parse_html_comment(input)?;
    let (snippet, _) = multispace0(snippet)?;
    let (snippet, _) = tag(WEAVER_TRAILER)(snippet)?;
    let (snippet, _) = multispace0(snippet)?;
    if snippet.is_empty() {
        Ok((input, ()))
    } else {
        Err(nom::Err::Failure(ParseError::from_error_kind(
            snippet,
            ErrorKind::Not,
        )))
    }
}

/// nom parser for <!-- weaver {template?} {query} -->
fn parse_weaver_snippet_raw(input: &str) -> IResult<&str, WeaverGenerateMarkdownArgs> {
    let (input, snippet) = parse_html_comment(input)?;
    let (remains, result) = parse_weaver_snippet_args(snippet)?;
    if remains.is_empty() {
        Ok((input, result))
    } else {
        Err(nom::Err::Failure(ParseError::from_error_kind(
            remains,
            ErrorKind::IsNot,
        )))
    }
}

/// nom parser for <!-- semconv {id}({args}) -->
fn parse_markdown_snippet_raw(input: &str) -> IResult<&str, GenerateMarkdownArgs> {
    let (input, snippet) = parse_html_comment(input)?;
    let (remains, result) = parse_semconv_snippet_directive(snippet)?;
    if remains.is_empty() {
        Ok((input, result))
    } else {
        Err(nom::Err::Failure(ParseError::from_error_kind(
            remains,
            ErrorKind::IsNot,
        )))
    }
}

/// nom parser for <!-- endsemconv -->
fn parse_semconv_trailer(input: &str) -> IResult<&str, ()> {
    let (input, snippet) = parse_html_comment(input)?;
    let (snippet, _) = multispace0(snippet)?;
    let (snippet, _) = tag(SEMCONV_TRAILER)(snippet)?;
    let (snippet, _) = multispace0(snippet)?;
    if snippet.is_empty() {
        Ok((input, ()))
    } else {
        Err(nom::Err::Failure(ParseError::from_error_kind(
            snippet,
            ErrorKind::Not,
        )))
    }
}

/// Returns true if the line is the <!-- endsemconv --> marker for markdown snippets.
pub fn is_semconv_trailer(line: &str) -> bool {
    matches!(parse_semconv_trailer(line), Ok((rest, _)) if rest.trim().is_empty())
}

/// Returns true if the line is the <!-- endweaver --> marker for markdown snippets.
pub fn is_weaver_trailer(line: &str) -> bool {
    matches!(parse_weaver_trailer(line), Ok((rest, _)) if rest.trim().is_empty())
}

/// Returns true if the line begins a markdown snippet directive and needs to be parsed.
pub fn is_markdown_snippet_directive(line: &str) -> bool {
    matches!(parse_markdown_snippet_raw(line), Ok((rest, _)) if rest.trim().is_empty())
}

/// Returns true if the line begins a weaver snippet directive and needs to be parsed.
pub fn is_weaver_directive(line: &str) -> bool {
    matches!(parse_weaver_snippet_raw(line), Ok((rest, _)) if rest.trim().is_empty())
}

/// Returns the markdown args for this markdown snippet directive.
pub fn parse_markdown_snippet_directive(line: &str) -> Result<GenerateMarkdownArgs, Error> {
    match parse_markdown_snippet_raw(line) {
        Ok((rest, result)) if rest.trim().is_empty() => Ok(result),
        _ => Err(Error::InvalidSnippetHeader {
            header: line.to_owned(),
        }),
    }
}

/// Returns the arguments used to generate a template snippet.
pub fn parse_weaver_snippet_directive(line: &str) -> Result<WeaverGenerateMarkdownArgs, Error> {
    match parse_weaver_snippet_raw(line) {
        Ok((rest, result)) if rest.trim().is_empty() => Ok(result),
        _ => Err(Error::InvalidSnippetHeader {
            header: line.to_owned(),
        }),
    }
}

/// Returns the V2 id lookup structure
pub(crate) fn parse_id_lookup_v2(input: &str) -> Result<IdLookupV2, Error> {
    match parse_id_lookup(input) {
        Ok((rest, result)) if rest.trim().is_empty() => Ok(result),
        _ => Err(Error::InvalidSnippetId {
            id: input.to_owned(),
        }),
    }
}

/// A query to lookup a particular semantic convention item in the V2 schema.
#[derive(Debug, PartialEq)]
pub(crate) enum IdLookupV2 {
    Registry(RegistryLookup),
    Refinement(RefinementLookup),
}

/// Lookup a particular item in the registry.
#[derive(Debug, PartialEq)]
pub(crate) enum RegistryLookup {
    Attribute { id: String },
    AttributeGroup { id: SignalId },
    Span { id: SignalId },
    Metric { id: SignalId },
    Event { id: SignalId },
    Entity { id: SignalId },
}

/// Lookup a particular refinement by id.
#[derive(Debug, PartialEq)]
pub(crate) enum RefinementLookup {
    Span { id: SignalId },
    Metric { id: SignalId },
    Event { id: SignalId },
}

fn parse_partial_id(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alpha1, // First character must be alpha, then anything is accepted.
        many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
    ))
    .parse(input)
}

fn parse_id_lookup(input: &str) -> IResult<&str, IdLookupV2> {
    let (input, name) = parse_partial_id(input)?;
    let (input, _) = tag(".").parse(input)?;
    match name {
        "registry" => parse_registry_lookup(input).map(|(i, l)| (i, IdLookupV2::Registry(l))),
        "refinements" => {
            parse_refinement_lookup(input).map(|(i, l)| (i, IdLookupV2::Refinement(l)))
        }
        _ => Err(nom::Err::Failure(ParseError::from_error_kind(
            input,
            ErrorKind::IsNot,
        ))),
    }
}

fn parse_registry_lookup(input: &str) -> IResult<&str, RegistryLookup> {
    let (input, name) = parse_partial_id(input)?;
    let (input, _) = tag(".").parse(input)?;
    let (input, id_rest) = parse_id(input)?;
    match name {
        "attributes" => Ok((
            input,
            RegistryLookup::Attribute {
                id: id_rest.to_owned(),
            },
        )),
        "attribute_groups" => Ok((
            input,
            RegistryLookup::AttributeGroup {
                id: id_rest.to_owned().into(),
            },
        )),
        "spans" => Ok((
            input,
            RegistryLookup::Span {
                id: id_rest.to_owned().into(),
            },
        )),
        "metrics" => Ok((
            input,
            RegistryLookup::Metric {
                id: id_rest.to_owned().into(),
            },
        )),
        "events" => Ok((
            input,
            RegistryLookup::Event {
                id: id_rest.to_owned().into(),
            },
        )),
        "entities" => Ok((
            input,
            RegistryLookup::Entity {
                id: id_rest.to_owned().into(),
            },
        )),
        _ => Err(nom::Err::Failure(ParseError::from_error_kind(
            input,
            ErrorKind::IsNot,
        ))),
    }
}

fn parse_refinement_lookup(input: &str) -> IResult<&str, RefinementLookup> {
    let (input, name) = parse_partial_id(input)?;
    let (input, _) = tag(".").parse(input)?;
    let (input, id_rest) = parse_id(input)?;
    match name {
        "spans" => Ok((
            input,
            RefinementLookup::Span {
                id: id_rest.to_owned().into(),
            },
        )),
        "metrics" => Ok((
            input,
            RefinementLookup::Metric {
                id: id_rest.to_owned().into(),
            },
        )),
        "events" => Ok((
            input,
            RefinementLookup::Event {
                id: id_rest.to_owned().into(),
            },
        )),
        _ => Err(nom::Err::Failure(ParseError::from_error_kind(
            input,
            ErrorKind::IsNot,
        ))),
    }
}

#[cfg(test)]
mod tests {

    use crate::parser::{
        is_markdown_snippet_directive, is_semconv_trailer, is_weaver_trailer, parse_id_lookup_v2,
        parse_weaver_snippet_directive, IdLookupV2, MarkdownGenParameters, RefinementLookup,
        RegistryLookup,
    };
    use crate::Error;

    use super::parse_markdown_snippet_directive;
    #[test]
    fn recognizes_trailer() {
        assert!(is_semconv_trailer("<!-- endsemconv -->"));
        assert!(!is_semconv_trailer("<!-- endsemconvded -->"));
        // Add whitespace friendly versions
        assert!(is_semconv_trailer("<!--endsemconv-->"));
        assert!(is_semconv_trailer("<!-- endsemconv-->"));
        assert!(is_semconv_trailer("<!--endsemconv -->"));
    }

    #[test]
    fn recognizes_header() {
        assert!(is_markdown_snippet_directive(
            "<!-- semconv registry.user_agent -->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!-- semconv registry.user_agent.p99 -->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!-- semconv my.id(full) -->"
        ));
        assert!(is_markdown_snippet_directive("<!-- semconv my.id -->"));
        assert!(is_markdown_snippet_directive(
            "<!-- semconv my.id(metric_table) -->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!-- semconv my.id(omit_requirement_level) -->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!-- semconv my.id(omit_requirement_level,tag=baz) -->"
        ));
        assert!(!is_markdown_snippet_directive("hello"));
        assert!(!is_markdown_snippet_directive(
            "<!-- other semconv stuff -->"
        ));
        // Test ignoring whitespace
        assert!(is_markdown_snippet_directive("<!-- semconv stuff-->"));
        assert!(is_markdown_snippet_directive("<!--semconv stuff -->"));
        assert!(is_markdown_snippet_directive("<!--semconv stuff-->"));
    }

    #[test]
    fn parses_header_success() -> Result<(), Error> {
        let result = parse_markdown_snippet_directive("<!-- semconv my.id -->")?;
        assert_eq!(result.id, "my.id");
        assert_eq!(result.args.len(), 0);

        let result = parse_markdown_snippet_directive("<!-- semconv my.id(metric_table) -->")?;
        assert_eq!(result.id, "my.id");
        assert_eq!(result.args.len(), 1);
        assert_eq!(result.args[0], MarkdownGenParameters::MetricTable);

        let result =
            parse_markdown_snippet_directive("<!-- semconv my.id(omit_requirement_level) -->")?;
        assert_eq!(result.id, "my.id");
        assert_eq!(result.args.len(), 1);
        assert_eq!(result.args[0], MarkdownGenParameters::OmitRequirementLevel);

        let result = parse_markdown_snippet_directive("<!-- semconv registry.messaging(omit_requirement_level,tag=tech-specific-rabbitmq) -->")?;
        assert_eq!(result.id, "registry.messaging");
        assert_eq!(result.args.len(), 2);
        assert!(result
            .args
            .iter()
            .any(|v| v == &MarkdownGenParameters::OmitRequirementLevel));
        assert!(result
            .args
            .iter()
            .any(|v| v == &MarkdownGenParameters::Tag("tech-specific-rabbitmq".into())));

        let result = parse_markdown_snippet_directive("<!--semconv stuff-->")?;
        assert_eq!(result.id, "stuff");

        Ok(())
    }

    #[test]
    fn parse_weaver_header() -> Result<(), Error> {
        let result = parse_weaver_snippet_directive("<!-- weaver .registry.metrics -->")?;
        assert_eq!(result.template, None);
        assert_eq!(result.query, ".registry.metrics");

        let result =
            parse_weaver_snippet_directive("<!-- weaver template:test.j2 .registry.spans[] -->")?;
        assert_eq!(result.template, Some("test.j2".to_owned()));
        assert_eq!(result.query, ".registry.spans[]");
        Ok(())
    }

    #[test]
    fn parse_weaver_trailer() {
        assert!(is_weaver_trailer("<!-- endweaver -->"));
        assert!(!is_weaver_trailer("<!-- endweaverded -->"));
    }

    #[test]
    fn parse_v2_lookups() -> Result<(), Error> {
        assert_eq!(
            parse_id_lookup_v2("registry.attributes.user_agent")?,
            IdLookupV2::Registry(RegistryLookup::Attribute {
                id: "user_agent".to_owned()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.metrics.one")?,
            IdLookupV2::Registry(RegistryLookup::Metric {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.spans.one")?,
            IdLookupV2::Registry(RegistryLookup::Span {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.events.one")?,
            IdLookupV2::Registry(RegistryLookup::Event {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.entities.one")?,
            IdLookupV2::Registry(RegistryLookup::Entity {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.attribute_groups.one")?,
            IdLookupV2::Registry(RegistryLookup::AttributeGroup {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("refinements.metrics.one")?,
            IdLookupV2::Refinement(RefinementLookup::Metric {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("refinements.spans.one")?,
            IdLookupV2::Refinement(RefinementLookup::Span {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("refinements.events.one")?,
            IdLookupV2::Refinement(RefinementLookup::Event {
                id: "one".to_owned().into()
            })
        );
        assert_eq!(
            parse_id_lookup_v2("registry.metrics.multiple.dots.and_underscores")?,
            IdLookupV2::Registry(RegistryLookup::Metric {
                id: "multiple.dots.and_underscores".to_owned().into()
            })
        );
        Ok(())
    }
}
