// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use crate::error::Error::CompoundError;
use miette::Diagnostic;
use serde::Serialize;
use std::{path::PathBuf, str::FromStr};
use weaver_common::error::WeaverError;
use weaver_resolved_schema::attribute::AttributeRef;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Diagnostic, Serialize)]
#[non_exhaustive]
pub enum Error {
    /// Invalid config file.
    #[error("Invalid config file `{config_file}`: {error}")]
    InvalidConfigFile {
        /// Config file.
        config_file: PathBuf,
        /// Error message.
        error: String,
    },

    /// Target not found.
    #[error("Target `{target}` not found in `{root_path}`. Error: {error}")]
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

    /// Invalid template file.
    #[error("Invalid template file '{template}': {error}")]
    InvalidTemplateFile {
        /// Template path.
        template: PathBuf,
        /// Error message.
        error: String,
    },

    /// Error loading a file content from the file loader.
    #[error("Error loading the file '{file}': {error}")]
    FileLoaderError {
        /// File path.
        file: PathBuf,
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
    #[error("File name undefined in the template `{template}`. To resolve this, use the function `config(file_name = <file_name, filter, or expression>)` to set the file name.")]
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

    /// A generic container for multiple errors.
    #[error("Errors:\n{0:#?}")]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    /// Retrieves a list of error messages associated with this error.
    fn errors(&self) -> Vec<String> {
        match self {
            CompoundError(errors) => errors.iter().flat_map(|e| e.errors()).collect(),
            _ => vec![self.to_string()],
        }
    }
    fn compound(errors: Vec<Error>) -> Error {
        Self::compound_error(errors)
    }
}

#[must_use]
pub(crate) fn jinja_err_convert(e: minijinja::Error) -> Error {
    Error::WriteGeneratedCodeFailed {
        template: PathBuf::from_str(e.template_source().unwrap_or(""))
            .expect("Template source should be path"),
        error: format!("{}", e),
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
