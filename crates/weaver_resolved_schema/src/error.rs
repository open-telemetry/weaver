// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use crate::attribute::AttributeRef;
use crate::error::Error::{AttributeNotFound, CompoundError};

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone)]
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
    pub fn compound_error(errors: Vec<Self>) -> Self {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e @ AttributeNotFound { .. } => vec![e],
                })
                .collect(),
        )
    }
}
