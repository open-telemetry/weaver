// SPDX-License-Identifier: Apache-2.0

use itertools::Itertools;
use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge};
use std::collections::HashSet;
use std::fmt::Display;
use std::path::MAIN_SEPARATOR;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_semconv::registry::SemConvRegistry;

use walkdir::DirEntry;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::json_schema::JsonSchemaValidator;
use weaver_semconv::registry_repo::{RegistryRepo, REGISTRY_MANIFEST};
use weaver_semconv::{group::ImportsWithProvenance, semconv::SemConvSpecWithProvenance};

use crate::Error;

/// Maximum allowed depth for registry dependency chains.
const MAX_DEPENDENCY_DEPTH: u32 = 10;

/// The result of loading a semantic convention URL prior to resolution.
pub enum LoadedSemconvRegistry {
    /// The semconv repository was unresolved and needs to be run through resolution.
    Unresolved {
        /// The specification of this raw repository.
        repo: RegistryRepo,
        /// The raw definition schema for this repository.
        specs: Vec<SemConvSpecWithProvenance>,
        /// List of unresolved imports that should be loaded from dependencies.
        imports: Vec<ImportsWithProvenance>,
        /// The dependencies of this repository.
        dependencies: Vec<LoadedSemconvRegistry>,
    },
    /// The semconv repository is already resolved and can be used as-is.
    Resolved(V1Schema),
    /// The semconv repository is already resolved and can be used as-is.
    ResolvedV2(V2Schema),
}

impl LoadedSemconvRegistry {
    /// Creates a loaded semconv registry from a single string.
    #[cfg(test)]
    pub fn create_from_string(spec: &str) -> Result<LoadedSemconvRegistry, Error> {
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_semconv::provenance::Provenance;
        let path: VirtualDirectoryPath = "data".try_into().expect("Bad fake path for test");
        let repo = RegistryRepo::try_new("default", &path).map_err(|e| Error::InvalidUrl {
            url: "test string".to_owned(),
            error: format!("{e}"),
        })?;
        let provenance = Provenance::new("default", "<str>");
        let spec_with_provenance = SemConvSpecWithProvenance::from_string(provenance, spec)
            .into_result_failing_non_fatal()
            .map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("{e}"),
            })?;
        Ok(LoadedSemconvRegistry::Unresolved {
            repo,
            specs: vec![spec_with_provenance],
            imports: vec![],
            dependencies: vec![],
        })
    }

    /// Returns true if the repository is unresolved.
    #[must_use]
    pub fn is_unresolved(&self) -> bool {
        matches!(self, LoadedSemconvRegistry::Unresolved { .. })
    }

    /// The path representing this registry.
    #[must_use]
    pub fn registry_path_repr(&self) -> &str {
        match self {
            LoadedSemconvRegistry::Unresolved { repo, .. } => repo.registry_path_repr(),
            // TODO - are these correct?
            LoadedSemconvRegistry::Resolved(schema) => &schema.schema_url,
            LoadedSemconvRegistry::ResolvedV2(schema) => &schema.schema_url,
        }
    }

    /// Returns the depth of the dependency chain for this loaded repository.
    #[cfg(test)]
    #[must_use]
    pub fn dependency_depth(&self) -> u32 {
        match self {
            LoadedSemconvRegistry::Unresolved { dependencies, .. } => {
                1 + dependencies
                    .iter()
                    .map(|d| d.dependency_depth())
                    .max()
                    .unwrap_or_default()
            }
            LoadedSemconvRegistry::Resolved(_) => 1,
            LoadedSemconvRegistry::ResolvedV2(_) => 1,
        }
    }

    /// Returns all the registry ids in this loaded registry and its dependencies.
    #[cfg(test)]
    #[must_use]
    pub fn registry_ids(&self) -> Vec<String> {
        match self {
            LoadedSemconvRegistry::Unresolved {
                repo, dependencies, ..
            } => {
                let mut result = vec![repo.id().to_string()];
                for d in dependencies {
                    result.extend(d.registry_ids());
                }
                result
            }
            LoadedSemconvRegistry::Resolved(schema) => vec![schema.registry_id.clone()],
            LoadedSemconvRegistry::ResolvedV2(schema) => vec![schema.registry_id.clone()],
        }
    }
}

impl Display for LoadedSemconvRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs: _,
                imports: _,
                dependencies,
            } => write!(
                f,
                "{} - [{}]",
                repo.id(),
                dependencies.iter().map(|d| format!("{d}")).join(",")
            ),
            LoadedSemconvRegistry::Resolved(schema) => write!(f, "{}", schema.registry_id),
            LoadedSemconvRegistry::ResolvedV2(schema) => write!(f, "{}", schema.registry_id),
        }
    }
}

