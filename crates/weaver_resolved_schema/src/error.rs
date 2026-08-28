// SPDX-License-Identifier: Apache-2.0

//! Error types and utilities.

use serde::{Deserialize, Serialize};

use crate::error::Error::{
    AttributeNotFound, CompoundError, EntityAssociationNotFound, EventNameNotFound,
    InvalidSchemaUrl, RefinementBaseNotFound,
};
use crate::v1::attribute::AttributeRef;

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

    /// An entity association names an entity that nothing in scope defines.
    #[error("Group {group_id} is associated with entity {entity_type}, which neither this registry nor any of its dependencies defines")]
    EntityAssociationNotFound {
        /// Group id.
        group_id: String,
        /// The entity type that nothing defines.
        entity_type: String,
    },

    /// Cannot convert from V1 to V2 schema due to invalid schema URL.
    #[error("Failed to convert from V1 to V2 schema, invalid schema URL: {url}, error: {error}")]
    InvalidSchemaUrl {
        /// The invalid schema URL.
        url: String,

        /// The error message from the URL validation.
        error: String,
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
                    e @ EntityAssociationNotFound { .. } => vec![e],
                    e @ InvalidSchemaUrl { .. } => vec![e],
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_errors_empty() {
        assert!(handle_errors(vec![]).is_ok());
    }

    #[test]
    fn test_handle_errors_and_compound_error_flattening() {
        let err1 = AttributeNotFound {
            group_id: "http.client".to_owned(),
            attr_ref: AttributeRef(42),
        };
        let err2 = EventNameNotFound {
            group_id: "event.unnamed".to_owned(),
        };
        let err3 = RefinementBaseNotFound {
            group_id: "refinement.orphan".to_owned(),
        };
        let err4 = EntityAssociationNotFound {
            group_id: "span.db".to_owned(),
            entity_type: "db.instance".to_owned(),
        };
        let err5 = InvalidSchemaUrl {
            url: "invalid::url".to_owned(),
            error: "bad scheme".to_owned(),
        };

        // Format checks
        assert!(err1
            .to_string()
            .contains("Attribute reference AttributeRef(42) (group: http.client) not found"));
        assert!(err2
            .to_string()
            .contains("Event name not found on group: event.unnamed"));
        assert!(err3
            .to_string()
            .contains("Refinement group refinement.orphan does not reference a base group"));
        assert!(err4
            .to_string()
            .contains("Group span.db is associated with entity db.instance"));
        assert!(err5
            .to_string()
            .contains("invalid schema URL: invalid::url"));

        // Nested compound error flattening
        let nested = CompoundError(vec![err1, err2]);
        let compound = Error::compound_error(vec![nested, err3, err4, err5]);
        match compound {
            CompoundError(flat_errors) => {
                assert_eq!(flat_errors.len(), 5);
            }
            _ => panic!("Expected Error::CompoundError"),
        }

        let res = handle_errors(vec![EventNameNotFound {
            group_id: "group1".to_owned(),
        }]);
        assert!(res.is_err());
    }
}
