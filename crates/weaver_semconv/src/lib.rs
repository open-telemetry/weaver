// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use std::collections::HashMap;

use crate::attribute::AttributeSpec;
use crate::group::{GroupSpec, GroupType};
use crate::metric::MetricSpec;
use weaver_common::error::WeaverError;

pub mod attribute;
pub mod group;
pub mod metric;
pub mod path;
pub mod registry;
pub mod semconv;
pub mod stability;
pub mod stats;

/// An error that can occur while loading a semantic convention registry.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The semantic convention registry path pattern is invalid.
    #[error("The semantic convention registry path pattern is invalid (path_pattern: {path_pattern:?}). {error}")]
    InvalidRegistryPathPattern {
        /// The path pattern pointing to the semantic convention registry.
        path_pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention registry is not found.
    #[error("The semantic convention registry is not found (path_or_url: {path_or_url:?}). {error}")]
    RegistryNotFound {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention spec is invalid.
    #[error("The semantic convention spec is invalid (path_or_url: {path_or_url:?}). {error}")]
    InvalidSemConvSpec {
        /// The path or URL of the semantic convention spec.
        path_or_url: String,
        /// The line where the error occurred.
        line: Option<usize>,
        /// The column where the error occurred.
        column: Option<usize>,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition.
    #[error("Invalid group '{group_id}' detected while resolving '{path_or_url:?}'. {error}")]
    InvalidGroup {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention asset contains an invalid attribute definition.
    #[error("Invalid attribute definition detected while resolving '{path_or_url:?}' (group_id='{group_id}', attribute_id='{attribute_id}'). {error}")]
    InvalidAttribute {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention asset contains an invalid metric definition.
    #[error("Invalid metric definition in {path_or_url:?}.\ngroup_id=`{group_id}`. {error}")]
    InvalidMetric {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the metric.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", Error::format_errors(.0))]
    CompoundError(Vec<Error>),
}

impl WeaverError for Error {
    /// Returns a list of human-readable error messages.
    fn errors(&self) -> Vec<String> {
        match self {
            Error::CompoundError(errors) => errors.iter().flat_map(|e| e.errors()).collect(),
            _ => vec![self.to_string()],
        }
    }
}

/// Handles a list of errors and returns a compound error if the list is not
/// empty or () if the list is empty.
pub fn handle_errors(mut errors: Vec<Error>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        Err(errors
            .pop()
            .expect("should never happen as we checked the length"))
    } else {
        Err(Error::compound_error(errors))
    }
}

impl Error {
    /// Creates a compound error from a list of errors.
    /// Note: All compound errors are flattened.
    #[must_use]
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

#[cfg(test)]
mod tests {
    use crate::registry::SemConvRegistry;
    use crate::Error;
    use std::vec;
    use weaver_common::error::WeaverError;

    /// Load multiple semantic convention files in the semantic convention registry.
    /// No error should be emitted.
    #[test]
    fn test_valid_semconv_registry() {
        let yaml_files = vec![
            "data/client.yaml",
            "data/cloud.yaml",
            "data/cloudevents.yaml",
            "data/database.yaml",
            "data/database-metrics.yaml",
            "data/exception.yaml",
            "data/faas.yaml",
            "data/faas-common.yaml",
            "data/faas-metrics.yaml",
            "data/http.yaml",
            "data/http-common.yaml",
            "data/http-metrics.yaml",
            "data/jvm-metrics.yaml",
            "data/media.yaml",
            "data/messaging.yaml",
            "data/network.yaml",
            "data/rpc.yaml",
            "data/rpc-metrics.yaml",
            "data/server.yaml",
            "data/source.yaml",
            "data/trace-exception.yaml",
            "data/url.yaml",
            "data/user-agent.yaml",
            "data/vm-metrics-experimental.yaml",
            "data/tls.yaml",
        ];

        let mut registry = SemConvRegistry::default();
        for yaml in yaml_files {
            let result = registry.add_semconv_spec_from_file(yaml);
            assert!(result.is_ok(), "{:#?}", result.err().unwrap());
        }
    }

    #[test]
    fn test_invalid_semconv_registry() {
        let yaml_files = vec!["data/invalid.yaml"];

        let mut registry = SemConvRegistry::default();
        for yaml in yaml_files {
            let result = registry.add_semconv_spec_from_file(yaml);
            assert!(result.is_err(), "{:#?}", result.ok().unwrap());
            if let Err(err) = result {
                assert_eq!(err.errors().len(), 1);
                let output = Error::format_errors(&[err]);
                assert!(!output.is_empty());
            }
        }
    }
}
