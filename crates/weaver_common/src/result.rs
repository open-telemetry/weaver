// SPDX-License-Identifier: Apache-2.0

//! Weaver Result type supporting both non-fatal errors (NFEs) and fatal errors.
//!
//! NFEs do not prevent the next operations from completing successfully. For example,
//! if a semconv file is invalid, we generate a non-fatal error and continue processing
//! the other files.
//!
//! NFEs in Weaver are standard Rust errors.

use crate::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use crate::error::WeaverError;
use miette::Diagnostic;
use serde::Serialize;
use std::error::Error;

/// Weaver Result type supporting both non-fatal errors (NFEs) and fatal errors.
#[must_use]
pub enum WResult<T, E> {
    /// The operation was successful, the result T is returned.
    Ok(T),
    /// The operation was successful, the result T is returned along with
    /// a list of non-fatal errors.
    OkWithNFEs(T, Vec<E>),
    /// The operation failed with a fatal error. By definition, we can only have
    /// one fatal error.
    FatalErr(E),
}

impl<T, E> WResult<T, E>
where
    E: WeaverError<E> + Error + Diagnostic + Serialize + Send + Sync + 'static,
{
    /// Converts a [`WResult`] into a standard [`Result`], optionally capturing non-fatal errors.
    pub fn capture_non_fatal_errors(
        self,
        non_fatal_errors: &mut Vec<DiagnosticMessage>,
    ) -> Result<T, E> {
        match self {
            WResult::Ok(result) => Ok(result),
            WResult::OkWithNFEs(result, nfes) => {
                for non_fatal_error in nfes {
                    non_fatal_errors.push(DiagnosticMessage::new(non_fatal_error));
                }
                Ok(result)
            }
            WResult::FatalErr(fatal_err) => Err(fatal_err),
        }
    }

    /// Capture the warnings into the provided vector and return a [`WResult`]
    /// without the warnings.
    pub fn capture_warnings(self, diag_msgs: &mut DiagnosticMessages) -> WResult<T, E> {
        if let WResult::OkWithNFEs(result, nfes) = self {
            let (warnings, errors): (Vec<_>, Vec<_>) = nfes
                .into_iter()
                .partition(|e| matches!(e.severity(), Some(miette::Severity::Warning)));
            let warnings: Vec<_> = warnings.into_iter().map(DiagnosticMessage::new).collect();
            diag_msgs.extend_from_vec(warnings);
            if errors.is_empty() {
                WResult::Ok(result)
            } else {
                WResult::OkWithNFEs(result, errors)
            }
        } else {
            self
        }
    }

    /// Return a [`WResult`] without the warnings.
    pub fn ignore_warnings(self) -> WResult<T, E> {
        match self {
            WResult::OkWithNFEs(result, non_fatal_errors) => {
                // Remove warnings from the non-fatal errors.
                let errors: Vec<_> = non_fatal_errors
                    .into_iter()
                    .filter(|e| !matches!(e.severity(), Some(miette::Severity::Warning)))
                    .collect();
                if errors.is_empty() {
                    WResult::Ok(result)
                } else {
                    WResult::OkWithNFEs(result, errors)
                }
            }
            _ => self,
        }
    }

    /// Calls a function with a reference to the contained value if [`Ok`].
    ///
    /// Returns the original result.
    pub fn inspect<F: FnOnce(&T, Option<&[E]>)>(self, f: F) -> Self {
        match &self {
            WResult::Ok(result) => f(result, None),
            WResult::OkWithNFEs(result, nfes) => f(result, Some(nfes)),
            WResult::FatalErr(_) => {}
        }

        self
    }

    /// Converts a [`WResult`] into a standard [`Result`], potentially
    /// aggregating non-fatal errors into a single error.
    pub fn into_result(self) -> Result<T, E> {
        match self {
            WResult::Ok(result) => Ok(result),
            WResult::OkWithNFEs(result, errors) => {
                if errors.is_empty() {
                    Ok(result)
                } else {
                    Err(E::compound(errors))
                }
            }
            WResult::FatalErr(e) => Err(e),
        }
    }

    /// Converts a [`WResult`] into a standard [`Result`], returning the result
    /// alongside any non-fatal errors.
    pub fn into_result_with_nfes(self) -> Result<(T, Vec<E>), E> {
        match self {
            WResult::Ok(result) => Ok((result, Vec::new())),
            WResult::OkWithNFEs(result, errors) => Ok((result, errors)),
            WResult::FatalErr(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use miette::Diagnostic;
    use serde::Serialize;
    use crate::diagnostic::DiagnosticMessages;
    use crate::error::WeaverError;
    use crate::result::WResult;

    #[derive(thiserror::Error,Debug,PartialEq,Serialize, Diagnostic)]
    enum TestError {
        #[error("Warning")]
        #[diagnostic(severity(Warning))]
        Warning,
        #[error("Error")]
        Error,
        #[error("Compound error")]
        CompoundError(Vec<TestError>),
    }

    impl WeaverError<TestError> for TestError {
        fn compound(errors: Vec<TestError>) -> Self {
            TestError::CompoundError(errors)
        }
    }

    #[test]
    pub fn test_werror_ok() {
        let mut diag_msgs = DiagnosticMessages::empty();
        let result: Result<i32, TestError> = WResult::Ok(42)
            .inspect(|r, _| assert_eq!(*r, 42))
            .capture_warnings(&mut diag_msgs)
            .into_result();

        assert_eq!(result, Ok(42));
        assert_eq!(diag_msgs.len(), 0);
    }

    #[test]
    pub fn test_non_fatal_errors() {
        let mut diag_msgs = DiagnosticMessages::empty();
        let result: Result<i32, TestError> = WResult::OkWithNFEs(42, vec![TestError::Warning, TestError::Error])
            .inspect(|r, nfes| {
                assert_eq!(*r, 42);
                assert_eq!(nfes.unwrap().len(), 2);
            })
            .capture_warnings(&mut diag_msgs)
            .into_result();

        assert_eq!(result, Err(TestError::CompoundError(vec![TestError::Error])));
        assert_eq!(diag_msgs.len(), 1);
    }
}
