// SPDX-License-Identifier: Apache-2.0

//! A Semantic Convention Repository abstraction for OTel Weaver.

use std::default::Default;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::manifest::RegistryManifest;
use crate::Error;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_common::{get_path_type, log_info};

/// The name of the registry manifest file.
pub const REGISTRY_MANIFEST: &str = "registry_manifest.yaml";

/// A semantic convention registry repository that can be:
/// - A definition repository, which is one of:
///   - A simple wrapper around a local directory
///   - Initialized from a Git repository
///   - Initialized from a Git archive
/// - A published repository, which is a manifest file
///   that denotes where to find aspects of the registry.
#[derive(Default, Debug, Clone)]
pub struct RegistryRepo {
    // A unique identifier for the registry (e.g. main, baseline, etc.)
    id: Arc<str>,

    // A virtual directory containing the registry.
    registry: VirtualDirectory,

    // The registry manifest definition.
    manifest: Option<RegistryManifest>,
}

impl RegistryRepo {
    /// Creates a new `RegistryRepo` from a `RegistryPath` object that
    /// specifies the location of the registry.
    pub fn try_new(
        registry_id_if_no_manifest: &str,
        registry_path: &VirtualDirectoryPath,
    ) -> Result<Self, Error> {
        let mut registry_repo = Self {
            id: Arc::from(registry_id_if_no_manifest),
            registry: VirtualDirectory::try_new(registry_path)
                .map_err(Error::VirtualDirectoryError)?,
            manifest: None,
        };
        if let Some(manifest) = registry_repo.manifest_path() {
            let registry_manifest = RegistryManifest::try_from_file(manifest)?;
            registry_repo.id = Arc::from(registry_manifest.name.as_str());
            registry_repo.manifest = Some(registry_manifest);
        }
        Ok(registry_repo)
    }

    /// Returns the unique identifier for the registry.
    #[must_use]
    pub fn id(&self) -> Arc<str> {
        self.id.clone()
    }

    /// Returns the local path to the semconv registry.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.registry.path()
    }

    /// Returns the registry path textual representation.
    #[must_use]
    pub fn registry_path_repr(&self) -> &str {
        self.registry.vdir_path()
    }

    /// Returns the registry manifest specified in the registry repo.
    #[must_use]
    pub fn manifest(&self) -> Option<&RegistryManifest> {
        self.manifest.as_ref()
    }

    /// Returns the resolved schema URL, if available in the manifest.
    #[must_use]
    pub fn resolved_schema_url(&self) -> Option<String> {
        let manifest_path = self.manifest_path()?;
        let manifest = self.manifest.as_ref()?;
        let resolved_url = manifest.resolved_schema_url.as_ref()?;
        match get_path_type(resolved_url) {
            weaver_common::PathType::RelativePath => {
                // Fix up relative paths to be absolute.
                let parent = manifest_path.parent()?.to_path_buf();
                Some(format!("{}", parent.join(resolved_url).display()))
            }
            _ => Some(resolved_url.to_owned()),
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
        if manifest_path.exists() {
            log_info(format!(
                "Found registry manifest: {}",
                manifest_path.display()
            ));
            Some(manifest_path)
        } else {
            log_info(format!(
                "No registry manifest found: {}",
                manifest_path.display()
            ));
            None
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
        let repo = RegistryRepo::try_new("main", &registry_path).unwrap();
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
        let repo =
            RegistryRepo::try_new("main", &registry_path).expect("Failed to load test repository.");
        let Some(manifest) = repo.manifest() else {
            panic!("Did not resolve manifest for repo: {repo:?}");
        };
        assert_eq!(manifest.name, "resolved");

        let Some(resolved_path) = repo.resolved_schema_url() else {
            panic!(
                "Should find a resolved schema path from manfiest in {}",
                repo.registry_path_repr()
            );
        };
        assert_eq!(
            "tests/published_repository/resolved/resolved_1.0.0.yaml",
            resolved_path
        );

        // Now make sure a different repository with full URL works too.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "tests/published_repository/resolved/2.0.0".to_owned(),
        };
        let repo =
            RegistryRepo::try_new("main", &registry_path).expect("Failed to load test repository.");
        let Some(resolved_path) = repo.resolved_schema_url() else {
            panic!(
                "Should find a resolved schema path from manfiest in {}",
                repo.registry_path_repr()
            );
        };
        assert_eq!("https://github.com/open-telemetry/weaver.git\\creates/weaver_semconv/tests/published_respository/resolved/resolved_2.0.0", resolved_path);
    }
}
