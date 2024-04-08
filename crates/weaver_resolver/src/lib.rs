// SPDX-License-Identifier: Apache-2.0

//! This crate implements the process of reference resolution for telemetry schemas.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use regex::Regex;
use url::Url;
use walkdir::DirEntry;

use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_policy_engine::violation::Violation;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::registry::Constraint;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_schema::TelemetrySchema;
use weaver_semconv::{ResolverConfig, SemConvRegistry, SemConvSpec, SemConvSpecWithProvenance};
use weaver_semconv::path::RegistryPath;
use weaver_version::VersionChanges;

use crate::attribute::AttributeCatalog;
use crate::events::resolve_events;
use crate::metrics::resolve_metrics;
use crate::registry::resolve_semconv_registry;
use crate::resource::resolve_resource;
use crate::spans::resolve_spans;

pub mod attribute;
mod constraint;
mod events;
mod metrics;
pub mod registry;
mod resource;
mod spans;
mod tags;

/// A resolver that can be used to resolve telemetry schemas.
/// All references to semantic conventions will be resolved.
pub struct SchemaResolver {}

/// An error that can occur while resolving a telemetry schema.
#[derive(thiserror::Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// A telemetry schema error.
    #[error("Telemetry schema error (error: {0:?})")]
    TelemetrySchemaError(weaver_schema::Error),

    /// A parent schema error.
    #[error("Parent schema error (error: {0:?})")]
    ParentSchemaError(weaver_schema::Error),

    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// A semantic convention error.
    #[error("Semantic convention error: {message}")]
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

    /// A policy violation error.
    #[error("Policy violation: {violation}, provenance: {provenance}")]
    PolicyViolation {
        /// The provenance of the violation (URL or path).
        provenance: String,
        /// The violation.
        violation: Violation,
    },

    /// A container for multiple errors.
    #[error("{:?}", Error::format_errors(.0))]
    CompoundError(Vec<Error>),
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
            Error::CompoundError(errors) => for error in errors {
                error.log(logger.clone());
            }
            _ => logger.error(&self.to_string()),
        }
    }
}

