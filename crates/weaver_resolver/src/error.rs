//! Errors used in weaver_resolver crate.

use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_common::log_error;
use weaver_semconv::provenance::Provenance;

/// An error that can occur while resolving a telemetry schema.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// There was an issue resolving definition schema.
    #[error(transparent)]
    #[diagnostic(transparent)]
    FailToResolveDefinition(#[from] weaver_semconv::Error),

    /// We discovered a circular dependency we cannot resolve.
    #[error("Circular dependency detected: registry '{registry_name}' depends on itself through the chain: {chain}")]
    CircularDependency {
        /// The registry that depends on itself.
        registry_name: String,

        /// A string representing the dependency chain.
        chain: String,
    },

    /// We've reached the maximum dependency depth for this registry.
    #[error("Maximum dependency depth reached for registry `{registry_name}`. Cannot load further dependencies.")]
    MaximumDependencyDepth {
        /// The registry which has too many dependencies.
        registry_name: String,
    },

    /// Failed to resolve the schema URL for a registry.
    #[error("Schema URL is missing in the manifest and cannot be constructed from the registry name and version.")]
    FailToResolveSchemaUrl {},

    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    #[diagnostic(help("Check the URL and try again."))]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a set of attributes.
    #[error("Failed to resolve a set of attributes {ids:?}: {error}")]
    FailToResolveAttributes {
        /// The ids of the attributes.
        ids: Vec<String>,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a metric.
    #[error("Failed to resolve the metric '{ref}'")]
    FailToResolveMetric {
        /// The reference to the metric.
        r#ref: String,
    },

    /// Metric attributes are incompatible within the metric group.
    #[error("Metric attributes are incompatible within the metric group '{metric_group_ref}' for metric '{metric_ref}' (error: {error})")]
    IncompatibleMetricAttributes {
        /// The metric group reference.
        metric_group_ref: String,
        /// The reference to the metric.
        metric_ref: String,
        /// The error that occurred.
        error: String,
    },

    /// A generic conversion error.
    #[error("Conversion error: {message}")]
    ConversionError {
        /// The error that occurred.
        message: String,
    },

    /// An unresolved attribute reference.
    #[error("The following attribute reference is not resolved for the group '{group_id}'.\nAttribute reference: {attribute_ref}\nProvenance: {provenance}")]
    UnresolvedAttributeRef {
        /// The id of the group containing the attribute reference.
        group_id: String,
        /// The unresolved attribute reference.
        attribute_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An unresolved `extends` clause reference.
    #[error("The following `extends` clause reference is not resolved for the group '{group_id}'.\n`extends` clause reference: {extends_ref}\nProvenance: {provenance}")]
    UnresolvedExtendsRef {
        /// The id of the group containing the `extends` clause reference.
        group_id: String,
        /// The unresolved `extends` clause reference.
        extends_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An unresolved `include` reference.
    #[error("The following `include` reference is not resolved for the group '{group_id}'.\n`include` reference: {include_ref}\nProvenance: {provenance}")]
    UnresolvedIncludeRef {
        /// The id of the group containing the `include` reference.
        group_id: String,
        /// The unresolved `include` reference.
        include_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An invalid Schema path.
    #[error("Invalid Schema path: {path}")]
    InvalidSchemaPath {
        /// The schema path.
        path: PathBuf,
    },

    /// A duplicate group id error.
    #[error("The group id `{group_id}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupId {
        /// The group id.
        group_id: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate group id error.
    #[error("The group name `{group_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupName {
        /// The group name.
        group_name: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate group id error.
    #[error("The metric name `{metric_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateMetricName {
        /// The metric name.
        metric_name: String,
        /// The provenances where this metric name is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate attribute id error.
    #[error("The attribute id `{attribute_id}` is declared multiple times in the following groups:\n{group_ids:?}")]
    DuplicateAttributeId {
        /// The groups where this attribute is duplicated.
        group_ids: Vec<String>,
        /// The attribute id.
        attribute_id: String,
    },

    /// Invalid import wildcard.
    #[error("Invalid import wildcard: {error:?}")]
    #[diagnostic(help(
        "Check the wildcard syntax supported here: https://crates.io/crates/globset"
    ))]
    InvalidWildcard {
        /// The error that occurred.
        error: String,
    },

    /// We
    #[error(
        "Invalid registry: {registry_name}. Unable to find attribute by index: {attribute_ref}"
    )]
    InvalidRegistryAttributeRef {
        /// The registry with the issue.
        registry_name: String,
        /// The attribute index that does not exist in the registry.
        attribute_ref: u32,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            Error::CompoundError(errors) => errors
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

impl Error {
    /// Logs one or multiple errors (if current error is a 1CompoundError`)
    /// using the given logger.
    pub fn log(&self) {
        match self {
            Error::CompoundError(errors) => {
                for error in errors {
                    error.log();
                }
            }
            _ => log_error(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Error;
    use weaver_common::{diagnostic::DiagnosticMessages, error::handle_errors};

    #[test]
    fn test_weaver_error_api_allowed() {
        let errors = vec![Error::FailToResolveMetric {
            r#ref: "test".to_owned(),
        }];
        let result = handle_errors(errors);
        assert!(result.is_err());
    }
    #[test]
    fn test_diagnostic_message_api_conversion() {
        let _: DiagnosticMessages = Error::FailToResolveMetric {
            r#ref: "test".to_owned(),
        }
        .into();
    }
}
