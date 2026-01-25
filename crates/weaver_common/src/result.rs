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

impl<T> WResult<T, DiagnosticMessage> {
    /// Converts a [`WResult`] into a standard [`Result`], optionally capturing non-fatal errors.
    #[allow(clippy::result_large_err)]
    pub fn capture_non_fatal_errors(
        self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<T, DiagnosticMessage> {
        match self {
            WResult::Ok(result) => Ok(result),
            WResult::OkWithNFEs(result, nfes) => {
                diag_msgs.extend_from_vec(nfes);
                Ok(result)
            }
            WResult::FatalErr(fatal_err) => Err(fatal_err),
        }
    }
}

impl<T, E> WResult<T, E>
where
    E: WeaverError<E> + Error + Diagnostic + Serialize + Send + Sync + 'static,
{
    /// Returns `true` if the result is a fatal error.
    pub fn is_fatal(&self) -> bool {
        matches!(self, WResult::FatalErr(_))
    }

    /// Returns `true` if the result is not Ok.
    pub fn has_errors(&self) -> bool {
        match self {
            WResult::Ok(_) => false,
            WResult::OkWithNFEs(_, errors) => !errors.is_empty(),
            WResult::FatalErr(_) => true,
        }
    }

    /// Returns the number of non-fatal errors, or 1 if the result is a fatal error, 0 otherwise.
    pub fn num_errors(&self) -> usize {
        match self {
            WResult::Ok(_) => 0,
            WResult::OkWithNFEs(_, errors) => errors.len(),
            WResult::FatalErr(_) => 1,
        }
    }

    /// Extends a WResult with additional non-fatal errors.
    ///
    /// If the result was `Ok``, this moves to an `OkWithNFEs`.
    /// If the result was a `FatalError` this method is ignored.
    pub fn extend_non_fatal_errors(self, non_fatal_errors: Vec<E>) -> Self {
        match self {
            WResult::Ok(result) => WResult::OkWithNFEs(result, non_fatal_errors),
            WResult::OkWithNFEs(result, mut items) => {
                items.extend(non_fatal_errors);
                WResult::OkWithNFEs(result, items)
            }
            WResult::FatalErr(e) => Self::FatalErr(e),
        }
    }

    /// Creates a new [`WResult`] with a successful result.
    pub fn with_non_fatal_errors(result: T, non_fatal_errors: Vec<E>) -> Self {
        if non_fatal_errors.is_empty() {
            WResult::Ok(result)
        } else {
            WResult::OkWithNFEs(result, non_fatal_errors)
        }
    }

    /// Converts a [`WResult`] into a standard [`Result`], optionally capturing non-fatal errors.
    pub fn capture_non_fatal_errors(self, diag_msgs: &mut DiagnosticMessages) -> Result<T, E> {
        match self {
            WResult::Ok(result) => Ok(result),
            WResult::OkWithNFEs(result, nfes) => {
                let msgs = nfes.into_iter().map(DiagnosticMessage::new).collect();
                diag_msgs.extend_from_vec(msgs);
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

    /// Return a [`WResult`] without the non-fatal errors with severity=warning.
    pub fn ignore<F>(self, ignore: F) -> WResult<T, E>
    where
        F: Fn(&E) -> bool,
    {
        match self {
            WResult::OkWithNFEs(result, non_fatal_errors) => {
                // Remove warnings from the non-fatal errors.
                let errors: Vec<_> = non_fatal_errors
                    .into_iter()
                    .filter(|e| !ignore(e))
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
    pub fn into_result_failing_non_fatal(self) -> Result<T, E> {
        match self {
            WResult::Ok(result) => Ok(result),
            WResult::OkWithNFEs(result, errors) => {
                if errors.is_empty() {
                    Ok(result)
                } else if errors.len() == 1 {
                    Err(errors
                        .into_iter()
                        .next()
                        .expect("should never happen as we checked the length"))
                } else {
                    let compound_error = E::compound(errors);
                    Err(compound_error)
                }
            }
            WResult::FatalErr(e) => Err(e),
        }
    }

    /// Converts a [`WResult`] into a standard [`Result`], returning the result
    /// alongside any non-fatal errors.
    pub fn into_result_with_non_fatal(self) -> Result<(T, Vec<E>), E> {
        match self {
            WResult::Ok(result) => Ok((result, Vec::new())),
            WResult::OkWithNFEs(result, errors) => Ok((result, errors)),
            WResult::FatalErr(e) => Err(e),
        }
    }

    /// Maps a [`WResult<T, E>`] to [`WResult<U, E>`] by applying a function to a
    /// contained [`Ok`] value, leaving an [`Err`] value untouched.
    pub fn map<U, F: FnOnce(T) -> U>(self, op: F) -> WResult<U, E> {
        match self {
            WResult::Ok(t) => WResult::Ok(op(t)),
            WResult::OkWithNFEs(t, nfes) => WResult::OkWithNFEs(op(t), nfes),
            WResult::FatalErr(err) => WResult::FatalErr(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::DiagnosticMessages;
    use crate::error::WeaverError;
    use crate::result::WResult;
    use miette::Diagnostic;
    use serde::Serialize;

    #[derive(thiserror::Error, Debug, PartialEq, Serialize, Diagnostic, Clone)]
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
    fn test_extend_nfes() -> Result<(), TestError> {
        let warnings = vec![TestError::Warning];
        let result: WResult<i32, TestError> =
            WResult::Ok(42).extend_non_fatal_errors(warnings.clone());
        match result {
            WResult::OkWithNFEs(value, nfes) => {
                assert_eq!(value, 42);
                assert_eq!(nfes, warnings);
            }
            _ => panic!("Failed to add warning to Ok"),
        }
        let result2: WResult<i32, TestError> = WResult::OkWithNFEs(42, vec![TestError::Warning])
            .extend_non_fatal_errors(warnings.clone());
        match result2 {
            WResult::OkWithNFEs(value, nfes) => {
                assert_eq!(value, 42);
                assert_eq!(nfes, vec![TestError::Warning, TestError::Warning]);
            }
            _ => panic!("Failed to add warning to OkWithNFEs"),
        }
        Ok(())
    }

    #[test]
    fn test_werror() -> Result<(), TestError> {
        let mut diag_msgs = DiagnosticMessages::empty();
        let result: Result<i32, TestError> = WResult::Ok(42)
            .inspect(|r, _| assert_eq!(*r, 42))
            .capture_warnings(&mut diag_msgs)
            .into_result_failing_non_fatal();

        assert_eq!(result, Ok(42));
        assert_eq!(diag_msgs.len(), 0);

        let mut diag_msgs = DiagnosticMessages::empty();
        let result: Result<i32, TestError> = WResult::Ok(42)
            .inspect(|r, _| assert_eq!(*r, 42))
            .capture_warnings(&mut diag_msgs)
            .into_result_failing_non_fatal();

        assert_eq!(result, Ok(42));
        assert_eq!(diag_msgs.len(), 0);

        let mut diag_msgs = DiagnosticMessages::empty();
        let result: Result<i32, TestError> =
            WResult::OkWithNFEs(42, vec![TestError::Warning, TestError::Error])
                .inspect(|r, nfes| {
                    assert_eq!(*r, 42);
                    assert_eq!(nfes.unwrap().len(), 2);
                })
                .capture_warnings(&mut diag_msgs)
                .into_result_failing_non_fatal();

        assert_eq!(result, Err(TestError::Error));
        assert_eq!(diag_msgs.len(), 1);

        let mut diag_msgs = DiagnosticMessages::empty();
        let result = WResult::OkWithNFEs(42, vec![TestError::Warning, TestError::Error])
            .inspect(|r, nfes| {
                assert_eq!(*r, 42);
                assert_eq!(nfes.unwrap().len(), 2);
            })
            .capture_non_fatal_errors(&mut diag_msgs)?;

        assert_eq!(result, 42);
        assert_eq!(diag_msgs.len(), 2);

        let (result, nfes) = WResult::OkWithNFEs(42, vec![TestError::Warning, TestError::Error])
            .inspect(|r, nfes| {
                assert_eq!(*r, 42);
                assert_eq!(nfes.unwrap().len(), 2);
            })
            .into_result_with_non_fatal()?;
        assert_eq!(result, 42);
        assert_eq!(nfes.len(), 2);

        Ok(())
    }
}