impl SchemaResolver {
    /// Loads a telemetry schema from an URL or a file and returns the resolved
    /// schema.
    pub fn resolve_schema(
        schema_url_or_path: &str,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<TelemetrySchema, Error> {
        let mut schema = Self::load_schema(schema_url_or_path, log.clone())?;
        Self::resolve(&mut schema, schema_url_or_path, cache, log, policy_engine)?;

        Ok(schema)
    }

    /// Loads a telemetry schema file and returns the resolved schema.
    pub fn resolve_schema_file<P: AsRef<Path> + Clone>(
        schema_path: P,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<TelemetrySchema, Error> {
        let mut schema = Self::load_schema_from_path(schema_path.clone(), log.clone())?;
        Self::resolve(
            &mut schema,
            schema_path
                .as_ref()
                .to_str()
                .ok_or_else(|| Error::InvalidSchemaPath {
                    path: schema_path.as_ref().to_path_buf(),
                })?,
            cache,
            log,
            policy_engine,
        )?;

        Ok(schema)
    }

    /// Resolve the given telemetry schema.
    fn resolve(
        schema: &mut TelemetrySchema,
        schema_path: &str,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<(), Error> {
        let registry_id = "default"; // ToDo add support for multiple registries
        let sem_conv_catalog =
            Self::semconv_registry_from_schema(registry_id, schema, cache, log.clone(), policy_engine)?;
        let start = Instant::now();

        // Merges the versions of the parent schema into the current schema.
        schema.merge_versions();

        // Generates version changes
        let version_changes = schema
            .versions
            .as_ref()
            .map(|versions| {
                if let Some(latest_version) = versions.latest_version() {
                    versions.version_changes_for(&latest_version)
                } else {
                    VersionChanges::default()
                }
            })
            .unwrap_or_default();

        // Resolve the references to the semantic conventions.
        log.loading("Solving semantic convention references");
        if let Some(schema) = schema.schema.as_mut() {
            resolve_resource(schema, &sem_conv_catalog, &version_changes)?;
            resolve_metrics(schema, &sem_conv_catalog, &version_changes)?;
            resolve_events(schema, &sem_conv_catalog, &version_changes)?;
            resolve_spans(schema, &sem_conv_catalog, version_changes)?;
        }
        log.success(&format!(
            "Resolved schema '{}' ({:.2}s)",
            schema_path,
            start.elapsed().as_secs_f32()
        ));

        schema.semantic_conventions.clear();
        schema.set_semantic_convention_catalog(sem_conv_catalog);

        Ok(())
    }

    /// Loads and resolves a semantic convention registry from the given Git URL.
    pub fn resolve_semconv_registry(
        registry_id: &str,
        registry_git_url: String,
        path: Option<String>,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        Self::semconv_registry_from_imports(
            registry_id,
            &[RegistryPath::GitUrl {
                git_url: registry_git_url,
                path,
            }],
            ResolverConfig::default(),
            cache,
            log.clone(),
            policy_engine,
        )
    }

    /// Loads a semantic convention registry from the given Git URL.
    pub fn load_semconv_registry(
        registry_id: &str,
        registry_path: RegistryPath,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        Self::load_semconv_registry_from_imports(registry_id, &[registry_path], cache, log.clone(), policy_engine)
    }

    /// Loads a telemetry schema from the given URL or path.
    pub fn load_schema(
        schema_url_or_path: &str,
        log: impl Logger + Clone + Sync,
    ) -> Result<TelemetrySchema, Error> {
        let start = Instant::now();
        log.loading(&format!("Loading schema '{}'", schema_url_or_path));

        let mut schema = TelemetrySchema::load(schema_url_or_path).map_err(|e| {
            log.error(&format!("Failed to load schema '{}'", schema_url_or_path));
            Error::TelemetrySchemaError(e)
        })?;
        log.success(&format!(
            "Loaded schema '{}' ({:.2}s)",
            schema_url_or_path,
            start.elapsed().as_secs_f32()
        ));

        let parent_schema = Self::load_parent_schema(&schema, log.clone())?;
        schema.set_parent_schema(parent_schema);
        Ok(schema)
    }

    /// Loads a telemetry schema from the given path.
    pub fn load_schema_from_path<P: AsRef<Path> + Clone>(
        schema_path: P,
        log: impl Logger + Clone + Sync,
    ) -> Result<TelemetrySchema, Error> {
        let start = Instant::now();
        log.loading(&format!(
            "Loading schema '{}'",
            schema_path.as_ref().display()
        ));

        let mut schema = TelemetrySchema::load_from_file(schema_path.clone()).map_err(|e| {
            log.error(&format!(
                "Failed to load schema '{}'",
                schema_path.as_ref().display()
            ));
            Error::TelemetrySchemaError(e)
        })?;
        log.success(&format!(
            "Loaded schema '{}' ({:.2}s)",
            schema_path.as_ref().display(),
            start.elapsed().as_secs_f32()
        ));

        let parent_schema = Self::load_parent_schema(&schema, log.clone())?;
        schema.set_parent_schema(parent_schema);
        Ok(schema)
    }

    /// Loads a semantic convention registry from the given schema.
    pub fn semconv_registry_from_schema(
        registry_id: &str,
        schema: &TelemetrySchema,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        Self::semconv_registry_from_imports(
            registry_id,
            &schema.merged_semantic_conventions(),
            ResolverConfig::default(),
            cache,
            log.clone(),
            policy_engine,
        )
    }

    /// Loads a semantic convention registry from the given semantic convention imports.
    pub fn load_semconv_registry_from_imports(
        registry_id: &str,
        imports: &[RegistryPath],
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        let start = Instant::now();
        let registry =
            Self::create_semantic_convention_registry(registry_id, imports, cache, log.clone(), policy_engine)?;
        log.success(&format!(
            "Loaded {} semantic convention files ({:.2}s)",
            registry.asset_count(),
            start.elapsed().as_secs_f32()
        ));

        Ok(registry)
    }

    /// Loads a semantic convention registry from the given semantic convention imports.
    pub fn semconv_registry_from_imports(
        registry_id: &str,
        imports: &[RegistryPath],
        resolver_config: ResolverConfig,
        cache: &Cache,
        log: impl Logger + Clone + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        let start = Instant::now();
        let mut registry =
            Self::create_semantic_convention_registry(registry_id, imports, cache, log.clone(), policy_engine)?;
        let warnings = registry
            .resolve(resolver_config)
            .map_err(|e| Error::SemConvError {
                message: e.to_string(),
            })?;
        for warning in warnings {
            log.warn("Semantic convention warning");
            log.log(&warning.error.to_string());
        }
        log.success(&format!(
            "Loaded {} semantic convention files containing the definition of {} attributes and {} metrics ({:.2}s)",
            registry.asset_count(),
            registry.attribute_count(),
            registry.metric_count(),
            start.elapsed().as_secs_f32()
        ));

        Ok(registry)
    }

    /// Resolves the given semantic convention registry and returns the
    /// corresponding resolved telemetry schema.
    pub fn resolve_semantic_convention_registry(
        registry: &mut SemConvRegistry,
        log: impl Logger + Clone + Sync,
    ) -> Result<ResolvedTelemetrySchema, Error> {
        let start = Instant::now();

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

        log.success(&format!(
            "Resolved {} semantic convention files containing the definition of {} attributes and {} metrics ({:.2}s)",
            registry.asset_count(),
            registry.attribute_count(),
            registry.metric_count(),
            start.elapsed().as_secs_f32()
        ));

        Ok(resolved_schema)
    }

    /// Loads the parent telemetry schema if it exists.
    fn load_parent_schema(
        schema: &TelemetrySchema,
        log: impl Logger,
    ) -> Result<Option<TelemetrySchema>, Error> {
        let start = Instant::now();
        // Load the parent schema and merge it into the current schema.
        let parent_schema = if let Some(parent_schema_url) = schema.parent_schema_url.as_ref() {
            log.loading(&format!("Loading parent schema '{}'", parent_schema_url));
            let url_pattern = Regex::new(r"^(https|http|file):.*")
                .expect("invalid regex, please report this bug");
            let parent_schema = if url_pattern.is_match(parent_schema_url) {
                let url = Url::parse(parent_schema_url).map_err(|e| {
                    log.error(&format!(
                        "Failed to parset parent schema url '{}'",
                        parent_schema_url
                    ));
                    Error::InvalidUrl {
                        url: parent_schema_url.clone(),
                        error: e.to_string(),
                    }
                })?;
                TelemetrySchema::load_from_url(&url).map_err(|e| {
                    log.error(&format!(
                        "Failed to load parent schema '{}'",
                        parent_schema_url
                    ));
                    Error::ParentSchemaError(e)
                })?
            } else {
                TelemetrySchema::load_from_file(parent_schema_url).map_err(|e| {
                    log.error(&format!(
                        "Failed to load parent schema '{}'",
                        parent_schema_url
                    ));
                    Error::ParentSchemaError(e)
                })?
            };

            log.success(&format!(
                "Loaded parent schema '{}' ({:.2}s)",
                parent_schema_url,
                start.elapsed().as_secs_f32()
            ));
            Some(parent_schema)
        } else {
            None
        };

        Ok(parent_schema)
    }

    /// Creates a semantic convention registry from the given telemetry schema.
    fn create_semantic_convention_registry(
        registry_id: &str,
        sem_convs: &[RegistryPath],
        cache: &Cache,
        log: impl Logger + Sync,
        policy_engine: Option<&weaver_policy_engine::Engine>,
    ) -> Result<SemConvRegistry, Error> {
        // Load all the semantic convention catalogs.
        let mut sem_conv_catalog = SemConvRegistry::new(registry_id);
        let total_file_count = sem_convs.len();
        let loaded_files_count = AtomicUsize::new(0);
        let error_count = AtomicUsize::new(0);

        let result: Vec<Result<(String, SemConvSpec), Error>> = sem_convs
            .par_iter()
            .flat_map(|sem_conv_import| {
                let results = Self::import_sem_conv_specs(sem_conv_import, cache);
                for result in results.iter() {
                    if result.is_err() {
                        _ = error_count.fetch_add(1, Relaxed);
                    }
                    _ = loaded_files_count.fetch_add(1, Relaxed);
                    if error_count.load(Relaxed) == 0 {
                        log.loading(&format!(
                            "Loaded {}/{} semantic convention files (no error detected)",
                            loaded_files_count.load(Relaxed),
                            total_file_count
                        ));
                    } else {
                        log.loading(&format!(
                            "Loaded {}/{} semantic convention files ({} error(s) detected)",
                            loaded_files_count.load(Relaxed),
                            total_file_count,
                            error_count.load(Relaxed)
                        ));
                    }
                }
                results
            })
            .collect();

        let mut errors = vec![];

        if let Some(policy_engine) = policy_engine {
            // Check policies in parallel
            errors.extend(result
                .par_iter()
                .flat_map(|semconv| {
                    let mut errors = Vec::new();
                    if let Ok((path, semconv)) = semconv {
                        // Create a local policy engine inheriting the policies
                        // from the global policy engine
                        let mut policy_engine = policy_engine.clone();

                        match policy_engine.set_input(semconv) {
                            Ok(_) => {
                                match policy_engine.check() {
                                    Ok(violations) => for violation in violations {
                                        errors.push(Error::PolicyViolation { provenance: path.clone(), violation });
                                    }
                                    Err(e) => errors.push(Error::SemConvError {
                                        message: format!("Invalid policy evaluation for file '{path}': {e}"),
                                    })
                                }
                            }
                            Err(e) => errors.push(Error::SemConvError {
                                message: format!("Invalid policy engine input for file '{path}': {e}"),
                            })
                        }
                    }
                    errors
                }).collect::<Vec<Error>>());
        }

        result.into_iter().for_each(|result| match result {
            Ok((provenance, spec)) => {
                sem_conv_catalog
                    .append_sem_conv_spec(SemConvSpecWithProvenance { provenance, spec });
            }
            Err(e) => {
                log.error(&e.to_string());
                errors.push(e);
            }
        });

        handle_errors(errors)?;

        Ok(sem_conv_catalog)
    }

    /// Imports the semantic convention specifications from the given registry path.
    /// This function returns a vector of results because the import declaration can be a
    /// URL or a git URL (containing potentially multiple semantic convention specifications).
    fn import_sem_conv_specs(
        registry_path: &RegistryPath,
        cache: &Cache,
    ) -> Vec<Result<(String, SemConvSpec), Error>> {
        match registry_path {
            RegistryPath::Local { local_path: path } => {
                Self::import_semconv_from_local_path(path.into(), path)
            }
            RegistryPath::GitUrl { git_url, path } => {
                match cache.git_repo(git_url.clone(), path.clone()) {
                    Ok(local_git_repo) => {
                        Self::import_semconv_from_local_path(local_git_repo, git_url)
                    }
                    Err(e) => vec![Err(Error::SemConvError {
                        message: e.to_string(),
                    })],
                }
            }
        }
    }

    /// Imports the semantic convention specifications from the given local path.
    ///
    /// # Arguments
    /// * `local_path` - The local path containing the semantic convention files.
    /// * `registry_path_repr` - The representation of the registry path (URL or path).
    /// * `policy_engine` - An optional policy engine to check the semantic convention files.
    fn import_semconv_from_local_path(
        local_path: PathBuf,
        registry_path_repr: &str,
    ) -> Vec<Result<(String, SemConvSpec), Error>> {
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

        let mut result = vec![];

        // Loads the semantic convention specifications from the git repo.
        // All yaml files are recursively loaded from the given path.
        for entry in walkdir::WalkDir::new(local_path.clone())
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
        {
            match entry {
                Ok(entry) => {
                    if is_semantic_convention_file(&entry) {
                        let spec = SemConvRegistry::load_sem_conv_spec_from_file(entry.path())
                            .map_err(|e| Error::SemConvError {
                                message: e.to_string(),
                            });
                        result.push(match spec {
                            Ok((path, spec)) => {
                                // Replace the local path with the git URL combined with the relative path
                                // of the semantic convention file.
                                let prefix = local_path
                                    .to_str()
                                    .map(|s| s.to_owned())
                                    .unwrap_or_default();
                                let path =
                                    format!("{}/{}", registry_path_repr, &path[prefix.len() + 1..]);
                                Ok((path, spec))
                            }
                            Err(e) => Err(e),
                        });
                    }
                }
                Err(e) => result.push(Err(Error::SemConvError {
                    message: e.to_string(),
                })),
            }
        }

        result
    }
}

#[cfg(test)]
mod test {
    use weaver_cache::Cache;
    use weaver_logger::{ConsoleLogger, Logger};

    use crate::SchemaResolver;

    #[test]
    fn resolve_schema() {
        let log = ConsoleLogger::new(0);
        let cache = Cache::try_new().unwrap_or_else(|e| {
            log.error(&e.to_string());
            #[allow(clippy::exit)] // Expected exit
            std::process::exit(1);
        });
        let schema = SchemaResolver::resolve_schema_file(
            "../../data/app-telemetry-schema.yaml",
            &cache,
            log,
            None,
        );
        assert!(schema.is_ok(), "{:#?}", schema.err().unwrap());
    }
}
