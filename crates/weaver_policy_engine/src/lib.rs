// SPDX-License-Identifier: Apache-2.0

//! This crate integrates a general purpose policy engine with the Weaver
//! project. The project `regorus` is the policy engine used in this crate to
//! evaluate policies.

mod engine;
mod violation;

/// An error that can occur while evaluating policies.
#[derive(thiserror::Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// An invalid policy.
    #[error("Invalid policy file '{file}', error: {error})")]
    InvalidPolicyFile {
        /// The file that caused the error.
        file: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid data.
    #[error("Invalid data, error: {error})")]
    InvalidData {
        /// The error that occurred.
        error: String,
    },

    /// An invalid input.
    #[error("Invalid input, error: {error})")]
    InvalidInput {
        /// The error that occurred.
        error: String,
    },

    /// Violation evaluation error.
    #[error("Violation evaluation error: {error}")]
    ViolationEvaluationError {
        /// The error that occurred.
        error: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", Error::format_errors(.0))]
    CompoundError(Vec<Error>),
}

/// Handles a list of errors and returns a compound error if the list is not
/// empty or () if the list is empty.
pub fn handle_errors(errors: Vec<Error>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::CompoundError(errors))
    }
}

impl Error {
    /// Creates a compound error from a list of errors.
    /// Note: All compound errors are flattened.
    pub fn compound_error(errors: Vec<Error>) -> Error {
        Error::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Error::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }

    /// Formats the given errors into a single string.
    /// This used to render compound errors.
    #[must_use]
    pub fn format_errors(errors: &[Error]) -> String {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("\n\n")
    }
}
