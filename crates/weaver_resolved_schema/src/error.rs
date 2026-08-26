// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeRef;
use crate::error::Error::{
    AttributeNotFound, CompoundError, EventNameNotFound, InvalidSchemaUrl, RefinementBaseNotFound,
    SpanLinkTargetNotFound, UnsupportedSpanLinkAttribute,
};

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

    /// Event name does not exist on an event group in V1 schema.
    #[error("Event name not found on group: {group_id}.  This is not supported in V2 schema!")]
    EventNameNotFound {
        /// Group id.
        group_id: String,
    },

    /// A refinement group does not reference the base group it extends.
    #[error("Refinement group {group_id} does not reference a base group to extend. This is not supported in V2 schema!")]
    RefinementBaseNotFound {
        /// Group id.
        group_id: String,
    },

    /// Cannot convert from V1 to V2 schema due to invalid schema URL.
    #[error("Failed to convert from V1 to V2 schema, invalid schema URL: {url}, error: {error}")]
    InvalidSchemaUrl {
        /// The invalid schema URL.
        url: String,

        /// The error message from the URL validation.
        error: String,
    },

    /// A span link references a span type that does not exist in the registry.
    #[error("Span link target '{link_ref}' (group: {group_id}) not found among span definitions")]
    SpanLinkTargetNotFound {
        /// Group id of the span declaring the link.
        group_id: String,
        /// The link's target span type.
        link_ref: String,
    },

    /// A span link attribute uses a feature that is not supported yet.
    #[error("Unsupported span link attribute in group {group_id} (link: {link_ref}): {reason}")]
    UnsupportedSpanLinkAttribute {
        /// Group id of the span declaring the link.
        group_id: String,
        /// The link's target span type.
        link_ref: String,
        /// The reason the attribute is unsupported.
        reason: String,
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
    #[must_use]
    pub fn compound_error(errors: Vec<Self>) -> Self {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e @ AttributeNotFound { .. } => vec![e],
                    e @ EventNameNotFound { .. } => vec![e],
                    e @ RefinementBaseNotFound { .. } => vec![e],
                    e @ InvalidSchemaUrl { .. } => vec![e],
                    e @ SpanLinkTargetNotFound { .. } => vec![e],
                    e @ UnsupportedSpanLinkAttribute { .. } => vec![e],
                })
                .collect(),
        )
    }
}
