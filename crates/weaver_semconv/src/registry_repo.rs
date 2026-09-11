// SPDX-License-Identifier: Apache-2.0

//! A Semantic Convention Repository abstraction for OTel Weaver.

use std::default::Default;
use std::path::{Path, PathBuf};

use crate::schema_url::SchemaUrl;
use crate::v1::manifest::RegistryManifest as V1RegistryManifest;
use crate::v2::manifest::{Dependency, RegistryManifest as V2RegistryManifest};
use crate::Error;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_common::{get_path_type, log_info};

/// The name of the legacy registry manifest file.
#[deprecated(note = "The registry manifest file is renamed to `manifest.yaml`.")]
pub(crate) const LEGACY_REGISTRY_MANIFEST: &str = "registry_manifest.yaml";

/// The name of the registry manifest file.
pub(crate) const REGISTRY_MANIFEST: &str = "manifest.yaml";

/// Returns true if the given path represents a semantic convention file.
///
/// A path is considered a semantic convention file if:
/// - It is a regular file
/// - It has a YAML extension (`.yaml` or `.yml`)
/// - It is not a manifest file (`manifest.yaml` or legacy `registry_manifest.yaml`)
/// - It is not a resolved schema artifact (`schema-next.yaml`)
#[must_use]
pub fn is_semantic_convention_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    let extension = path.extension().unwrap_or_default();
    let file_name = path.file_name().unwrap_or_default();
    #[allow(deprecated)]
    let is_manifest = file_name == REGISTRY_MANIFEST || file_name == LEGACY_REGISTRY_MANIFEST;
    path.is_file()
        && (extension == "yaml" || extension == "yml")
        && file_name != "schema-next.yaml"
        && !is_manifest
}

/// A versioned semantic convention registry manifest, either V1 or V2.
#[derive(Debug, Clone)]
pub enum VersionedManifest {
    /// A version 1 registry manifest (`registry_manifest.yaml`).
    V1(V1RegistryManifest),
    /// A version 2 registry manifest (`manifest.yaml`).
    V2(V2RegistryManifest),
}

impl VersionedManifest {
    /// Attempts to load a registry manifest from a [`ManifestPath`].
    ///
    /// - If the manifest path is [`ManifestPath::LegacyPath`], it parses as a V1 manifest.
    /// - If the manifest path is [`ManifestPath::RegistryPath`], it parses according to `file_format`:
    ///   - `file_format: manifest/2.0` parses as a V2 publication manifest.
    ///   - `file_format: definition_manifest/2.0` parses as a V2 definition manifest.
    ///   - missing `file_format` is assumed to be a V2 definition manifest and issues a warning.
    pub fn try_from_file(
        manifest_path: &ManifestPath,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        match manifest_path {
            ManifestPath::LegacyPath(path) => {
                V1RegistryManifest::try_from_file(path, nfes).map(VersionedManifest::V1)
            }
            ManifestPath::RegistryPath(path) => {
                V2RegistryManifest::try_from_file(path, nfes).map(VersionedManifest::V2)
            }
            ManifestPath::None => Err(Error::RegistryManifestNotFound {
                path: PathBuf::new(),
            }),
        }
    }

