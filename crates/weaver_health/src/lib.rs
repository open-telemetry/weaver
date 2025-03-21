// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver-health library
use miette::Diagnostic;
use serde::Serialize;
use weaver_common::{
    diagnostic::{DiagnosticMessage, DiagnosticMessages},
    Logger,
};

/// Advisors for health checks
pub mod attribute_advice;
/// An ingester that reads attribute names from a text file.
pub mod attribute_file_ingester;
/// Attribute health checker
pub mod attribute_health;
/// An ingester that reads attribute names and values from a JSON file.
pub mod attribute_json_file_ingester;
/// An ingester that reads attribute names and values from standard input.
pub mod attribute_json_stdin_ingester;
/// An ingester that reads attribute names from standard input.
pub mod attribute_stdin_ingester;
/// The intermediary format
pub mod sample;

/// Weaver health errors
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Ingest error.
    #[error("Fatal error during ingest. {error}")]
    IngestError {
        /// The error that occurred.
        error: String,
    },
    /// Advice error.
    #[error("Fatal error from Advisor. {error}")]
    AdviceError {
        /// The error that occurred.
        error: String,
    },
    /// Output error.
    #[error("Output error. {error}")]
    OutputError {
        /// The error that occurred.
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Ingesters implement a generic trait that specifies both their input and output types
pub trait Ingester<I, O> {
    /// Ingest data from the input type and return the output type
    fn ingest(&self, input: I, logger: impl Logger + Sync + Clone) -> Result<O, Error>;
}
