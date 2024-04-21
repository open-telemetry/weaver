// SPDX-License-Identifier: Apache-2.0

//! This crate implements the process of reference resolution for telemetry schemas.

use std::collections::HashMap;
use std::path::PathBuf;

use rayon::iter::ParallelBridge;
use rayon::iter::ParallelIterator;
use walkdir::DirEntry;

use weaver_cache::Cache;
use weaver_common::error::WeaverError;
use weaver_common::Logger;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::registry::Constraint;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::path::RegistryPath;
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
#[derive(thiserror::Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// A semantic convention error.
    #[error("{message}")]
    SemConvError {
        /// The error that occurred.
        message: String,
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
pub fn handle_errors(errors: Vec<Error>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::CompoundError(errors))
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
    /// * `registry_path` - The registry path containing the semantic convention files.
    /// * `cache` - The cache to store the semantic convention files.
    pub fn load_semconv_specs(
        registry_path: &RegistryPath,
        cache: &Cache,
    ) -> Result<Vec<(String, SemConvSpec)>, Error> {
        match registry_path {
            RegistryPath::Local { path_pattern: path } => {
                Self::load_semconv_from_local_path(path.into(), path)
            }
            RegistryPath::GitUrl { git_url, path } => {
                match cache.git_repo(git_url.clone(), path.clone()) {
                    Ok(local_git_repo) => {
                        Self::load_semconv_from_local_path(local_git_repo, git_url)
                    }
                    Err(e) => Err(Error::SemConvError {
                        message: e.to_string(),
                    }),
                }
            }
        }
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
            .filter_map(|entry| {
                match entry {
                    Ok(entry) => {
                        if !is_semantic_convention_file(&entry) {
                            return None;
                        }

                        let spec =
                            SemConvRegistry::semconv_spec_from_file(entry.path()).map_err(|e| {
                                Error::SemConvError {
                                    message: e.to_string(),
                                }
                            });
                        match spec {
                            Ok((path, spec)) => {
                                // Replace the local path with the git URL combined with the relative path
                                // of the semantic convention file.
                                let prefix = local_path
                                    .to_str()
                                    .map(|s| s.to_owned())
                                    .unwrap_or_default();
                                let path =
                                    format!("{}/{}", registry_path_repr, &path[prefix.len() + 1..]);
                                Some(Ok((path, spec)))
                            }
                            Err(e) => Some(Err(e)),
                        }
                    }
                    Err(e) => Some(Err(Error::SemConvError {
                        message: e.to_string(),
                    })),
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