/// Loads a semantic convention repository.
///
/// Note: This may load in a definition (raw) repository *or* an already resolved repository.
///       When loading a raw repository, dependencies will also be loaded.
pub(crate) fn load_semconv_repository(
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
) -> WResult<LoadedSemconvRegistry, Error> {
    // This method simply sets up the resolution state and delegates to the actual work.
    let mut visited_registries = HashSet::new();
    let mut dependency_chain = Vec::new();
    load_semconv_repository_recursive(
        registry_repo,
        follow_symlinks,
        MAX_DEPENDENCY_DEPTH,
        &mut visited_registries,
        &mut dependency_chain,
    )
}

/// Recursively iterates over semconv dependencies and loads their definition.
/// Note: Prevents circular dependencies.
fn load_semconv_repository_recursive(
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    max_dependency_depth: u32,
    visited_registries: &mut HashSet<String>,
    dependency_chain: &mut Vec<String>,
) -> WResult<LoadedSemconvRegistry, Error> {
    // Make sure we don't go past our max dependency depth.
    if max_dependency_depth == 0 {
        return WResult::FatalErr(Error::MaximumDependencyDepth {
            registry: registry_repo.registry_path_repr().to_owned(),
        });
    }
    let registry_id = registry_repo.id().to_string();
    // Check for circular dependency
    if visited_registries.contains(&registry_id) {
        dependency_chain.push(registry_id.clone());
        let chain_str = dependency_chain.join(" → ");
        return WResult::FatalErr(Error::CircularDependency {
            registry_id,
            chain: chain_str,
        });
    }
    // Add current registry to visited set and dependency chain
    let _ = visited_registries.insert(registry_id.clone());
    dependency_chain.push(registry_id.clone());

    // Either load a fully resolved repository, or read in raw files.
    if let Some(manifest) = registry_repo.manifest() {
        if let Some(resolved_url) = registry_repo.resolved_schema_url() {
            load_resolved_repository(&resolved_url)
        } else {
            if manifest.dependencies.len() > 1 {
                todo!("Multiple dependencies is not supported yet.")
            }
            // Load dependencies.
            let mut loaded_dependencies = vec![];
            let mut non_fatal_errors = vec![];
            for d in manifest.dependencies.iter() {
                match RegistryRepo::try_new(&d.name, &d.registry_path) {
                    Ok(d_repo) => {
                        // so we need to make sure the dependency chain only include direct dependencies of each other.
                        match load_semconv_repository_recursive(
                            d_repo,
                            follow_symlinks,
                            max_dependency_depth - 1,
                            visited_registries,
                            dependency_chain,
                        ) {
                            WResult::Ok(d) => loaded_dependencies.push(d),
                            WResult::OkWithNFEs(d, nfes) => {
                                loaded_dependencies.push(d);
                                non_fatal_errors.extend(nfes);
                            }
                            WResult::FatalErr(err) => return WResult::FatalErr(err),
                        }
                    }
                    Err(err) => return WResult::FatalErr(err.into()),
                }
            }
            // Now load the raw repository.
            // TODO - Allow ignoring dependency warnings - https://github.com/open-telemetry/weaver/issues/1126.
            load_definition_repository(registry_repo, follow_symlinks, loaded_dependencies)
                .extend_non_fatal_errors(non_fatal_errors)
        }
    } else {
        // This is a raw repository with *no* manifest.
        // TODO - issue a warning that manifest will be required w/ 2.0 to allow publishing.
        load_definition_repository(registry_repo, follow_symlinks, vec![])
    }
}

/// Loads a resolved repository.
fn load_resolved_repository(path: &VirtualDirectoryPath) -> WResult<LoadedSemconvRegistry, Error> {
    // TODO - should we handle V1 and V2?
    match from_vdir(path) {
        Ok(resolved) => WResult::Ok(LoadedSemconvRegistry::ResolvedV2(resolved)),
        Err(err) => WResult::FatalErr(err),
    }
}

/// Reads a serialized object with serde from the given virtual directory path.
fn from_vdir<T: serde::de::DeserializeOwned>(f: &VirtualDirectoryPath) -> Result<T, Error> {
    let path = VirtualDirectory::try_new(f).map_err(|e| Error::InvalidUrl {
        url: format!("{f}"),
        error: format!("Invalid weaver path reference: {e}"),
    })?;
    let file = std::fs::File::open(path.path()).map_err(|_| Error::InvalidSchemaPath {
        path: path.path().to_path_buf(),
    })?;
    let reader = std::io::BufReader::new(file);
    Ok(
        serde_yaml::from_reader(reader).map_err(|e| Error::ConversionError {
            message: format!("Unable to read resolved schema: {e}"),
        })?,
    )
}

