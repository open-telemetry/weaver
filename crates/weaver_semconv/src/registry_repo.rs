// SPDX-License-Identifier: Apache-2.0

//! A Semantic Convention Repository abstraction for OTel Weaver.

use std::default::Default;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::manifest::{Dependency, RegistryManifest};
use crate::schema_url::SchemaUrl;
use crate::Error;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_common::{get_path_type, log_info};

/// The name of the legacy registry manifest file.
#[deprecated(note = "The registry manifest file is renamed to `manifest.yaml`.")]
pub const LEGACY_REGISTRY_MANIFEST: &str = "registry_manifest.yaml";

/// The name of the registry manifest file.
pub const REGISTRY_MANIFEST: &str = "manifest.yaml";

/// A semantic convention registry repository that can be:
/// - A definition repository, which is one of:
///   - A simple wrapper around a local directory
///   - Initialized from a Git repository
///   - Initialized from a Git archive
/// - A published repository, which is a manifest file
///   that denotes where to find aspects of the registry.
#[derive(Debug, Clone)]
pub struct RegistryRepo {
    /// The schema URL associated with the registry
    /// May be derived from the manifest or the registry name and version if the manifest is not present.
    schema_url: SchemaUrl,

    // A virtual directory containing the registry.
    registry: VirtualDirectory,

    // The registry manifest definition.
    manifest: Option<RegistryManifest>,
}

impl RegistryRepo {
    /// Creates a new `RegistryRepo` from a `Dependency` object that specifies the schema URL and path.
    pub fn try_new_dependency(
        dependency: &Dependency,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        let path = dependency.registry_path.clone().unwrap_or_else(|| {
            // If no registry path is provided, we assume it's the same schema_url.
            VirtualDirectoryPath::RemoteArchive {
                url: dependency.schema_url.to_string(),
                sub_folder: None,
            }
        });
        Self::try_new(Some(dependency.schema_url.clone()), &path, nfes)
    }

    /// Creates a new `RegistryRepo` from a schema URL and `RegistryPath` object that
    /// specifies the location of the registry.
    /// If there is no manifest and schema URL is not provided, registry
    /// name and version are set to "unknown".
    pub fn try_new(
        schema_url: Option<SchemaUrl>,
        registry_path: &VirtualDirectoryPath,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        let registry =
            VirtualDirectory::try_new(registry_path).map_err(Error::VirtualDirectoryError)?;
        // Try to load manifest
        if let Some(manifest_path) = {
            // We need a temporary RegistryRepo to call manifest_path
            let temp_repo = Self {
                schema_url: SchemaUrl::new_unknown(),
                registry: registry.clone(),
                manifest: None,
            };
            temp_repo.manifest_path()
        } {
            let registry_manifest = RegistryManifest::try_from_file(manifest_path, nfes)?;
            Ok(Self {
                schema_url: registry_manifest.schema_url.clone(),
                registry,
                manifest: Some(registry_manifest),
            })
        } else {
            // No manifest
            let schema_url_combined = schema_url.unwrap_or_else(SchemaUrl::new_unknown);
            Ok(Self {
                schema_url: schema_url_combined.clone(),
                registry,
                manifest: None,
            })
        }
    }

    /// Returns the registry name (from manifest if present, otherwise top-level field).
    #[must_use]
    pub fn name(&self) -> Arc<str> {
        self.schema_url.name().into()
    }

    /// Returns the registry version (from manifest if present, otherwise top-level field).
    #[must_use]
    pub fn version(&self) -> Arc<str> {
        self.schema_url.version().into()
    }

