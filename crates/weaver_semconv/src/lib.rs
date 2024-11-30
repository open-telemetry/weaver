// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use crate::Error::CompoundError;
use miette::Diagnostic;
use serde::Serialize;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};

pub mod any_value;
pub mod attribute;
pub mod group;
pub mod metric;
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

    /// A generic error related to a semantic convention spec.
    #[error(
        "The following error occurred during the processing of semantic convention file: {error}"
    )]
    SemConvSpecError {
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

    /// The semantic convention spec contains an invalid group stability definition.
    #[error("Invalid stability on group '{group_id}' detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupStability {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition. Missing extends or attributes
    #[error("Invalid group '{group_id}', missing extends or attributes, detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupMissingExtendsOrAttributes {
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

    /// This error occurs when a semantic convention asset contains an invalid example.
    /// This is treated as a critical error in the current context.
    #[error("The attribute `{attribute_id}` in the group `{group_id}` contains an invalid example. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Error))]
    InvalidExampleError {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This warning indicates that a semantic convention asset contains an invalid example.
    /// It is treated as a non-critical warning unless the `--future` flag is enabled.
    /// With the `--future` flag, this warning is elevated to an error.
    #[error("The attribute `{attribute_id}` in the group `{group_id}` contains an example that will be considered invalid in the future. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Warning))]
    InvalidExampleWarning {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This warning indicates usage of `prefix` on a group.
    /// With the `--future` flag, this warning is elevated to an error.
    #[error("The group `{group_id}` defines a prefix. These are no longer used.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupUsesPrefix {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
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

    /// This indicates that a semantic convention asset contains an invalid example.
    #[error("The value `{value_id}` in the group `{group_id}` contains an invalid example. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Error))]
    InvalidAnyValueExampleError {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the any_value
        value_id: String,
        /// The reason of the error.
        error: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),
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
            "data/event.yaml",
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
            let result = registry
                .add_semconv_spec_from_file(yaml)
                .into_result_failing_non_fatal();
            assert!(result.is_ok(), "{:#?}", result.err().unwrap());
        }
    }

    #[test]
    fn test_invalid_semconv_registry() {
        let yaml_files = vec!["data/invalid.yaml"];

        let mut registry = SemConvRegistry::default();
        for yaml in yaml_files {
            let result = registry
                .add_semconv_spec_from_file(yaml)
                .into_result_failing_non_fatal();
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