/// Loads a "raw" repository (composed of the original definition).
fn load_definition_repository(
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    dependencies: Vec<LoadedSemconvRegistry>,
) -> WResult<LoadedSemconvRegistry, Error> {
    // Define helper functions for filtering files.
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
            && file_name != REGISTRY_MANIFEST
    }
    let local_path = registry_repo.path().to_path_buf();
    let registry_path_repr = registry_repo.registry_path_repr();
    let versioned_validator = JsonSchemaValidator::new_versioned();
    let unversioned_validator = JsonSchemaValidator::new_unversioned();

    // Loads the semantic convention specifications from the git repo.
    // All yaml files are recursively loaded and parsed in parallel from
    // the given path.
    let result = walkdir::WalkDir::new(local_path.clone())
        .follow_links(follow_symlinks)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .par_bridge()
        .flat_map(|entry| {
            match entry {
                Ok(entry) => {
                    if !is_semantic_convention_file(&entry) {
                        return vec![].into_par_iter();
                    }

                    // TODO - less confusing way to load semconv specs.
                    vec![SemConvRegistry::semconv_spec_from_file(
                        &registry_repo.id(),
                        entry.path(),
                        &unversioned_validator,
                        &versioned_validator,
                        |path| {
                            // Replace the local path with the git URL combined with the relative path
                            // of the semantic convention file.
                            let prefix = local_path
                                .to_str()
                                .map(|s| s.to_owned())
                                .unwrap_or_default();
                            if registry_path_repr.ends_with(MAIN_SEPARATOR) {
                                let relative_path = &path[prefix.len()..];
                                format!("{registry_path_repr}{relative_path}")
                            } else {
                                let relative_path = &path[prefix.len() + 1..];
                                format!("{registry_path_repr}/{relative_path}")
                            }
                        },
                    )]
                    .into_par_iter()
                }
                Err(e) => vec![WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                    error: e.to_string(),
                })]
                .into_par_iter(),
            }
        })
        .collect::<Vec<_>>();

    let mut non_fatal_errors = vec![];
    let mut specs = vec![];
    let mut imports = vec![];
    // Process all the results of the previous parallel processing.
    // The first fatal error will stop the processing and return the error.
    // Otherwise, all non-fatal errors will be collected and returned along
    // with the result.
    for r in result {
        match r {
            WResult::Ok(t) => specs.push(t),
            WResult::OkWithNFEs(t, nfes) => {
                specs.push(t);
                non_fatal_errors.extend(nfes);
            }
            WResult::FatalErr(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
        }
    }

    // Load imports from the specification.
    for (i, provenance) in specs.iter().filter_map(|s| {
        let v1 = s.clone().into_v1();
        v1.spec.imports().map(|i| (i.clone(), v1.provenance))
    }) {
        imports.push(ImportsWithProvenance {
            imports: i,
            provenance,
        });
    }

    // Create loaded repository, pulling imports, specs, etc.
    WResult::OkWithNFEs(
        LoadedSemconvRegistry::Unresolved {
            repo: registry_repo,
            specs,
            imports,
            dependencies,
        },
        non_fatal_errors
            .into_iter()
            .map(|e| Error::FailToResolveDefinition(e))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use weaver_common::{
        diagnostic::DiagnosticMessages, result::WResult, vdir::VirtualDirectoryPath,
    };
    use weaver_semconv::registry_repo::RegistryRepo;

    use crate::{
        loader::{load_semconv_repository, load_semconv_repository_recursive},
        Error, LoadedSemconvRegistry,
    };

    #[test]
    fn test_load_unresolved_registry_with_dependencies() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("main", &registry_path)?;
        let mut diag_msgs = DiagnosticMessages::empty();
        let loaded = load_semconv_repository(registry_repo, false)
            .capture_non_fatal_errors(&mut diag_msgs)?;
        // Assert that we've loaded the ACME repository and the dependency of OTEL.
        if let LoadedSemconvRegistry::Unresolved {
            repo,
            specs,
            imports,
            dependencies,
        } = loaded
        {
            assert_eq!("acme", repo.id().as_ref());
            assert_eq!(dependencies.len(), 1);
            assert_eq!(specs.len(), 1);
            assert_eq!(imports.len(), 1);
            if let &[LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                dependencies,
            }] = &dependencies.as_slice()
            {
                assert_eq!("otel", repo.id().as_ref());
                assert_eq!(dependencies.len(), 0);
                assert_eq!(specs.len(), 1);
                assert_eq!(imports.len(), 0);
            } else {
                panic!("Failed to load unresolved registry dependency")
            }
        } else {
            panic!("Failed to load unresolved registry")
        }
        Ok(())
    }

    #[test]
    fn test_depth_limit_enforcement() -> Result<(), weaver_semconv::Error> {
        // Test that depth limit is properly enforced by using internal method
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("app", &registry_path)?;

        // Try with depth limit of 1 - should fail at acme->otel transition
        let mut visited_registries = HashSet::new();
        let mut dependency_chain = Vec::new();
        let result = load_semconv_repository_recursive(
            registry_repo,
            true,
            1,
            &mut visited_registries,
            &mut dependency_chain,
        );

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Maximum dependency depth reached"),
                    "Expected depth limit error, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to depth limit, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_circular_dependency_detection() -> Result<(), weaver_semconv::Error> {
        // Test circular dependency: registry_a -> registry_b -> registry_a
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/circular-registry-test/registry_a".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("registry_a", &registry_path)?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Circular dependency detected") && 
                    error_msg.contains("registry_a") &&
                    error_msg.contains("registry_b"),
                    "Expected circular dependency error mentioning both registries, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to circular dependency, but got success");
            }
        }

        Ok(())
    }
}
