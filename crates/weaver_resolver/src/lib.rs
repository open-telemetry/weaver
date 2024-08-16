// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use miette::Diagnostic;
use std::collections::HashMap;
use std::path::{PathBuf, MAIN_SEPARATOR};

use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge};
use serde::Serialize;
use walkdir::DirEntry;

use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, handle_errors, WeaverError};
use weaver_common::Logger;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::registry::Constraint;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::semconv::SemConvSpec;

use crate::attribute::AttributeCatalog;
use crate::registry::resolve_semconv_registry;

pub mod attribute;
mod constraint;
pub mod registry;

/// A resolver that can be used to resolve telemetry schemas.
/// All references to semantic conventions will be resolved.
pub struct SchemaResolver {}

/// An error that can occur while resolving a telemetry schema.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    #[diagnostic(help("Check the URL and try again."))]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// A semantic convention error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    SemConvError {
        /// The semconv error that occurred.
        #[from]
        error: weaver_semconv::Error,
    },

    /// Failed to walk dir.
    #[error("Failed walk dir: {error}")]
    FailToWalkDir {
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
    #[error("Failed to resolve the metric '{r#ref}'")]
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
        provenance: String,
    },

    /// An unresolved `extends` clause reference.
    #[error("The following `extends` clause reference is not resolved for the group '{group_id}'.\n`extends` clause reference: {extends_ref}\nProvenance: {provenance}")]
    UnresolvedExtendsRef {
        /// The id of the group containing the `extends` clause reference.
        group_id: String,
        /// The unresolved `extends` clause reference.
        extends_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: String,
    },

    /// An unresolved `include` reference.
    #[error("The following `include` reference is not resolved for the group '{group_id}'.\n`include` reference: {include_ref}\nProvenance: {provenance}")]
    UnresolvedIncludeRef {
        /// The id of the group containing the `include` reference.
        group_id: String,
        /// The unresolved `include` reference.
        include_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: String,
    },

    /// An `any_of` constraint that is not satisfied for a group.
    #[error("The following `any_of` constraint is not satisfied for the group '{group_id}'.\n`any_of` constraint: {any_of:#?}\nMissing attributes: {missing_attributes:?}")]
    UnsatisfiedAnyOfConstraint {
        /// The id of the group containing the unsatisfied `any_of` constraint.
        group_id: String,
        /// The `any_of` constraint that is not satisfied.
        any_of: Constraint,
        /// The detected missing attributes.
        missing_attributes: Vec<String>,
    },

    /// An invalid Schema path.
    #[error("Invalid Schema path: {path}")]
    InvalidSchemaPath {
        /// The schema path.
        path: PathBuf,
    },

    /// A duplicate attribute id error.
    #[error("The attribute id `{attribute_id}` is declared multiple times in the following groups:\n{group_ids:?}")]
    DuplicateAttributeId {
        /// The groups where this attribute is duplicated.
        group_ids: Vec<String>,
        /// The attribute id.
        attribute_id: String,
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

/// A constraint that is not satisfied and its missing attributes.
#[derive(Debug)]
pub struct UnsatisfiedAnyOfConstraint {
    /// The `any_of` constraint that is not satisfied.
    pub any_of: Constraint,
    /// The detected missing attributes.
    pub missing_attributes: Vec<String>,
}

impl Error {
    /// Logs one or multiple errors (if current error is a 1CompoundError`)
    /// using the given logger.
    pub fn log(&self, logger: impl Logger + Clone + Sync) {
        match self {
            Error::CompoundError(errors) => {
                for error in errors {
                    error.log(logger.clone());
                }
            }
            _ => logger.error(&self.to_string()),
        }
    }
}

impl SchemaResolver {
    /// Resolves the given semantic convention registry and returns the
    /// corresponding resolved telemetry schema.
    pub fn resolve_semantic_convention_registry(
        registry: &mut SemConvRegistry,
    ) -> Result<ResolvedTelemetrySchema, Error> {
        let mut attr_catalog = AttributeCatalog::default();
        let resolved_registry = resolve_semconv_registry(&mut attr_catalog, "", registry)?;

        let catalog = Catalog {
            attributes: attr_catalog.drain_attributes(),
        };

        let mut registries = HashMap::new();
        _ = registries.insert(registry.id().into(), resolved_registry);

        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "1.0.0".to_owned(),
            schema_url: "".to_owned(),
            registries,
            catalog,
            resource: None,
            instrumentation_library: None,
            dependencies: vec![],
            versions: None, // ToDo LQ: Implement this!
        };

        Ok(resolved_schema)
    }

    /// Loads the semantic convention specifications from the given registry path.
    /// Implementation note: semconv files are read and parsed in parallel and
    /// all errors are collected and returned as a compound error.
    ///
    /// # Arguments
    /// * `registry_repo` - The registry repository containing the semantic convention files.
    pub fn load_semconv_specs(
        registry_repo: &RegistryRepo,
    ) -> Result<Vec<(String, SemConvSpec)>, Error> {
        Self::load_semconv_from_local_path(
            registry_repo.path().to_path_buf(),
            registry_repo.registry_path_repr(),
        )
    }

    /// Loads the semantic convention specifications from the given local path.
    /// Implementation note: semconv files are read and parsed in parallel and
    /// all errors are collected and returned as a compound error.
    ///
    /// # Arguments
    /// * `local_path` - The local path containing the semantic convention files.
    /// * `registry_path_repr` - The representation of the registry path (URL or path).
    fn load_semconv_from_local_path(
        local_path: PathBuf,
        registry_path_repr: &str,
    ) -> Result<Vec<(String, SemConvSpec)>, Error> {
        fn is_hidden(entry: &DirEntry) -> bool {
            entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        }
        fn is_semantic_convention_file(entry: &DirEntry) -> bool {
            let path = entry.path();
            let extension = path.extension().unwrap_or_else(|| std::ffi::OsStr::new(""));
            let file_name = path.file_name().unwrap_or_else(|| std::ffi::OsStr::new(""));
            path.is_file()
                && (extension == "yaml" || extension == "yml")
                && file_name != "schema-next.yaml"
        }

        // Loads the semantic convention specifications from the git repo.
        // All yaml files are recursively loaded and parsed in parallel from
        // the given path.
        let result = walkdir::WalkDir::new(local_path.clone())
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .par_bridge()
            .flat_map(|entry| {
                match entry {
                    Ok(entry) => {
                        if !is_semantic_convention_file(&entry) {
                            return vec![].into_par_iter();
                        }

                        match SemConvRegistry::semconv_spec_from_file(entry.path()) {
                            Ok((path, spec)) => {
                                // Replace the local path with the git URL combined with the relative path
                                // of the semantic convention file.
                                let prefix = local_path
                                    .to_str()
                                    .map(|s| s.to_owned())
                                    .unwrap_or_default();
                                let path = if registry_path_repr.ends_with(MAIN_SEPARATOR) {
                                    let relative_path = &path[prefix.len()..];
                                    format!("{}{}", registry_path_repr, relative_path)
                                } else {
                                    let relative_path = &path[prefix.len() + 1..];
                                    format!("{}/{}", registry_path_repr, relative_path)
                                };
                                vec![Ok((path, spec))].into_par_iter()
                            }
                            Err(e) => match e {
                                weaver_semconv::Error::CompoundError(errors) => errors
                                    .into_iter()
                                    .map(|e| Err(Error::SemConvError { error: e }))
                                    .collect::<Vec<_>>()
                                    .into_par_iter(),
                                _ => vec![Err(Error::SemConvError { error: e })].into_par_iter(),
                            },
                        }
                    }
                    Err(e) => vec![Err(Error::FailToWalkDir {
                        error: e.to_string(),
                    })]
                    .into_par_iter(),
                }
            })
            .collect::<Vec<_>>();

        let mut error = vec![];
        let result = result
            .into_iter()
            .filter_map(|r| match r {
                Ok(r) => Some(r),
                Err(e) => {
                    error.push(e);
                    None
                }
            })
            .collect::<Vec<_>>();

        handle_errors(error)?;

        Ok(result)
    }
}
