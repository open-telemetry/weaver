// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use miette::Diagnostic;
use serde::Serialize;
use weaver_common::{
    diagnostic::{DiagnosticMessage, DiagnosticMessages},
    Logger,
};

/// Advisors for live checks
pub mod attribute_advice;
/// An ingester that reads attribute names from a text file.
pub mod attribute_file_ingester;
/// An ingester that reads attribute names and values from a JSON file.
pub mod attribute_json_file_ingester;
/// An ingester that reads attribute names and values from standard input.
pub mod attribute_json_stdin_ingester;
/// Attribute live checker
pub mod attribute_live_check;
/// An ingester that reads attribute names from standard input.
pub mod attribute_stdin_ingester;
/// The intermediary format
pub mod sample;

/// Weaver live check errors
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Generic ingest error.
    #[error("Fatal error during ingest. {error}")]
    IngestError {
        /// The error that occurred.
        error: String,
    },

    /// Attempt to Ingest an empty line.
    #[error("Attempt to ingest an empty line.")]
    IngestEmptyLine,

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

/// Ingesters implement a generic trait that returns an iterator
pub trait Ingester<T> {
    /// Ingest data and return an iterator of the output type
    fn ingest(
        &self,
        logger: impl Logger + Sync + Clone + 'static,
    ) -> Result<Box<dyn Iterator<Item = T>>, Error>;
}
