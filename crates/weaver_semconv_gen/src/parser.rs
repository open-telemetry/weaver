// SPDX-License-Identifier: Apache-2.0

//! Parsing Utilities.

use crate::Error;
use crate::GenerateMarkdownArgs;
use crate::MarkdownGenParameters;
use nom::multi::many0_count;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, multispace0},
    combinator::{opt, recognize, value},
    multi::separated_list0,
    sequence::pair,
    IResult,
};

/// exact string we expect for ending a semconv snippet.
const SEMCONV_TRAILER: &str = "<!-- endsemconv -->";

/// nom parser for tag values.
fn parse_value(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)
}

/// nom parser for tag={value}.
fn parse_markdown_gen_tag(input: &str) -> IResult<&str, MarkdownGenParameters> {
    let (input, _) = tag("tag=")(input)?;
    let (input, value) = parse_value(input)?;
    Ok((input, MarkdownGenParameters::Tag(value.to_owned())))
}

/// nom parser for full.
fn parse_markdown_full(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(MarkdownGenParameters::Full, tag("full"))(input)
}

/// nom parser for metric_table.
fn parse_markdown_metric_table(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(MarkdownGenParameters::MetricTable, tag("metric_table"))(input)
}

/// nom parser for omit_requirement_level.
fn parse_markdown_omit(input: &str) -> IResult<&str, MarkdownGenParameters> {
    value(
        MarkdownGenParameters::OmitRequirementLevel,
        tag("omit_requirement_level"),
    )(input)
}

/// nom parser for single parameters to semconv generation.
fn parse_markdown_gen_parameter(input: &str) -> IResult<&str, MarkdownGenParameters> {
    alt((
        parse_markdown_full,
        parse_markdown_metric_table,
        parse_markdown_omit,
        parse_markdown_gen_tag,
    ))(input)
}

/// nom parser for arguments to semconv generation. ({arg},{arg},..)
fn parse_markdown_gen_parameters(input: &str) -> IResult<&str, Vec<MarkdownGenParameters>> {
    let (input, _) = tag("(")(input)?;
    let (input, result) = separated_list0(tag(","), parse_markdown_gen_parameter)(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, result))
}

/// nom parser for semconv ids.
fn parse_id(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alpha1, // First character must be alpha, then anything is accepted.
        many0_count(alt((alphanumeric1, tag("."), tag("_"), tag("-")))),
    ))(input)
}

/// nom parser for <!-- semconv {id}({args}) -->
fn parse_markdown_snippet_raw(input: &str) -> IResult<&str, GenerateMarkdownArgs> {
    let (input, _) = tag("<!--")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("semconv")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, id) = parse_id(input)?;
    let (input, opt_args) = opt(parse_markdown_gen_parameters)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("-->")(input)?;

    Ok((
        input,
        GenerateMarkdownArgs {
            id: id.to_owned(),
            args: opt_args.unwrap_or(Vec::new()),
        },
    ))
}

/// Returns true if the line is the <!-- endsemconv --> marker for markdown snippets.
pub fn is_semconv_trailer(line: &str) -> bool {
    // TODO - more flexibility in what we recognize here.
    line.trim() == SEMCONV_TRAILER
}

/// Returns true if the line begins a markdown snippet directive and needs tobe parsed.
pub fn is_markdown_snippet_directive(line: &str) -> bool {
    matches!(parse_markdown_snippet_raw(line), Ok((rest, _)) if rest.trim().is_empty())
}

/// Returns the markdown args for this markdown snippet directive.
pub fn parse_markdown_snippet_directive(line: &str) -> Result<GenerateMarkdownArgs, Error> {
    match parse_markdown_snippet_raw(line) {
        Ok((rest, result)) if rest.trim().is_empty() => Ok(result),
        Ok((rest, _)) => {
            println!("Failed to parse {line}, leftover: [{rest}]");
            Err(Error::InvalidSnippetHeader { header: line.to_owned() })
        },
        Err(e) => {
            println!("Failed to parse {line}, errors: [{e}]");
            Err(Error::InvalidSnippetHeader {
                header: line.to_owned(),
            })
        },
    }
}

#[cfg(test)]
mod tests {

    use crate::parser::{is_markdown_snippet_directive, is_semconv_trailer};
    use crate::{Error, MarkdownGenParameters};

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
        assert!(is_markdown_snippet_directive(
            "<!-- semconv stuff-->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!--semconv stuff -->"
        ));
        assert!(is_markdown_snippet_directive(
            "<!--semconv stuff-->"
        ));
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

        let result =
        parse_markdown_snippet_directive("<!--semconv stuff-->")?;
        assert_eq!(result.id, "stuff");

        Ok(())
    }
}