    /// Returns the local path to the semconv registry.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.registry.path()
    }

    /// Returns the registry path textual representation.
    #[must_use]
    pub fn registry_path_repr(&self) -> &str {
        self.registry.vdir_path_str()
    }

    /// Returns the registry manifest specified in the registry repo.
    #[must_use]
    pub fn manifest(&self) -> Option<&RegistryManifest> {
        self.manifest.as_ref()
    }

    /// Returns the resolved schema URI, if available in the manifest.
    #[must_use]
    pub fn resolved_schema_uri(&self) -> Option<VirtualDirectoryPath> {
        let manifest = self.manifest.as_ref()?;
        let resolved_uri: &str = manifest.resolved_schema_uri.as_ref()?;
        match get_path_type(resolved_uri) {
            weaver_common::PathType::RelativePath => {
                // We need to understand if the manifest URI is the same as the registry URI.
                let vdir_was_manifest_file = self.manifest_path()? == self.registry.path();
                Some(self.registry.vdir_path().map_sub_folder(|path| {
                    if vdir_was_manifest_file {
                        match Path::new(&path).parent() {
                            Some(parent) => format!("{}/{resolved_uri}", parent.display()),
                            None => "".to_owned(),
                        }
                    } else {
                        format!("{path}/{resolved_uri}")
                    }
                }))
            }
            _ => resolved_uri.try_into().ok(),
        }
    }

    /// Returns the path to the `registry_manifest.yaml` file (if any).
    #[must_use]
    pub fn manifest_path(&self) -> Option<PathBuf> {
        // First check to see if we're pointing at a manifest.
        if self.registry.path().is_file() {
            // The VirtualDirectory *is* the registry.
            return Some(self.registry.path().to_path_buf());
        }
        let manifest_path = self.registry.path().join(REGISTRY_MANIFEST);
        let legacy_path = self.registry.path().join(LEGACY_REGISTRY_MANIFEST);
        if manifest_path.exists() {
            log_info(format!(
                "Found registry manifest: {}",
                manifest_path.display()
            ));
            Some(manifest_path)
        } else if legacy_path.exists() {
            log_info(format!(
                "Found registry manifest: {}",
                legacy_path.display()
            ));
            Some(legacy_path)
        } else {
            log_info(format!(
                "No registry manifest found: {}",
                manifest_path.display()
            ));
            None
        }
    }

    /// Returns the registry schema URL.
    #[must_use]
    pub fn schema_url(&self) -> SchemaUrl {
        self.schema_url.clone()
    }
}

impl Default for RegistryRepo {
    fn default() -> Self {
        Self {
            schema_url: SchemaUrl::new_unknown(),
            registry: VirtualDirectory::default(),
            manifest: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_common::vdir::VirtualDirectoryPath;

    fn count_yaml_files(repo_path: &Path) -> usize {
        let count = walkdir::WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml"))
            .count();
        count
    }

    #[test]
    fn test_semconv_registry_local_repo() {
        // A RegistryRepo created from a local folder.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "../../crates/weaver_codegen_test/semconv_registry".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![]).unwrap();
        let repo_path = repo.path().to_path_buf();
        assert!(repo_path.exists());
        assert!(
            count_yaml_files(&repo_path) > 0,
            "There should be at least one `.yaml` file in the repo"
        );
        // Simulate a RegistryRepo going out of scope.
        drop(repo);
        // The local folder should not be deleted.
        assert!(repo_path.exists());
    }

    #[test]
    fn test_resolved_registry_path() {
        // A RegistryRepo created from a local folder.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "tests/published_repository/resolved/1.0.0".to_owned(),
        };

        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to load test repository.");

        let Some(manifest) = repo.manifest() else {
            panic!("Did not resolve manifest for repo: {repo:?}");
        };
        assert_eq!(manifest.name(), "resolved");

        let Some(resolved_path) = repo.resolved_schema_uri() else {
            panic!(
                "Should find a resolved schema path from manifest in {}",
                repo.registry_path_repr()
            );
        };
        assert_eq!(
            "tests/published_repository/resolved/resolved_1.0.0.yaml",
            format!("{resolved_path}")
        );

        // Now make sure a different repository with full URL works too.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "tests/published_repository/resolved/2.0.0".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to load test repository.");
        let Some(resolved_path) = repo.resolved_schema_uri() else {
            panic!(
                "Should find a resolved schema path from manifest in {}",
                repo.registry_path_repr()
            );
        };
        assert_eq!("https://github.com/open-telemetry/weaver.git\\creates/weaver_semconv/tests/published_respository/resolved/resolved_2.0.0", format!("{resolved_path}"));

        // Now make sure when we publish a directory, we can find relative files in it.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "tests/published_repository/3.0.0".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to load test repository.");
        let Some(resolved_path) = repo.resolved_schema_uri() else {
            panic!(
                "Should find a resolved schema path from manifest in {}",
                repo.registry_path_repr()
            );
        };
        assert_eq!(
            "tests/published_repository/3.0.0/resolved_schema.yaml",
            format!("{resolved_path}")
        );
    }
}
