// SPDX-License-Identifier: Apache-2.0

use itertools::Itertools;
use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge};
use std::fmt::Display;
use std::path::MAIN_SEPARATOR;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_semconv::registry::SemConvRegistry;

use walkdir::DirEntry;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::registry_repo::{RegistryRepo, LEGACY_REGISTRY_MANIFEST, REGISTRY_MANIFEST};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::{group::ImportsWithProvenance, semconv::SemConvSpecWithProvenance};

use crate::Error;

/// Maximum allowed depth for registry dependency chains.
const MAX_DEPENDENCY_DEPTH: u32 = 10;

/// The result of loading a semantic convention URL prior to resolution.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
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
        use std::io::Write;
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_semconv::schema_url::SchemaUrl;
        let path: VirtualDirectoryPath = "data".try_into().expect("Bad fake path for test");
        let repo =
            RegistryRepo::try_new(None, &path, &mut vec![]).map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("{e}"),
            })?;
        let mut temp_file =
            tempfile::NamedTempFile::with_suffix(".yaml").map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("Failed to create temp file: {e}"),
            })?;
        temp_file
            .write_all(spec.as_bytes())
            .map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("Failed to write to temp file: {e}"),
            })?;
        let spec_with_provenance =
            SemConvSpecWithProvenance::from_file(SchemaUrl::new_unknown(), temp_file.path())
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
            LoadedSemconvRegistry::ResolvedV2(schema) => schema.schema_url.as_str(),
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
    pub fn registry_names(&self) -> Vec<String> {
        match self {
            LoadedSemconvRegistry::Unresolved {
                repo, dependencies, ..
            } => {
                let mut result = vec![repo.name().to_owned()];
                for d in dependencies {
                    result.extend(d.registry_names());
                }
                result
            }
            LoadedSemconvRegistry::Resolved(schema) => vec![schema.registry_id.to_owned()],
            LoadedSemconvRegistry::ResolvedV2(schema) => vec![schema.schema_url.name().to_owned()],
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
                repo.schema_url(),
                dependencies.iter().map(|d| format!("{d}")).join(",")
            ),
            LoadedSemconvRegistry::Resolved(schema) => write!(f, "{}", schema.schema_url),
            LoadedSemconvRegistry::ResolvedV2(schema) => write!(f, "{}", schema.schema_url),
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
    let mut visited_registries = std::collections::HashMap::new();
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
    visited_registries: &mut std::collections::HashMap<String, SchemaUrl>,
    dependency_chain: &mut Vec<String>,
) -> WResult<LoadedSemconvRegistry, Error> {
    // Make sure we don't go past our max dependency depth.
    if max_dependency_depth == 0 {
        return WResult::FatalErr(Error::MaximumDependencyDepth {
            registry_name: registry_repo.registry_path_repr().to_owned(),
        });
    }
    let registry_name = registry_repo.name().to_owned();
    let schema_url = registry_repo.schema_url().clone();

    // Check for circular dependency in the current path
    if dependency_chain.contains(&registry_name) {
        dependency_chain.push(registry_name.clone());
        let chain_str = dependency_chain.join(" → ");
        return WResult::FatalErr(Error::CircularDependency {
            registry_name: registry_name.clone(),
            chain: chain_str,
        });
    }

    // Check for conflict across the graph
    if let Some(prev_schema_url) = visited_registries.get(&registry_name) {
        if prev_schema_url != &schema_url {
            if let Err(e) =
                check_version_compatibility(&registry_name, prev_schema_url, &schema_url)
            {
                return WResult::FatalErr(e);
            }
        }
    } else {
        let _ = visited_registries.insert(registry_name.clone(), schema_url.clone());
    }

    // Add current registry to dependency chain
    dependency_chain.push(registry_name.clone());

    // Either load a fully resolved repository, or read in raw files.
    if let Some(manifest) = registry_repo.manifest() {
        if let Some(resolved_url) = registry_repo.resolved_schema_uri() {
            let res = load_resolved_repository(&resolved_url);
            let _ = dependency_chain.pop();
            res
        } else {
            // Load dependencies.
            let mut loaded_dependencies = vec![];
            let mut non_fatal_errors: Vec<Error> = vec![];
            let mut seen_dependencies: std::collections::HashMap<String, SchemaUrl> =
                std::collections::HashMap::new();

            for d in manifest.dependencies().iter() {
                let dep_name = d.schema_url.name().to_owned();

                if let Some(prev_schema_url) = seen_dependencies.get(&dep_name) {
                    if prev_schema_url != &d.schema_url {
                        if let Err(e) =
                            check_version_compatibility(&dep_name, prev_schema_url, &d.schema_url)
                        {
                            // Clean up the state of dependency_chain before erroring.
                            let _ = dependency_chain.pop();
                            return WResult::FatalErr(e);
                        }
                    }
                } else {
                    let _ = seen_dependencies.insert(dep_name, d.schema_url.clone());
                }
                let mut semconv_nfes: Vec<weaver_semconv::Error> = vec![];
                match RegistryRepo::try_new_dependency(d, &mut semconv_nfes) {
                    Ok(d_repo) => {
                        non_fatal_errors
                            .extend(semconv_nfes.into_iter().map(Error::FailToResolveDefinition));
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
                            WResult::FatalErr(err) => {
                                let _ = dependency_chain.pop();
                                return WResult::FatalErr(err);
                            }
                        }
                    }
                    Err(err) => {
                        let _ = dependency_chain.pop();
                        return WResult::FatalErr(err.into());
                    }
                }
            }
            // Now load the raw repository.
            // TODO - Allow ignoring dependency warnings - https://github.com/open-telemetry/weaver/issues/1126.
            let res =
                load_definition_repository(registry_repo, follow_symlinks, loaded_dependencies)
                    .extend_non_fatal_errors(non_fatal_errors);
            let _ = dependency_chain.pop();
            res
        }
    } else {
        // This is a raw repository with *no* manifest.
        // TODO - issue a warning that manifest will be required w/ 2.0 to allow publishing.
        let res = load_definition_repository(registry_repo, follow_symlinks, vec![]);
        let _ = dependency_chain.pop();
        res
    }
}

