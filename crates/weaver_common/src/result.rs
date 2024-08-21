// SPDX-License-Identifier: Apache-2.0

//! Weaver Result type supporting both non-fatal errors (NFEs) and fatal errors.
//!
//! NFEs do not prevent the next operations from completing successfully.
//! NFEs in Weaver are standard Rust errors.

use crate::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use crate::error::WeaverError;
use miette::Diagnostic;
use serde::Serialize;
use std::error::Error;

/// Weaver Result type supporting both non-fatal errors (NFEs) and fatal errors.
#[must_use]
pub enum WResult<T, E> {
    /// The operation was successful, the result T is returned along with
    /// non-fatal errors.
    Ok(T, Vec<E>),
    /// The operation failed with a fatal errors.
    Err(E),
}

impl<T, E> WResult<T, E>
where
    E: WeaverError<E> + Error + Diagnostic + Serialize + Send + Sync + 'static,
{
    /// Create a new [`WResult`] with the given result and non-fatal errors.
    pub fn with_non_fatal_errors(result: T, non_fatal_errors: Vec<E>) -> Self {
        WResult::Ok(result, non_fatal_errors)
    }

    /// Create a new [`WResult`] with a given fatal error.
    pub fn with_fatal_error(error: E) -> Self {
        WResult::Err(error)
    }

    /// Converts a `[WResult]` into a standard `[Result]`, optionally capturing non-fatal errors.
    pub fn capture_non_fatal_errors(
        self,
        non_fatal_errors: &mut Vec<DiagnosticMessage>,
    ) -> Result<T, E> {
        match self {
            WResult::Ok(result, nfes) => {
                for non_fatal_error in nfes {
                    non_fatal_errors.push(DiagnosticMessage::new(non_fatal_error));
                }
                Ok(result)
            }
            WResult::Err(fatal_err) => Err(fatal_err),
        }
    }

    /// Capture the warnings into the provided vector and return a `[WResult]`
    /// without the warnings.
    pub fn capture_warnings(self, diag_msgs: &mut DiagnosticMessages) -> WResult<T, E> {
        if let WResult::Ok(result, nfes) = self {
            let (warnings, errors): (Vec<_>, Vec<_>) = nfes
                .into_iter()
                .partition(|e| matches!(e.severity(), Some(miette::Severity::Warning)));
            let warnings: Vec<_> = warnings.into_iter().map(DiagnosticMessage::new).collect();
            diag_msgs.extend_from_vec(warnings);
            WResult::Ok(result, errors)
        } else {
            self
        }
    }

    /// Return a [`WResult`] without the warnings.
    pub fn ignore_warnings(self) -> WResult<T, E> {
        match self {
            WResult::Ok(result, non_fatal_errors) => {
                // Remove warnings from the non-fatal errors.
                let errors = non_fatal_errors
                    .into_iter()
                    .filter(|e| !matches!(e.severity(), Some(miette::Severity::Warning)))
                    .collect();
                WResult::Ok(result, errors)
            }
            WResult::Err(e) => WResult::Err(e),
        }
    }

    /// Calls a function with a reference to the contained value if [`Ok`].
    ///
    /// Returns the original result.
    pub fn inspect<F: FnOnce(&T, &[E])>(self, f: F) -> Self {
        if let WResult::Ok(ref result, ref nfes) = self {
            f(result, nfes);
        }

        self
    }

    /// Converts a `[WResult]` into a standard `[Result]`, potentially
    /// aggregating non-fatal errors into a single error.
    pub fn into_result(self) -> Result<T, E> {
        match self {
            WResult::Ok(result, errors) => {
                if errors.is_empty() {
                    Ok(result)
                } else {
                    Err(E::compound(errors))
                }
            }
            WResult::Err(e) => Err(e),
        }
    }

    /// Converts a `[WResult]` into a standard `[Result]`, returning the result
    /// alongside any non-fatal errors.
    pub fn into_result_with_nfes(self) -> Result<(T, Vec<E>), E> {
        match self {
            WResult::Ok(result, errors) => Ok((result, errors)),
            WResult::Err(e) => Err(e),
        }
    }
}
