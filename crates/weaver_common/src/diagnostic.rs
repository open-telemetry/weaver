// SPDX-License-Identifier: Apache-2.0

//! A generic diagnostic message

use crate::Logger;
use miette::{Diagnostic, LabeledSpan, MietteDiagnostic, Report, Severity};
use serde::Serialize;
use std::error::Error;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;

/// A flag to globally enable future mode for diagnostics.
/// When enabled, all the warning messages will be treated as errors.
static FUTURE_MODE: AtomicBool = AtomicBool::new(false);

/// Enable future mode for diagnostics.
/// When enabled, all the warning messages will be treated as errors.
pub fn enable_future_mode() {
    FUTURE_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Disable future mode for diagnostics.
/// When disabled, all the warning messages will be treated as warnings.
pub fn disable_future_mode() {
    FUTURE_MODE.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// Returns true if future mode is enabled for diagnostics.
pub fn is_future_mode_enabled() -> bool {
    FUTURE_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

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
#[derive(Debug, serde::Serialize, Clone)]
pub struct DiagnosticMessage {
    /// The error
    pub(crate) error: serde_json::Value,
    /// The diagnostic message
    pub(crate) diagnostic: MietteDiagnosticExt,
}

/// A list of diagnostic messages
#[derive(Debug, serde::Serialize, Clone)]
#[serde(transparent)]
pub struct DiagnosticMessages(Vec<DiagnosticMessage>);

impl From<DiagnosticMessage> for DiagnosticMessages {
    fn from(value: DiagnosticMessage) -> Self {
        Self(vec![value])
    }
}

impl DiagnosticMessage {
    /// Creates a new diagnostic message from an error
    pub fn new<M: Error + Diagnostic + Serialize + Send + Sync + 'static>(error: M) -> Self {
        let json_error = serde_json::to_value(&error).expect("Failed to serialize error");
        let message = error.to_string();
        let code = error.code().map(|error_code| error_code.to_string());
        let mut severity = error.severity();
        let help = error.help().map(|help| help.to_string());
        let url = error.url().map(|url| url.to_string());
        let labels = error.labels().map(|labels| labels.collect());
        let ansi_message = format!(
            "{:?}",
            if is_future_mode_enabled() {
                severity = Some(Severity::Error);
                Report::new(MietteDiagnostic {
                    message: message.clone(),
                    code: code.clone(),
                    severity,
                    help: help.clone(),
                    url: url.clone(),
                    labels: labels.clone(),
                })
            } else {
                Report::new(error)
            }
        );

        let diagnostic = MietteDiagnosticExt {
            message,
            ansi_message,
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

    /// Returns true if the diagnostic message is a warning
    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.diagnostic.severity == Some(Severity::Warning)
    }
}

impl DiagnosticMessages {
    /// Creates a new list of diagnostic messages
    #[must_use]
    pub fn new(diag_msgs: Vec<DiagnosticMessage>) -> Self {
        Self(diag_msgs)
    }

    /// Creates an empty list of diagnostic messages
    #[must_use]
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    /// Extends the current `DiagnosticMessages` with the provided
    /// `DiagnosticMessages`.
    pub fn extend(&mut self, diag_msgs: DiagnosticMessages) {
        self.0.extend(diag_msgs.0);
    }

    /// Extends the current `DiagnosticMessages` with the provided
    /// `Vec<DiagnosticMessage>`.
    pub fn extend_from_vec(&mut self, diag_msgs: Vec<DiagnosticMessage>) {
        self.0.extend(diag_msgs);
    }

    /// Logs all the diagnostic messages
    pub fn log(&self, logger: impl Logger) {
        self.0
            .iter()
            .for_each(|msg| logger.error(&msg.diagnostic.message));
    }

    /// Returns the number of diagnostic messages
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the diagnostic messages
    #[must_use]
    pub fn into_inner(self) -> Vec<DiagnosticMessage> {
        self.0
    }

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

    /// Returns true if at least one diagnostic message has an error severity.
    #[must_use]
    pub fn has_error(&self) -> bool {
        let non_error_count = self
            .0
            .iter()
            .filter(|message| {
                message.diagnostic.severity == Some(Severity::Warning)
                    || message.diagnostic.severity == Some(Severity::Advice)
            })
            .count();
        self.0.len() - non_error_count > 0
    }

    /// Returns true if there are no diagnostic messages
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An extension trait for `Result` that captures the diagnostic messages
pub trait ResultExt<T, E> {
    /// Captures the diagnostic messages into the provided `DiagnosticMessages`
    /// or returns the value if there are no diagnostic messages.
    fn capture_diag_msgs_into(self, diags: &mut DiagnosticMessages) -> Option<T>;

    /// Combines the diagnostic messages with the provided `DiagnosticMessages`
    /// and returns the `ok` value or the combined `DiagnosticMessages`.
    fn combine_diag_msgs_with(self, diags: &DiagnosticMessages) -> Result<T, DiagnosticMessages>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: Into<DiagnosticMessages>,
{
    fn capture_diag_msgs_into(self, diags: &mut DiagnosticMessages) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(diag_msgs) => {
                diags.extend(diag_msgs.into());
                None
            }
        }
    }

    fn combine_diag_msgs_with(self, diags: &DiagnosticMessages) -> Result<T, DiagnosticMessages> {
        match self {
            Ok(v) => Ok(v),
            Err(errs) => {
                let mut diag_msgs: DiagnosticMessages = errs.into();
                diag_msgs.extend(diags.clone());
                Err(diag_msgs)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miette::Diagnostic;

    #[derive(thiserror::Error, Debug, Clone, Diagnostic, Serialize)]
    #[error("This is a test error")]
    #[diagnostic(code(test::error))]
    #[diagnostic(url = "https://example.com")]
    #[diagnostic(help = "This is a test error")]
    #[diagnostic(severity = "error")]
    struct TestError {
        message: String,
    }

    #[test]
    fn test_diagnostic_message() {
        let error = TestError {
            message: "This is a test error".to_owned(),
        };
        let diagnostic_message = DiagnosticMessage::new(error);
        assert_eq!(
            diagnostic_message.diagnostic.message,
            "This is a test error"
        );
        assert_eq!(
            diagnostic_message.diagnostic.code,
            Some("test::error".to_owned())
        );
        assert_eq!(
            diagnostic_message.diagnostic.severity,
            Some(Severity::Error)
        );
        assert_eq!(
            diagnostic_message.diagnostic.help,
            Some("This is a test error".to_owned())
        );
        assert_eq!(
            diagnostic_message.diagnostic.url,
            Some("https://example.com".to_owned())
        );
    }

    #[test]
    fn test_diagnostic_messages() {
        let error = TestError {
            message: "This is a test error".to_owned(),
        };
        let diagnostic_messages = DiagnosticMessages::from_error(error.clone());
        assert_eq!(diagnostic_messages.0.len(), 1);
        assert!(diagnostic_messages.has_error());
        assert!(!diagnostic_messages.is_empty());
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.message,
            "This is a test error"
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.code,
            Some("test::error".to_owned())
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.severity,
            Some(Severity::Error)
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.help,
            Some("This is a test error".to_owned())
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.url,
            Some("https://example.com".to_owned())
        );

        let diagnostic_messages = DiagnosticMessages::from_errors(vec![error]);
        assert_eq!(diagnostic_messages.0.len(), 1);
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.message,
            "This is a test error"
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.code,
            Some("test::error".to_owned())
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.severity,
            Some(Severity::Error)
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.help,
            Some("This is a test error".to_owned())
        );
        assert_eq!(
            diagnostic_messages.0[0].diagnostic.url,
            Some("https://example.com".to_owned())
        );
    }
}