    /// Attempts to load a registry manifest from a file or directory path.
    pub fn try_from_path<P: AsRef<Path>>(path: P, nfes: &mut Vec<Error>) -> Result<Self, Error> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::RegistryManifestNotFound {
                path: path.to_path_buf(),
            });
        }
        let manifest_path = ManifestPath::find(path);
        Self::try_from_file(&manifest_path, nfes)
    }

    /// Returns the schema URL of the registry.
    #[must_use]
    pub fn schema_url(&self) -> &SchemaUrl {
        match self {
            VersionedManifest::V1(v1) => v1.schema_url(),
            VersionedManifest::V2(v2) => v2.schema_url(),
        }
    }

    /// Returns the description of the registry, if present.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            VersionedManifest::V1(v1) => v1.description(),
            VersionedManifest::V2(v2) => v2.description(),
        }
    }

    /// Returns the dependencies converted to V2 dependencies.
    #[must_use]
    pub fn dependencies(&self) -> Vec<Dependency> {
        match self {
            VersionedManifest::V1(v1) => v1
                .dependencies()
                .iter()
                .cloned()
                .map(crate::convert::v1_dependency_to_v2)
                .collect(),
            VersionedManifest::V2(v2) => v2.dependencies().to_vec(),
        }
    }

    /// Returns a reference to the V1 manifest, if this is a V1 manifest.
    #[must_use]
    pub fn as_v1(&self) -> Option<&V1RegistryManifest> {
        match self {
            VersionedManifest::V1(v1) => Some(v1),
            VersionedManifest::V2(_) => None,
        }
    }

    /// Returns a reference to the V2 manifest, if this is a V2 manifest.
    #[must_use]
    pub fn as_v2(&self) -> Option<&V2RegistryManifest> {
        match self {
            VersionedManifest::V1(_) => None,
            VersionedManifest::V2(v2) => Some(v2),
        }
    }

    /// Converts this manifest to a V1 manifest.
    #[must_use]
    pub fn to_v1(&self) -> V1RegistryManifest {
        match self {
            VersionedManifest::V1(v1) => v1.clone(),
            VersionedManifest::V2(v2) => crate::convert::v2_manifest_to_v1(v2.clone()),
        }
    }

    /// Converts this manifest to a V2 manifest.
    pub fn to_v2(&self) -> Result<V2RegistryManifest, Error> {
        match self {
            VersionedManifest::V1(v1) => crate::convert::v1_manifest_to_v2(v1.clone()),
            VersionedManifest::V2(v2) => Ok(v2.clone()),
        }
    }
}

/// The detected manifest path kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestPath {
    /// Found the legacy path (`registry_manifest.yaml`), which parses as a V1 registry.
    LegacyPath(PathBuf),
    /// Found the new path (`manifest.yaml` or directly specified file), which checks for `file_format`.
    RegistryPath(PathBuf),
    /// Did not find a registry path at all.
    None,
}

impl ManifestPath {
    /// Discovers and classifies the manifest path for a given file or directory.
    #[must_use]
    pub fn find(registry_path: &Path) -> Self {
        find_manifest_path(registry_path)
    }

    /// Returns the path to the manifest, if one was found.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            ManifestPath::LegacyPath(p) | ManifestPath::RegistryPath(p) => Some(p),
            ManifestPath::None => None,
        }
    }

    /// Attempts to load the manifest from this [`ManifestPath`].
    pub fn try_into_manifest(&self, nfes: &mut Vec<Error>) -> Result<VersionedManifest, Error> {
        VersionedManifest::try_from_file(self, nfes)
    }
}

/// Finds the path to the manifest file, which could be:
/// - directly the path to the manifest file, or
/// - `manifest.yaml` in the given directory (`RegistryPath`), or
/// - `registry_manifest.yaml` in the given directory (`LegacyPath`), or
/// - `None` otherwise.
fn find_manifest_path(registry_path: &Path) -> ManifestPath {
    // First check to see if we're pointing at a manifest directly.
    if registry_path.is_file() {
        // Check if the file itself has the legacy filename.
        #[allow(deprecated)]
        let is_legacy = registry_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == LEGACY_REGISTRY_MANIFEST);
        if is_legacy {
            return ManifestPath::LegacyPath(registry_path.to_path_buf());
        }
        return ManifestPath::RegistryPath(registry_path.to_path_buf());
    }

    let manifest_path = registry_path.join(REGISTRY_MANIFEST);
    #[allow(deprecated)]
    let legacy_path = registry_path.join(LEGACY_REGISTRY_MANIFEST);
    if manifest_path.exists() {
        log_info(format!(
            "Found registry manifest: {}",
            manifest_path.display()
        ));
        ManifestPath::RegistryPath(manifest_path)
    } else if legacy_path.exists() {
        log_info(format!(
            "Found registry manifest: {}",
            legacy_path.display()
        ));
        ManifestPath::LegacyPath(legacy_path)
    } else {
        log_info(format!(
            "No registry manifest found: {}",
            manifest_path.display()
        ));
        ManifestPath::None
    }
}

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
    manifest: Option<VersionedManifest>,

    // Cached path to the manifest file (if it exists).
    manifest_path: Option<PathBuf>,
}

