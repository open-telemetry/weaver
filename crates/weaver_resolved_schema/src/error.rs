// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeRef;
use crate::error::Error::{AttributeNotFound, CompoundError, NotImplemented};

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Deserialize, Serialize)]
pub enum Error {
    /// Attribute reference not found in the catalog.
    #[error("Attribute reference {attr_ref} (group: {group_id}) not found in the catalog")]
    AttributeNotFound {
        /// Group id.
        group_id: String,
        /// Attribute reference.
        attr_ref: AttributeRef,
    },

    /// A generic container for multiple errors.
    #[error("Errors:\n{0:#?}")]
    CompoundError(Vec<Error>),

    /// A generic error identifying a feature that has not yet been implemented.
    #[error("Not Implemented: {message}")]
    NotImplemented {
        /// A message describing the feature that has not been implemented.
        message: String,
    },
}

/// Handles a list of errors and returns a compound error if the list is not
/// empty or () if the list is empty.
pub fn handle_errors(errors: Vec<Error>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::compound_error(errors))
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
                    e @ AttributeNotFound { .. } => vec![e],
                    e @ NotImplemented { .. } => vec![e],
                })
                .collect(),
        )
    }
}
