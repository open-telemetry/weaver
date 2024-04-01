// SPDX-License-Identifier: Apache-2.0

//! Template engine based on Tera.
//! This crate is deprecated and will be removed in the future once the new
//! template engine based on MiniJinja is used in all the code generators.

use std::path::PathBuf;

mod config;
mod filters;
mod functions;
pub mod sdkgen;
mod testers;

/// An error that can occur while generating a client SDK.
#[derive(thiserror::Error, Debug)]
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

    /// Language not found.
    #[error(
        "Language `{0}` is not supported. Use the command `languages` to list supported languages."
    )]
    LanguageNotSupported(String),

    /// Invalid template directory.
    #[error("Invalid template directory: {0}")]
    InvalidTemplateDirectory(PathBuf),

    /// Invalid template file.
    #[error("Invalid template file: {0}")]
    InvalidTemplateFile(PathBuf),

    /// Invalid template.
    #[error("{error}")]
    InvalidTemplate {
        /// Template directory.
        template: PathBuf,
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

    /// Write generated code failed.
    #[error("Writing of the generated code {template} failed: {error}")]
    WriteGeneratedCodeFailed {
        /// Template path.
        template: PathBuf,
        /// Error message.
        error: String,
    },

    /// Internal error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Template file name undefined.
    #[error("File name undefined in the template `{template}`. To resolve this, use the function `config(file_name = <file_name, filter, or expression>)` to set the file name.")]
    TemplateFileNameUndefined {
        /// Template path.
        template: PathBuf,
    },
}

/// General configuration for the generator.
pub struct GeneratorConfig {
    template_dir: PathBuf,
}

impl Default for GeneratorConfig {
    /// Create a new generator configuration with default values.
    fn default() -> Self {
        Self {
            template_dir: PathBuf::from("templates"),
        }
    }
}