impl RegistryRepo {
    /// Build a `RegistryRepo` from a `Dependency` with no HTTP credentials.
    /// For private remote registries, use [`Self::try_new_dependency_with_auth`].
    pub fn try_new_dependency(
        dependency: &Dependency,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        Self::try_new_dependency_with_auth(dependency, nfes, &HttpAuthResolver::empty())
    }

    /// Build a `RegistryRepo` from a `Dependency`, resolving credentials via `auth`.
    pub fn try_new_dependency_with_auth(
        dependency: &Dependency,
        nfes: &mut Vec<Error>,
        auth: &HttpAuthResolver,
    ) -> Result<Self, Error> {
        let path = dependency.registry_path.clone().unwrap_or_else(|| {
            // If no registry path is provided, we assume it's the same schema_url.
            VirtualDirectoryPath::RemoteArchive {
                url: dependency.schema_url.to_string(),
                sub_folder: None,
            }
        });
        Self::try_new_with_auth(Some(dependency.schema_url.clone()), &path, nfes, auth)
    }

    /// Build a `RegistryRepo` at `registry_path` with no HTTP credentials.
    /// If there is no manifest and no schema URL, registry name/version are "unknown".
    /// For private remote registries, use [`Self::try_new_with_auth`].
    pub fn try_new(
        schema_url: Option<SchemaUrl>,
        registry_path: &VirtualDirectoryPath,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        Self::try_new_with_auth(schema_url, registry_path, nfes, &HttpAuthResolver::empty())
    }

    /// Build a `RegistryRepo` at `registry_path`, resolving credentials via `auth`.
    pub fn try_new_with_auth(
        schema_url: Option<SchemaUrl>,
        registry_path: &VirtualDirectoryPath,
        nfes: &mut Vec<Error>,
        auth: &HttpAuthResolver,
    ) -> Result<Self, Error> {
        let registry = VirtualDirectory::try_new_with_auth(registry_path, auth)
            .map_err(Error::VirtualDirectoryError)?;
        // Try to load manifest
        let manifest_path = ManifestPath::find(registry.path());
        let (manifest, schema_url, manifest_path_buf) = match manifest_path {
            ManifestPath::LegacyPath(ref path) | ManifestPath::RegistryPath(ref path) => {
                let manifest = VersionedManifest::try_from_file(&manifest_path, nfes)?;
                let schema_url = manifest.schema_url().clone();
                (Some(manifest), schema_url, Some(path.clone()))
            }
            ManifestPath::None => {
                // No manifest
                let schema_url_combined = schema_url.unwrap_or_else(SchemaUrl::new_unknown);
                (None, schema_url_combined, None)
            }
        };

        Ok(Self {
            schema_url,
            registry,
            manifest,
            manifest_path: manifest_path_buf,
        })
    }

    /// Returns the registry name (from manifest if present, otherwise top-level field).
    #[must_use]
    pub fn name(&self) -> &str {
        self.schema_url.name()
    }

