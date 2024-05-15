// SPDX-License-Identifier: Apache-2.0

//! A generic diagnostic message

use crate::error::WeaverDiagnostic;
use miette::{Diagnostic, LabeledSpan, Report, Severity};
use serde::Serialize;
use std::error::Error;

/// An extension to the [`miette::Diagnostic`] struct that adds an ansi message
/// representation of the diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MietteDiagnosticExt {
    /// Displayed diagnostic message
    pub message: String,
    /// Displayed diagnostic message with ansi color codes
    pub ansi_message: String,
    /// Unique diagnostic code to look up more information
    /// about this Diagnostic. Ideally also globally unique, and documented
    /// in the toplevel crate's documentation for easy searching.
    /// Rust path format (`foo::bar::baz`) is recommended, but more classic
    /// codes like `E0123` will work just fine
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// [`Diagnostic`] severity. Intended to be used by
    /// [`ReportHandler`](crate::ReportHandler)s to change the way different
    /// [`Diagnostic`]s are displayed. Defaults to [`Severity::Error`]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// Additional help text related to this Diagnostic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// URL to visit for a more detailed explanation/help about this
    /// [`Diagnostic`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Labels to apply to this `Diagnostic`'s [`Diagnostic::source_code`]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<LabeledSpan>>,
}

/// A generic and serializable representation of a diagnostic message
#[derive(Debug, serde::Serialize)]
pub struct DiagnosticMessage {
    /// The error
    error: serde_json::Value,
    /// The diagnostic message
    diagnostic: MietteDiagnosticExt,
}

/// A list of diagnostic messages
#[derive(Debug, serde::Serialize)]
#[serde(transparent)]
pub struct DiagnosticMessages(Vec<DiagnosticMessage>);

impl DiagnosticMessage {
    /// Creates a new diagnostic message from an error
    pub fn new<M: Error + Diagnostic + Serialize + Send + Sync + 'static>(error: M) -> Self {
        let json_error = serde_json::to_value(&error).expect("Failed to serialize error");
        let message = error.to_string();
        let code = error.code().map(|error_code| error_code.to_string());
        let severity = error.severity();
        let help = error.help().map(|help| help.to_string());
        let url = error.url().map(|url| url.to_string());
        let labels = error.labels().map(|labels| labels.collect());

        let diagnostic = MietteDiagnosticExt {
            message,
            ansi_message: format!("{:?}", Report::new(error)),
            code,
            severity,
            help,
            url,
            labels,
        };
        Self {
            error: json_error,
            diagnostic,
        }
    }
}

impl DiagnosticMessages {
    /// Creates a new list of diagnostic messages for a list of errors
    pub fn from_errors<M: Error + Diagnostic + Serialize + Send + Sync + 'static>(
        errors: Vec<M>,
    ) -> Self {
        Self(errors.into_iter().map(DiagnosticMessage::new).collect())
    }

    /// Creates a new list of diagnostic messages from a single error
    pub fn from_error<M: Error + Diagnostic + Serialize + Send + Sync + 'static>(error: M) -> Self {
        Self(vec![DiagnosticMessage::new(error)])
    }
}

impl<T: WeaverDiagnostic + Diagnostic + Serialize + Send + Sync + 'static> From<T>
    for DiagnosticMessages
{
    /// Convert errors marked with the DiagnosticMessage trait into a DiagnosticMessages.
    fn from(error: T) -> Self {
        Self(vec![DiagnosticMessage::new(error)])
    }
}