/// Checks version compatibility between two schema URLs.
fn check_version_compatibility(
    registry_name: &str,
    prev_schema_url: &SchemaUrl,
    schema_url: &SchemaUrl,
) -> Result<(), Error> {
    let prev_v = prev_schema_url
        .semver()
        .map_err(|e| Error::InvalidSchemaUrlBadVersion {
            url: prev_schema_url.to_string(),
            error: e.to_string(),
        })?;
    let cur_v = schema_url
        .semver()
        .map_err(|e| Error::InvalidSchemaUrlBadVersion {
            url: schema_url.to_string(),
            error: e.to_string(),
        })?;
    // TODO - Should we use `VersionReq.parse("^{major}.0.0"))?` instead?
    if prev_v.major != cur_v.major {
        return Err(Error::DuplicateDependency {
            name: registry_name.to_owned(),
            version1: prev_schema_url.version().to_owned(),
            version2: schema_url.version().to_owned(),
        });
    }
    Ok(())
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
        url: f.to_string(),
        error: format!("Invalid weaver path reference: {e}"),
    })?;
    let file = std::fs::File::open(path.path()).map_err(|_| Error::InvalidSchemaPath {
        path: path.path().to_path_buf(),
    })?;
    let reader = std::io::BufReader::new(file);
    serde_yaml::from_reader(reader).map_err(|e| Error::ConversionError {
        message: format!("Unable to read resolved schema: {e}"),
    })
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
            && file_name != LEGACY_REGISTRY_MANIFEST
    }
    let local_path = registry_repo.path().to_path_buf();
    let registry_path_repr = registry_repo.registry_path_repr();

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
                        registry_repo.schema_url().clone(),
                        entry.path(),
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
            .map(Error::FailToResolveDefinition)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use weaver_semconv::schema_url::SchemaUrl;

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
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
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
            assert_eq!("acme.com/schemas", repo.name());
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
                assert_eq!("opentelemetry.io/schemas", repo.name());
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
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;

        // Try with depth limit of 1 - should fail at acme->otel transition
        let mut visited_registries: std::collections::HashMap<String, SchemaUrl> =
            std::collections::HashMap::new();
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
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
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

    #[test]
    fn test_incompatible_version_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/incompatible-version-conflict/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Duplicate dependency") &&
                    error_msg.contains("example.com/c") &&
                    error_msg.contains("1.0.0") &&
                    error_msg.contains("2.0.0"),
                    "Expected duplicate dependency error mentioning both versions, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to duplicate dependency, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_compatible_version_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {
                // Success is expected now that compatible versions are allowed.
            }
            WResult::FatalErr(fatal) => {
                panic!("Expected success, but got fatal error: {fatal}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_duplicate_dependency_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/duplicate-dependency/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Duplicate dependency") &&
                    error_msg.contains("test/dep") &&
                    error_msg.contains("1.0.0") &&
                    error_msg.contains("2.0.0"),
                    "Expected duplicate dependency error mentioning both versions, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to duplicate dependency, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_dependency_not_found() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/dependency-not-found/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                // TODO: Use typed errors instead of string matching when weaver_semconv exposes them.
                assert!(
                    error_msg.contains("Failed to resolve definition")
                        || error_msg.contains("No such file or directory")
                        || error_msg.contains("The system cannot find the file specified"),
                    "Expected file not found error, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to missing dependency, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_invalid_version_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/invalid-version-conflict/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(registry_repo, true);

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Invalid schema URL")
                        && error_msg.contains("http://test/dep/v1"),
                    "Expected invalid schema URL error, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to invalid version, but got success");
            }
        }

        Ok(())
    }
}
