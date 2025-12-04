// SPDX-License-Identifier: Apache-2.0

//! This crate will generate code for markdown files.
//! The entire crate is a rush job to catch feature parity w/ existing python tooling by
//! poorly porting the code into RUST.  We expect to optimise and improve things over time.

use miette::Diagnostic;
use serde::Serialize;
use std::{fmt, fs};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_diff::diff_output;

mod parser;
mod v1;
mod v2;

/// SnippetGenerator for v1 resolution.
pub use v1::SnippetGenerator;
pub use v2::SnippetGenerator as SnipperGeneratorV2;

use crate::parser::GenerateMarkdownArgs;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
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
    #[error("{:?}", format_errors(.0))]
    CompoundError(Vec<Error>),
}

/// A trait denoting how to update the contents of a markdown using `semconv` snippet headers.
pub trait MarkdownSnippetGenerator {
    /// Update the contents of a markdown file.
    ///
    /// file: The markdown file.
    /// dry_run: Whether to actually change the file or just report if changes would be made.
    /// attribute_registry_base_url: Legacy mechanism to pass parameters to the snippet templates.
    /// Returns: empty, or an error if one occurred.
    fn update_markdown(
        &self,
        file: &str,
        dry_run: bool,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<(), Error> {
        let original_markdown = fs::read_to_string(file)
            .map_err(|e| Error::StdIoError(e.to_string()))?
            .replace("\r\n", "\n");
        let updated_markdown =
            self.update_markdown_contents(&original_markdown, attribute_registry_base_url)?;
        if !dry_run {
            fs::write(file, updated_markdown).map_err(|e| Error::StdIoError(e.to_string()))?;
            Ok(())
        } else if original_markdown != updated_markdown {
            Err(Error::MarkdownIsNotEqual {
                original: original_markdown,
                updated: updated_markdown,
            })
        } else {
            Ok(())
        }
    }

    /// Update the contents of a markdown string.
    ///
    /// contents: The contents of the markdown.
    /// attribute_registry_base_url: Legacy mechanism to pass parameters to the snippet templates.
    /// Returns: the updated markdown or an error.
    fn update_markdown_contents(
        &self,
        contents: &str,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error> {
        let mut result = String::new();
        let mut handling_snippet = false;
        for line in contents.lines() {
            if handling_snippet {
                if parser::is_semconv_trailer(line) {
                    result.push_str(line);
                    // TODO - do we always need this or did we trim oddly?
                    result.push('\n');
                    handling_snippet = false;
                }
            } else {
                // Always push this line.
                result.push_str(line);
                // TODO - don't do this on last line.
                result.push('\n');
                // Check to see if line matches snippet request.
                // If so, generate the snippet and continue.
                if parser::is_markdown_snippet_directive(line) {
                    handling_snippet = true;
                    let arg = parser::parse_markdown_snippet_directive(line)?;
                    let snippet =
                        self.generate_markdown_snippet(arg, attribute_registry_base_url)?;
                    result.push_str(&snippet);
                }
            }
        }
        Ok(result)
    }

    /// Generates a markdown snipper for a given parsed argument.
    // TODO - move registry base url into state of the struct...
    fn generate_markdown_snippet(
        &self,
        args: GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error>;
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
