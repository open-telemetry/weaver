// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use std::{path::PathBuf, str::FromStr};

use miette::Diagnostic;
use serde::Serialize;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};

use weaver_common::error::WeaverError;
use weaver_resolved_schema::attribute::AttributeRef;

use crate::error::Error::CompoundError;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Diagnostic, Serialize)]
#[non_exhaustive]
pub enum Error {
    /// Invalid config file.
    #[error("Invalid config file `{config_file}`: {error}")]
    #[diagnostic(
        help("Please check the syntax of the weaver.yaml file."),
        url("https://github.com/open-telemetry/weaver/blob/main/docs/weaver-config.md")
    )]
    InvalidConfigFile {
        /// Config file.
        config_file: PathBuf,
        /// Error message.
        error: String,
    },

    /// Target not found.
    #[error("Target `{target}` not found in `{root_path}`. Error: {error}")]
    #[diagnostic(
        help("Please check the subdirectories of the template path for the target."),
        url("https://github.com/open-telemetry/weaver/blob/main/crates/weaver_forge/README.md")
    )]
    TargetNotSupported {
        /// Root path.
        root_path: String,
        /// Target name.
        target: String,
        /// Error message.
        error: String,
    },

    /// Invalid template directory.
    #[error("Invalid template directory {template_dir}: {error}")]
    InvalidTemplateDir {
        /// Template directory.
        template_dir: PathBuf,
        /// Error message.
        error: String,
    },

    /// Invalid telemetry schema.
    #[error("Invalid telemetry schema {schema}: {error}")]
    InvalidTelemetrySchema {
        /// Schema file.
        schema: PathBuf,
        /// Error message.
        error: String,
    },

    /// Invalid file path.
    #[error("A `file_path` must be a valid Jinja expression (file_path: '{file_path}'): {error}")]
    InvalidFilePath {
        /// File path.
        file_path: String,
        /// Error message.
        error: String,
    },

    /// Invalid template file.
    #[error("Invalid template file '{template}': {error}")]
    InvalidTemplateFile {
        /// Template path.
        template: PathBuf,
        /// Error message.
        error: String,
    },

    /// Invalid code snippet.
    #[error("Invalid {mode} code snippet: {error}")]
    InvalidCodeSnippet {
        /// Comment format.
        format: String,
        /// Snippet mode.
        mode: String,
        /// Error message.
        error: String,
    },

    /// Comment format not found in the configuration.
    #[error(
        "Comment format `{format}` not found in the configuration. Available formats: {formats:?}"
    )]
    CommentFormatNotFound {
        /// Comment format.
        format: String,
        /// Available formats.
        formats: Vec<String>,
    },

    /// Error loading a file content from the file loader.
    #[error("Error loading the file '{file}': {error}")]
    FileLoaderError {
        /// File path.
        file: PathBuf,
        /// Error message.
        error: String,
    },

    /// Template engine error.
    #[error("Template engine error -> {error}")]
    TemplateEngineError {
        /// Error message.
        error: String,
    },

    /// Template evaluation failed.
    #[error("Template evaluation error -> {error}")]
    TemplateEvaluationFailed {
        /// Template path.
        template: PathBuf,
        /// Error id used to deduplicate the error.
        error_id: String,
        /// Error message.
        error: String,
    },

    /// Invalid template directory.
    #[error("Invalid template directory: {0}")]
    InvalidTemplateDirectory(PathBuf),

    /// Template file name undefined.
    #[error("File name undefined in the template `{template}`. To resolve this, use the function `config(file_name = <file_name, filter, or expression>)` to set the file name."
    )]
    TemplateFileNameUndefined {
        /// Template path.
        template: PathBuf,
    },

    /// Write generated code failed.
    #[error("Writing of the generated code {template} failed: {error}")]
    WriteGeneratedCodeFailed {
        /// Template path.
        template: PathBuf,
        /// Error message.
        error: String,
    },

    /// Attribute reference not found in the catalog.
    #[error("Attribute reference {attr_ref} (group: {group_id}) not found in the catalog")]
    AttributeNotFound {
        /// Group id.
        group_id: String,
        /// Attribute reference.
        attr_ref: AttributeRef,
    },

    /// Filter error.
    #[error("Filter '{filter}' failed: {error}")]
    FilterError {
        /// Filter that caused the error.
        filter: String,
        /// Error message.
        error: String,
    },

    /// Import JQ package error.
    #[error("Import JQ package '{package}' failed: {error}")]
    ImportError {
        /// Package that caused the error.
        package: String,
        /// Error message.
        error: String,
    },

    /// Invalid template pattern.
    #[error("Invalid template pattern: {error}")]
    InvalidTemplatePattern {
        /// Error message.
        error: String,
    },

    /// The serialization of the context failed.
    #[error("The serialization of the context failed: {error}")]
    ContextSerializationFailed {
        /// Error message.
        error: String,
    },

    /// Duplicate parameter key.
    #[error("Duplicate parameter key '{key}': {error}")]
    DuplicateParamKey {
        /// The duplicate key.
        key: String,
        /// Error message.
        error: String,
    },

    /// Invalid case convention.
    #[error("`{case}` is not a valid case convention. Valid case conventions are: lower_case, upper_case, title_case, snake_case, kebab_case, camel_case, pascal_case, screaming_snake_case, and screaming_kebab_case.")]
    InvalidCaseConvention {
        /// The invalid case
        case: String,
    },

    /// Invalid Markdown text.
    #[error("Invalid Markdown content: {error}")]
    InvalidMarkdown {
        /// Error message.
        error: String,
    },

    /// Failed to serialize data.
    #[error("Serialization failed: {error}")]
    SerializationError {
        /// Error message.
        error: String,
    },

    /// Output file operation failed.
    #[error("Output file `{path}` failed: {error}")]
    OutputFileError {
        /// File path.
        path: PathBuf,
        /// Error message.
        error: String,
    },

    /// A generic container for multiple errors.
    #[error("Errors:\n{0:#?}")]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::compound_error(errors)
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

impl From<std::fmt::Error> for Error {
    fn from(_: std::fmt::Error) -> Self {
        Self::TemplateEngineError {
            error: "Unexpected string formatting error".to_owned(),
        }
    }
}

#[must_use]
pub(crate) fn jinja_err_convert(e: minijinja::Error) -> Error {
    Error::WriteGeneratedCodeFailed {
        template: PathBuf::from_str(e.template_source().unwrap_or(""))
            .expect("Template source should be path"),
        error: format!("{e}"),
    }
}

impl Error {
    /// Creates a compound error from a list of errors.
    /// Note: All compound errors are flattened.
    #[must_use]
    pub fn compound_error(errors: Vec<Self>) -> Self {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}