    /// Returns the registry version (from manifest if present, otherwise top-level field).
    #[must_use]
    pub fn version(&self) -> &str {
        self.schema_url.version()
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
    pub fn manifest(&self) -> Option<&VersionedManifest> {
        self.manifest.as_ref()
    }

    /// Returns the manifest as a V1 manifest, converting if it was loaded as V2.
    #[must_use]
    pub fn v1_manifest(&self) -> Option<V1RegistryManifest> {
        self.manifest.as_ref().map(|m| m.to_v1())
    }

    /// Returns the manifest as a V2 manifest, converting if it was loaded as V1.
    #[must_use]
    pub fn v2_manifest(&self) -> Option<Result<V2RegistryManifest, Error>> {
        self.manifest.as_ref().map(|m| m.to_v2())
    }

    /// Converts the registry repo's manifest into a V2 manifest.
    ///
    /// - If a manifest is present, it is converted to a V2 manifest.
    /// - If no manifest is present, a default V2 definition manifest is constructed
    ///   from the repository's schema URL. If the schema URL cannot be resolved
    ///   from the repository name and version, returns [`Error::FailToResolveSchemaUrl`].
    pub fn to_v2_manifest(&self) -> Result<V2RegistryManifest, Error> {
        if let Some(manifest) = self.manifest.as_ref() {
            manifest.to_v2()
        } else {
            let schema_url = SchemaUrl::try_from_name_version(self.name(), self.version())
                .map_err(|_| Error::FailToResolveSchemaUrl {})?;
            Ok(V2RegistryManifest::Definition(
                crate::v2::manifest::DefinitionRegistryManifest::from_schema_url(schema_url),
            ))
        }
    }

    /// Returns the resolved registry URI, if available in the manifest.
    #[must_use]
    pub fn resolved_registry_uri(&self) -> Option<VirtualDirectoryPath> {
        let manifest = self.manifest.as_ref()?;
        let registry_uri: &str = match manifest {
            VersionedManifest::V2(V2RegistryManifest::Publication(m)) => &m.resolved_registry_uri,
            VersionedManifest::V2(V2RegistryManifest::Definition(_)) | VersionedManifest::V1(_) => {
                return None
            }
        };
        match get_path_type(registry_uri) {
            weaver_common::PathType::RelativePath => {
                // We need to understand if the manifest URI is the same as the registry URI.
                let vdir_was_manifest_file = self
                    .manifest_path
                    .clone()
                    .is_some_and(|mp| mp == self.registry.path());
                Some(self.registry.vdir_path().map_sub_folder(|path| {
                    if vdir_was_manifest_file {
                        match Path::new(&path).parent() {
                            Some(parent) => format!("{}/{registry_uri}", parent.display()),
                            None => "".to_owned(),
                        }
                    } else {
                        format!("{path}/{registry_uri}")
                    }
                }))
            }
            _ => registry_uri.try_into().ok(),
        }
    }

    /// Returns the registry schema URL.
    #[must_use]
    pub fn schema_url(&self) -> &SchemaUrl {
        &self.schema_url
    }
}

impl Default for RegistryRepo {
    fn default() -> Self {
        Self {
            schema_url: SchemaUrl::new_unknown(),
            registry: VirtualDirectory::default(),
            manifest: None,
            manifest_path: None,
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
    fn test_versioned_manifest_discrimination() {
        let dir = tempfile::tempdir().unwrap();

        // 1. Filename is registry_manifest.yaml -> ALWAYS parses as V1 manifest
        #[allow(deprecated)]
        let v1_path = dir.path().join(LEGACY_REGISTRY_MANIFEST);
        std::fs::write(
            &v1_path,
            "schema_url: https://example.com/schemas/1.0.0\ndescription: v1 test\n",
        )
        .unwrap();
        let mut nfes = vec![];
        let v1_manifest_path = ManifestPath::find(&v1_path);
        let v1 = VersionedManifest::try_from_file(&v1_manifest_path, &mut nfes).unwrap();
        assert!(matches!(v1, VersionedManifest::V1(_)));
        assert_eq!(
            v1.schema_url().as_str(),
            "https://example.com/schemas/1.0.0"
        );
        assert_eq!(v1.description(), Some("v1 test"));
        assert!(v1.to_v2().is_ok());
        assert!(VersionedManifest::try_from_path(&v1_path, &mut vec![]).is_ok());

        // 2. Filename is manifest.yaml with missing file_format -> Assumed V2 definition manifest + warning
        let v2_empty_path = dir.path().join(REGISTRY_MANIFEST);
        std::fs::write(
            &v2_empty_path,
            "schema_url: https://example.com/schemas/2.0.0\ndescription: v2 empty test\n",
        )
        .unwrap();
        let mut nfes_empty = vec![];
        let v2_empty_manifest_path = ManifestPath::find(&v2_empty_path);
        let v2_empty =
            VersionedManifest::try_from_file(&v2_empty_manifest_path, &mut nfes_empty).unwrap();
        assert!(matches!(
            v2_empty,
            VersionedManifest::V2(V2RegistryManifest::Definition(_))
        ));
        assert!(
            nfes_empty
                .iter()
                .any(|w| matches!(w, Error::MissingManifestFileFormat { .. })),
            "Expected MissingManifestFileFormat warning, got: {nfes_empty:?}"
        );

        // 3. V2 definition manifest with definition_manifest/2.0
        let v2_def_path = dir.path().join("v2_def_manifest.yaml");
        std::fs::write(
            &v2_def_path,
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/schemas/2.0.0\ndescription: v2 test\n",
        )
        .unwrap();
        let v2_def_manifest_path = ManifestPath::find(&v2_def_path);
        let v2_def = VersionedManifest::try_from_file(&v2_def_manifest_path, &mut nfes).unwrap();
        assert!(matches!(
            v2_def,
            VersionedManifest::V2(V2RegistryManifest::Definition(_))
        ));
        assert_eq!(
            v2_def.schema_url().as_str(),
            "https://example.com/schemas/2.0.0"
        );
        assert_eq!(v2_def.description(), Some("v2 test"));
        let v1_from_v2 = v2_def.to_v1();
        assert_eq!(
            v1_from_v2.schema_url().as_str(),
            "https://example.com/schemas/2.0.0"
        );

        // 4. V2 publication manifest with manifest/2.0
        let v2_pub_path = dir.path().join("v2_pub_manifest.yaml");
        std::fs::write(
            &v2_pub_path,
            "file_format: manifest/2.0\nschema_url: https://example.com/schemas/2.0.0\nresolved_registry_uri: resolved.yaml\n",
        )
        .unwrap();
        let v2_pub_manifest_path = ManifestPath::find(&v2_pub_path);
        let v2_pub = VersionedManifest::try_from_file(&v2_pub_manifest_path, &mut nfes).unwrap();
        assert!(matches!(
            v2_pub,
            VersionedManifest::V2(V2RegistryManifest::Publication(_))
        ));

        // 5. Unknown file_format rejected
        let invalid_path = dir.path().join("invalid_manifest.yaml");
        std::fs::write(
            &invalid_path,
            "file_format: manifest/1.0\nschema_url: https://example.com/schemas/1.0.0\n",
        )
        .unwrap();
        let invalid_manifest_path = ManifestPath::find(&invalid_path);
        let err = VersionedManifest::try_from_file(&invalid_manifest_path, &mut nfes).unwrap_err();
        assert!(matches!(err, Error::InvalidRegistryManifest { .. }));
    }

    #[test]
    fn test_find_manifest_path() {
        let dir = tempfile::tempdir().unwrap();

        // When neither manifest exists, returns None.
        assert_eq!(find_manifest_path(dir.path()), ManifestPath::None);
        assert_eq!(ManifestPath::None.path(), None);

        // When legacy manifest exists, returns LegacyPath.
        #[allow(deprecated)]
        let legacy_file = dir.path().join(LEGACY_REGISTRY_MANIFEST);
        std::fs::write(
            &legacy_file,
            "schema_url: https://example.com/schemas/1.0.0\n",
        )
        .unwrap();
        assert_eq!(
            find_manifest_path(dir.path()),
            ManifestPath::LegacyPath(legacy_file.clone())
        );
        assert_eq!(
            find_manifest_path(&legacy_file),
            ManifestPath::LegacyPath(legacy_file.clone())
        );
        assert_eq!(
            ManifestPath::LegacyPath(legacy_file.clone()).path(),
            Some(legacy_file.as_path())
        );

        // When new manifest exists, returns RegistryPath (takes precedence over legacy).
        let new_file = dir.path().join(REGISTRY_MANIFEST);
        std::fs::write(
            &new_file,
            "file_format: definition_manifest/2.0\nschema_url: https://example.com/schemas/2.0.0\n",
        )
        .unwrap();
        assert_eq!(
            find_manifest_path(dir.path()),
            ManifestPath::RegistryPath(new_file.clone())
        );
        assert_eq!(
            find_manifest_path(&new_file),
            ManifestPath::RegistryPath(new_file.clone())
        );
        assert_eq!(
            ManifestPath::RegistryPath(new_file.clone()).path(),
            Some(new_file.as_path())
        );
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
        assert_eq!(manifest.schema_url().name(), "resolved");

        let Some(resolved_path) = repo.resolved_registry_uri() else {
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
        let Some(resolved_path) = repo.resolved_registry_uri() else {
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
        let Some(resolved_path) = repo.resolved_registry_uri() else {
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
