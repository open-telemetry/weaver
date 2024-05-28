// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use crate::Error::CompoundError;
use miette::Diagnostic;
use serde::Serialize;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};

pub mod attribute;
pub mod group;
pub mod metric;
pub mod path;
pub mod registry;
pub mod semconv;
pub mod stability;
pub mod stats;

/// An error that can occur while loading a semantic convention registry.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
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
    #[error(
        "The semantic convention registry is not found (path_or_url: {path_or_url:?}). {error}"
    )]
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
    #[error("{:?}", format_errors(.0))]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::SemConvRegistry;
    use std::vec;
    use weaver_common::diagnostic::DiagnosticMessages;

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
                let output = format!("{}", err);
                let diag_msgs: DiagnosticMessages = err.into();
                assert_eq!(diag_msgs.len(), 1);
                assert!(!output.is_empty());
            }
        }
    }
}
