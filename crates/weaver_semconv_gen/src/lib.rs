// SPDX-License-Identifier: Apache-2.0

//! This crate will generate code for markdown files.
//! The entire crate is a rush job to catch feature parity w/ existing python tooling by
//! poorly porting the code into RUST.  We expect to optimise and improve things over time.

use miette::Diagnostic;
use serde::Serialize;
use std::{fmt, fs};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_common::result::WResult;
use weaver_diff::diff_output;

mod parser;
mod v1;
mod v2;

/// SnippetGenerator for v1 resolution.
pub use v1::SnippetGenerator;
pub use v2::SnippetGenerator as SnipperGeneratorV2;

use crate::parser::{GenerateMarkdownArgs, WeaverGenerateMarkdownArgs};

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Using <!-- weaver ... --> is not supported.
    #[error("Weaver snippets are not supported in V1")]
    WeaverSnippetNotSupported,

    /// Thrown when we are unable to find a semconv by id.
    #[error("Could not find: {id}")]
    GroupNotFound {
        /// The id of the semconv lookup
        id: String,
    },

    /// Thrown when forcing a group to be a metricl
    #[error("Expected metric: {id}")]
    GroupMustBeMetric {
        /// The id of the semconv lookup that was not a metric.
        id: String,
    },
    /// Thrown when rendering an attribute group, but no attributes remain after filtering.
    #[error("No attributes retained for '{id}' filtering by '{filter}'")]
    GroupHasNoRenderableAttributesAfterFilter {
        /// The id of the semconv lookup.
        id: String,
        /// The filter for which attributes to display.
        filter: String,
    },
    /// Errors thrown when we are running a dry run and markdown doesn't match.
    #[error("Markdown is not equal:\n{}", diff_output(.original, .updated))]
    MarkdownIsNotEqual {
        /// Original markdown value.
        original: String,
        /// Updated markdown value.
        updated: String,
        // TODO - smart diff.
    },
    /// Thrown when snippet header is invalid.
    #[error("Could not parse snippet header: [{header}]")]
    InvalidSnippetHeader {
        /// Markdown snippet identifier <!-- semconv {header} -->
        header: String,
    },

    /// A comment looks like a snippet marker but can't be parsed.
    #[error("Invalid markdown snippet marker in {path}:{line}: {marker}")]
    #[diagnostic(severity(Warning))]
    InvalidSnippetMarker {
        /// The markdown file.
        path: String,
        /// The line number, starting at 1.
        line: usize,
        /// The marker as written.
        marker: String,
    },

    /// Thrown when a snippet lookup id is not valid.
    #[error("Could not parse snippet id: [{id}]")]
    InvalidSnippetId {
        /// Markdown snipper identifier:  <!-- semconv {id} -->
        id: String,
    },
    /// Errors from using std io library.
    #[error("{0}")]
    StdIoError(String),

    /// Errors from using std fmt library.
    #[error("{error}")]
    StdFmtError {
        /// The error message.
        error: String,
    },

    /// Errors from using weaver_semconv.
    #[error(transparent)]
    SemconvError(#[from] weaver_semconv::Error),

    /// Errors from using weaver_resolver.
    #[error(transparent)]
    ResolverError(#[from] weaver_resolver::Error),

    /// Errors from using weaver_forge.
    #[error(transparent)]
    ForgeError(#[from] weaver_forge::error::Error),

    /// A container for multiple errors.
    #[error("{}", format_errors(.0))]
    CompoundError(Vec<Error>),
}

enum SnippetType {
    Semconv,
    Weaver,
}

/// A trait denoting how to update the contents of a markdown using `semconv` snippet headers.
pub trait MarkdownSnippetGenerator {
    /// Update the contents of a markdown file.
    ///
    /// file: The markdown file.
    /// dry_run: Whether to actually change the file or just report if changes would be made.
    /// attribute_registry_base_url: Legacy mechanism to pass parameters to the snippet templates.
    /// Returns: empty with any marker warnings, or an error if one occurred.
    fn update_markdown(
        &self,
        file: &str,
        dry_run: bool,
        attribute_registry_base_url: Option<&str>,
    ) -> WResult<(), Error> {
        let original_markdown = match fs::read_to_string(file) {
            Ok(contents) => contents.replace("\r\n", "\n"),
            Err(e) => return WResult::FatalErr(Error::StdIoError(e.to_string())),
        };
        let (updated_markdown, warnings) = match self
            .update_markdown_contents(file, &original_markdown, attribute_registry_base_url)
            .into_result_with_non_fatal()
        {
            Ok(result) => result,
            Err(e) => return WResult::FatalErr(e),
        };
        if !dry_run {
            if let Err(e) = fs::write(file, updated_markdown) {
                return WResult::FatalErr(Error::StdIoError(e.to_string()));
            }
        } else if original_markdown != updated_markdown {
            return WResult::FatalErr(Error::MarkdownIsNotEqual {
                original: original_markdown,
                updated: updated_markdown,
            });
        }
        WResult::with_non_fatal_errors((), warnings)
    }

    /// Update the contents of a markdown string.
    ///
    /// file: The markdown file, used in warnings.
    /// contents: The contents of the markdown.
    /// attribute_registry_base_url: Legacy mechanism to pass parameters to the snippet templates.
    /// Returns: the updated markdown with any marker warnings, or an error.
    fn update_markdown_contents(
        &self,
        file: &str,
        contents: &str,
        attribute_registry_base_url: Option<&str>,
    ) -> WResult<String, Error> {
        let mut result = String::new();
        let mut warnings = Vec::new();
        let mut snippet_context: Option<SnippetType> = None;
        for (index, line) in contents.lines().enumerate() {
            if parser::looks_like_snippet_marker(line) && !parser::is_valid_snippet_marker(line) {
                warnings.push(Error::InvalidSnippetMarker {
                    path: file.to_owned(),
                    line: index + 1,
                    marker: line.to_owned(),
                });
            }
            match snippet_context {
                Some(SnippetType::Semconv) => {
                    if parser::is_semconv_trailer(line) {
                        result.push_str(line);
                        result.push('\n');
                        snippet_context = None;
                    }
                }
                Some(SnippetType::Weaver) => {
                    if parser::is_weaver_trailer(line) {
                        result.push_str(line);
                        result.push('\n');
                        snippet_context = None;
                    }
                }
                None => {
                    // Always push this line.
                    result.push_str(line);
                    // TODO - don't do this on last line.
                    result.push('\n');
                    // Check to see if line matches snippet request.
                    // If so, generate the snippet and continue.
                    if parser::is_markdown_snippet_directive(line) {
                        snippet_context = Some(SnippetType::Semconv);
                        match parser::parse_markdown_snippet_directive(line).and_then(|arg| {
                            self.generate_markdown_snippet(arg, attribute_registry_base_url)
                        }) {
                            Ok(snippet) => result.push_str(&snippet),
                            Err(e) => return WResult::FatalErr(e),
                        }
                    } else if parser::is_weaver_directive(line) {
                        snippet_context = Some(SnippetType::Weaver);
                        match parser::parse_weaver_snippet_directive(line)
                            .and_then(|arg| self.generate_weaver_snippet(arg))
                        {
                            Ok(snippet) => result.push_str(&snippet),
                            Err(e) => return WResult::FatalErr(e),
                        }
                    }
                }
            }
        }
        WResult::with_non_fatal_errors(result, warnings)
    }

    /// Generates a markdown snippet for a given parsed argument.
    // TODO - move registry base url into state of the struct...
    fn generate_markdown_snippet(
        &self,
        args: GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error>;

    /// Generates a snippet for a given parsed weaver command.
    fn generate_weaver_snippet(&self, args: WeaverGenerateMarkdownArgs) -> Result<String, Error>;
}

/// Our error supports WeaverError's error combination capabilities.
impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

/// Converts our error into DiagnostMessages.
impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        match error {
            Error::CompoundError(errors) => DiagnosticMessages::new(
                errors
                    .into_iter()
                    .flat_map(|e| {
                        let diag_msgs: DiagnosticMessages = e.into();
                        diag_msgs.into_inner()
                    })
                    .collect(),
            ),
            Error::SemconvError(e) => e.into(),
            Error::ResolverError(e) => e.into(),
            Error::ForgeError(e) => e.into(),
            _ => DiagnosticMessages::new(vec![DiagnosticMessage::new(error)]),
        }
    }
}

/// Converts format errors to io errors.
impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error::StdIoError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{GenerateMarkdownArgs, WeaverGenerateMarkdownArgs};
    use crate::{Error, MarkdownSnippetGenerator};
    use std::fs;
    use weaver_common::result::WResult;

    struct TestGenerator;

    impl MarkdownSnippetGenerator for TestGenerator {
        fn generate_markdown_snippet(
            &self,
            _args: GenerateMarkdownArgs,
            _attribute_registry_base_url: Option<&str>,
        ) -> Result<String, Error> {
            Ok("generated\n".to_owned())
        }

        fn generate_weaver_snippet(
            &self,
            _args: WeaverGenerateMarkdownArgs,
        ) -> Result<String, Error> {
            Ok("generated\n".to_owned())
        }
    }

    /// A generator whose snippets always fail.
    struct FailingGenerator;

    impl MarkdownSnippetGenerator for FailingGenerator {
        fn generate_markdown_snippet(
            &self,
            _args: GenerateMarkdownArgs,
            _attribute_registry_base_url: Option<&str>,
        ) -> Result<String, Error> {
            Err(Error::GroupNotFound {
                id: "metric.test".to_owned(),
            })
        }

        fn generate_weaver_snippet(
            &self,
            _args: WeaverGenerateMarkdownArgs,
        ) -> Result<String, Error> {
            Err(Error::WeaverSnippetNotSupported)
        }
    }

    /// Returns the line of each marker warning, checking it names `file`.
    fn marker_lines(file: &str, warnings: &[Error]) -> Vec<usize> {
        warnings
            .iter()
            .map(|warning| match warning {
                Error::InvalidSnippetMarker { path, line, .. } if path == file => *line,
                other => panic!("unexpected warning: {other}"),
            })
            .collect()
    }

    #[test]
    fn update_markdown_contents_warns_on_malformed_markers() -> Result<(), Error> {
        for (header, trailer, bad_trailer) in [
            ("semconv metric.test", "endsemconv", "endsemconvded"),
            ("weaver .registry.metrics", "endweaver", "endweaverded"),
        ] {
            let contents = format!(
                "<!-- {bad_trailer} -->\n<!--{header}-->\nold\n<!-- {bad_trailer} -->\n<!--{trailer}-->\n"
            );
            let (updated, warnings) = TestGenerator
                .update_markdown_contents("markers.md", &contents, None)
                .into_result_with_non_fatal()?;
            assert_eq!(
                updated,
                format!("<!-- {bad_trailer} -->\n<!--{header}-->\ngenerated\n<!--{trailer}-->\n")
            );
            assert_eq!(marker_lines("markers.md", &warnings), [1, 4]);
        }
        Ok(())
    }

    #[test]
    fn update_markdown_contents_valid_markers() {
        let contents = "<!--semconv metric.test-->\ngenerated\n<!--endsemconv-->\n<!--weaver .registry.metrics-->\ngenerated\n<!--endweaver-->\n<!-- other semconv stuff -->\n";
        match TestGenerator.update_markdown_contents("markers.md", contents, None) {
            WResult::Ok(updated) => assert_eq!(updated, contents),
            WResult::OkWithNFEs(_, warnings) => panic!("unexpected warnings: {warnings:?}"),
            WResult::FatalErr(error) => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn update_markdown_returns_marker_warnings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("markers.md");
        let file = path.to_str().expect("UTF-8 path");
        let current =
            "<!-- endweaverded -->\n<!--semconv metric.test-->\ngenerated\n<!--endsemconv-->\n";
        let stale = "<!-- endweaverded -->\n<!--semconv metric.test-->\nstale\n<!--endsemconv-->\n";

        // Marker warnings alone don't fail either mode.
        for dry_run in [true, false] {
            fs::write(&path, current).expect("write markdown");
            match TestGenerator.update_markdown(file, dry_run, None) {
                WResult::OkWithNFEs((), warnings) => assert_eq!(marker_lines(file, &warnings), [1]),
                WResult::Ok(()) => panic!("expected a marker warning"),
                WResult::FatalErr(error) => panic!("unexpected error: {error}"),
            }
            assert_eq!(fs::read_to_string(&path).expect("read markdown"), current);
        }

        // A dry run still fails on stale markdown and leaves the file alone.
        fs::write(&path, stale).expect("write markdown");
        match TestGenerator.update_markdown(file, true, None) {
            WResult::FatalErr(Error::MarkdownIsNotEqual { .. }) => {}
            WResult::FatalErr(error) => panic!("unexpected error: {error}"),
            _ => panic!("expected the dry run to fail"),
        }
        assert_eq!(fs::read_to_string(&path).expect("read markdown"), stale);

        // A normal run updates the file and keeps the warning.
        match TestGenerator.update_markdown(file, false, None) {
            WResult::OkWithNFEs((), warnings) => assert_eq!(marker_lines(file, &warnings), [1]),
            WResult::Ok(()) => panic!("expected a marker warning"),
            WResult::FatalErr(error) => panic!("unexpected error: {error}"),
        }
        assert_eq!(fs::read_to_string(&path).expect("read markdown"), current);
    }

    #[test]
    fn update_markdown_fatal_errors() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("markers.md");
        let file = path.to_str().expect("UTF-8 path");

        // A file that can't be read is an error.
        assert!(matches!(
            TestGenerator.update_markdown(file, true, None),
            WResult::FatalErr(Error::StdIoError(_))
        ));

        // So is a snippet that can't be generated, even next to a marker warning.
        for markdown in [
            "<!-- endweaverded -->\n<!--semconv metric.test-->\n<!--endsemconv-->\n",
            "<!-- endweaverded -->\n<!--weaver .registry.metrics-->\n<!--endweaver-->\n",
        ] {
            fs::write(&path, markdown).expect("write markdown");
            for dry_run in [true, false] {
                match FailingGenerator.update_markdown(file, dry_run, None) {
                    WResult::FatalErr(Error::GroupNotFound { .. })
                    | WResult::FatalErr(Error::WeaverSnippetNotSupported) => {}
                    WResult::FatalErr(error) => panic!("unexpected error: {error}"),
                    _ => panic!("expected the snippet to fail"),
                }
                assert_eq!(fs::read_to_string(&path).expect("read markdown"), markdown);
            }
        }
    }
}
