// SPDX-License-Identifier: Apache-2.0

//! A Semantic Convention Repository abstraction for OTel Weaver.

use std::default::Default;
use std::fs::{create_dir_all, File};
use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::clone::PrepareFetch;
use gix::create::Kind;
use gix::remote::fetch::Shallow;
use gix::{create, open, progress};
use miette::Diagnostic;
use serde::Serialize;
use tempdir::TempDir;
use url::Url;

use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};

use crate::registry_path::RegistryPath;
use crate::Error::{GitError, InvalidRegistryArchive, UnsupportedRegistryArchive};

pub mod registry_path;

/// The extension for a tar gz archive.
const TAR_GZ_EXT: &str = ".tar.gz";
/// The extension for a zip archive.
const ZIP_EXT: &str = ".zip";
/// The name of the registry manifest file.
const REGISTRY_MANIFEST: &str = "registry_manifest.yaml";

/// An error that can occur while creating or using a cache.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Home directory not found.
    #[error("Home directory not found")]
    HomeDirNotFound,

    /// Cache directory not created.
    #[error("Cache directory not created: {message}")]
    CacheDirNotCreated {
        /// The error message
        message: String,
    },

    /// Git repo not created.
    #[error("Git repo `{repo_url}` not created: {message}")]
    GitRepoNotCreated {
        /// The git repo URL
        repo_url: String,
        /// The error message
        message: String,
    },

    /// A git error occurred.
    #[error("Git error occurred while cloning `{repo_url}`: {message}")]
    GitError {
        /// The git repo URL
        repo_url: String,
        /// The error message
        message: String,
    },

    /// An invalid registry path.
    #[error("The registry path `{path}` is invalid: {error}")]
    InvalidRegistryPath {
        /// The registry path
        path: String,
        /// The error message
        error: String,
    },

    /// An invalid registry archive.
    #[error("This archive `{archive}` is not supported. Supported formats are: .tar.gz, .zip")]
    UnsupportedRegistryArchive {
        /// The registry archive path
        archive: String,
    },

    /// An invalid registry archive.
    #[error("The registry archive `{archive}` is invalid: {error}")]
    InvalidRegistryArchive {
        /// The registry archive path
        archive: String,
        /// The error message
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// A semantic convention registry repository that can be:
/// - A simple wrapper around a local directory
/// - Initialized from a Git repository
/// - Initialized from a Git archive
#[derive(Default, Debug)]
pub struct RegistryRepo {
    // A unique identifier for the registry (e.g. main, baseline, etc.)
    id: String,
    registry_path: String,
    path: PathBuf,
    // Need to keep the tempdir live for the lifetime of the RegistryRepo.
    #[allow(dead_code)]
    tmp_dir: Option<TempDir>,
}

impl RegistryRepo {
    /// Creates a new `RegistryRepo` from a `RegistryPath` object that
    /// specifies the location of the registry.
    pub fn try_new(id: &str, registry_path: &RegistryPath) -> Result<Self, Error> {
        let registry_path_repr = registry_path.to_string();
        match registry_path {
            RegistryPath::LocalFolder { path } => Ok(Self {
                id: id.to_owned(),
                registry_path: registry_path_repr,
                path: path.into(),
                tmp_dir: None,
            }),
            RegistryPath::GitRepo {
                url, sub_folder, ..
            } => Self::try_from_git_url(id, url, sub_folder, registry_path_repr),
            RegistryPath::LocalArchive { path, sub_folder } => {
                // Create a temporary directory for the repo that will be deleted
                // when the RegistryRepo goes out of scope.
                let tmp_dir = Self::create_tmp_repo()?;
                Self::try_from_local_archive(
                    id,
                    path,
                    sub_folder.as_ref(),
                    tmp_dir,
                    registry_path_repr,
                )
            }
            RegistryPath::RemoteArchive { url, sub_folder } => {
                // Create a temporary directory for the repo that will be deleted
                // when the RegistryRepo goes out of scope.
                let tmp_dir = Self::create_tmp_repo()?;
                Self::try_from_remote_archive(
                    id,
                    url,
                    sub_folder.as_ref(),
                    tmp_dir,
                    registry_path_repr,
                )
            }
        }
    }

    /// Creates a new `RegistryRepo` from a Git URL.
    fn try_from_git_url(
        id: &str,
        url: &str,
        sub_folder: &Option<String>,
        registry_path: String,
    ) -> Result<Self, Error> {
        let tmp_dir = Self::create_tmp_repo()?;
        let tmp_path = tmp_dir.path().to_path_buf();

        // Clones the repo into the temporary directory.
        // Use shallow clone to save time and space.
        let mut fetch = PrepareFetch::new(
            url,
            tmp_path.clone(),
            Kind::WithWorktree,
            create::Options {
                destination_must_be_empty: true,
                fs_capabilities: None,
            },
            open::Options::isolated(),
        )
        .map_err(|e| GitError {
            repo_url: url.to_owned(),
            message: e.to_string(),
        })?
        .with_shallow(Shallow::DepthAtRemote(
            NonZeroU32::new(1).expect("1 is not zero"),
        ));

        let (mut prepare, _outcome) = fetch
            .fetch_then_checkout(progress::Discard, &AtomicBool::new(false))
            .map_err(|e| GitError {
                repo_url: url.to_owned(),
                message: e.to_string(),
            })?;

        let (_repo, _outcome) = prepare
            .main_worktree(progress::Discard, &AtomicBool::new(false))
            .map_err(|e| GitError {
                repo_url: url.to_owned(),
                message: e.to_string(),
            })?;

        // Determines the final path to the repo taking into account the sub_folder.
        let path = if let Some(sub_folder) = sub_folder {
            let path_to_repo = tmp_path.join(sub_folder);

            // Checks the existence of the path in the repo.
            // If the path doesn't exist, returns an error.
            if !path_to_repo.exists() {
                return Err(GitError {
                    repo_url: url.to_owned(),
                    message: format!("Path `{}` not found in repo", sub_folder),
                });
            }

            path_to_repo
        } else {
            tmp_path
        };

        Ok(Self {
            id: id.to_owned(),
            registry_path,
            path,
            tmp_dir: Some(tmp_dir),
        })
    }

    /// Create a new `RegistryRepo` from a local archive.
    /// The archive can be in `.tar.gz` or `.zip` format.
    /// The sub_folder is used to filter the entries inside the archive to unpack.
    /// The temporary directory is created in the `.weaver/semconv_registry_cache`.
    /// The temporary directory is deleted when the `RegistryRepo` goes out of scope.
    ///
    /// Arguments:
    /// - `id`: The unique identifier for the registry.
    /// - `archive_filename`: The path to the archive file.
    /// - `sub_folder`: The sub-folder to unpack inside the archive.
    /// - `target_dir`: The temporary target directory where the archive will be unpacked.
    /// - `registry_path`: The registry path representation (for debug purposes).
    fn try_from_local_archive(
        id: &str,
        archive_filename: &str,
        sub_folder: Option<&String>,
        target_dir: TempDir,
        registry_path: String,
    ) -> Result<Self, Error> {
        let archive_path = Path::new(archive_filename);
        if !archive_path.exists() {
            return Err(InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: "This archive file doesn't exist".to_owned(),
            });
        }
        let archive_file = File::open(archive_path).map_err(|e| InvalidRegistryArchive {
            archive: archive_filename.to_owned(),
            error: e.to_string(),
        })?;
        let target_path_buf = target_dir.path().to_path_buf();

        // Process the supported formats (i.e.: `.tar.gz`, and `.zip`)
        if archive_filename.ends_with(TAR_GZ_EXT) {
            Self::unpack_tar_gz(archive_filename, archive_file, &target_path_buf, sub_folder)?;
        } else if archive_filename.ends_with(ZIP_EXT) {
            Self::unpack_zip(archive_filename, archive_file, &target_path_buf, sub_folder)?;
        } else {
            return Err(UnsupportedRegistryArchive {
                archive: archive_filename.to_owned(),
            });
        };

        Ok(Self {
            id: id.to_owned(),
            registry_path,
            path: target_path_buf,
            tmp_dir: Some(target_dir),
        })
    }

    /// Returns the unique identifier for the registry.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Unpacks a tar.gz archive into the specified target directory.
    ///
    /// This first directory in the archive is skipped as it is the directory corresponding to the
    /// archive itself. The sub_folder is used to filter the entries to unpack. The sub_folder
    /// directory is also skipped in the folder hierarchy to only unpack the content of the
    /// sub-folder.
    fn unpack_tar_gz(
        archive_filename: &str,
        archive_file: File,
        target_path: &Path,
        sub_folder: Option<&String>,
    ) -> Result<(), Error> {
        let tar_file = flate2::read::GzDecoder::new(archive_file);
        let mut archive = tar::Archive::new(tar_file);

        for entry in archive.entries().map_err(|e| InvalidRegistryArchive {
            archive: archive_filename.to_owned(),
            error: e.to_string(),
        })? {
            let mut entry = entry.map_err(|e| InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: e.to_string(),
            })?;

            let path = entry.path().map_err(|e| InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: e.to_string(),
            })?;

            if let Some(valid_entry_path) = Self::path_to_unpack(&path, sub_folder, target_path) {
                Self::create_parent_dirs(&valid_entry_path, archive_filename)?;
                // Unpack returns an Unpacked type containing the file descriptor to the
                // unpacked file. The file descriptor is ignored as we don't have any use for it.
                _ = entry
                    .unpack(valid_entry_path)
                    .map_err(|e| InvalidRegistryArchive {
                        archive: archive_filename.to_owned(),
                        error: e.to_string(),
                    })?;
            }
        }
        Ok(())
    }

    /// Unpacks a zip archive into the specified target directory.
    ///
    /// This first directory in the archive is skipped as it is the directory corresponding to the
    /// archive itself. The sub_folder is used to filter the entries to unpack. The sub_folder
    /// directory is also skipped in the folder hierarchy to only unpack the content of the
    /// sub-folder.
    fn unpack_zip(
        archive_filename: &str,
        archive_file: File,
        tmp_path: &Path,
        sub_folder: Option<&String>,
    ) -> Result<(), Error> {
        let mut archive =
            zip::ZipArchive::new(archive_file).map_err(|e| InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: e.to_string(),
            })?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: e.to_string(),
            })?;

            if let Some(path) = entry.enclosed_name() {
                if let Some(valid_entry_path) = Self::path_to_unpack(&path, sub_folder, tmp_path) {
                    Self::create_parent_dirs(&valid_entry_path, archive_filename)?;

                    if entry.is_dir() {
                        create_dir_all(&valid_entry_path).map_err(|e| InvalidRegistryArchive {
                            archive: archive_filename.to_owned(),
                            error: e.to_string(),
                        })?;
                    } else {
                        let mut outfile = File::create(&valid_entry_path).map_err(|e| {
                            InvalidRegistryArchive {
                                archive: archive_filename.to_owned(),
                                error: e.to_string(),
                            }
                        })?;
                        // Copy the content of the entry to the output file.
                        // `io::copy` returns the number of bytes copied, but it is ignored here
                        // as the function will return an error if the copy fails.
                        _ = io::copy(&mut entry, &mut outfile).map_err(|e| {
                            InvalidRegistryArchive {
                                archive: archive_filename.to_owned(),
                                error: e.to_string(),
                            }
                        })?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns the corrected path to unpack from an entry in the archive knowing:
    /// - the top-level directory in the archive corresponds to the initial directory archived
    /// - the sub-folder in the archive to unpack
    fn path_to_unpack(
        entry_path: &Path,
        sub_folder: Option<&String>,
        target_path: &Path,
    ) -> Option<PathBuf> {
        let mut components = entry_path.components();

        // Skip the first component, i.e. the top-level directory in the archive that
        // corresponds to the initial directory archived.
        _ = components.next();

        // If a sub-folder is specified, skip entries not in the sub-folder.
        if let Some(sub_folder) = sub_folder {
            if !sub_folder.trim().is_empty() {
                // Skip any entry that is not in the sub-folder.
                // If the entry is in the sub-folder, the sub-folder component is skipped.
                let component = components.next();
                if let Some(component) = component {
                    if component.as_os_str() != sub_folder.as_str() {
                        return None; // Skip entries not in the sub-folder
                    }
                }
            }
        }
        Some(target_path.join(components.collect::<PathBuf>()))
    }

    /// Creates parent directories for the given path.
    fn create_parent_dirs(new_path: &Path, archive_filename: &str) -> Result<(), Error> {
        if let Some(parent) = new_path.parent() {
            create_dir_all(parent).map_err(|e| InvalidRegistryArchive {
                archive: archive_filename.to_owned(),
                error: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Create a new `RegistryRepo` from a remote archive.
    ///
    /// The archive can be in `.tar.gz` or `.zip` format.
    /// The sub_folder is used to filter the entries inside the archive to unpack.
    /// The temporary directory is created in the `.weaver/semconv_registry_cache`.
    /// The temporary directory is deleted when the `RegistryRepo` goes out of scope.
    ///
    /// Arguments:
    /// - `id`: The unique identifier for the registry.
    /// - `url`: The URL of the archive.
    /// - `sub_folder`: The sub-folder to unpack inside the archive.
    /// - `target_dir`: The temporary target directory where the archive will be unpacked.
    /// - `registry_path`: The registry path representation (for debug purposes).
    fn try_from_remote_archive(
        id: &str,
        url: &str,
        sub_folder: Option<&String>,
        target_dir: TempDir,
        registry_path: String,
    ) -> Result<Self, Error> {
        let tmp_path = target_dir.path().to_path_buf();

        // Download the archive from the URL
        let response = ureq::get(url).call().map_err(|e| InvalidRegistryArchive {
            archive: url.to_owned(),
            error: e.to_string(),
        })?;
        if response.status() != 200 {
            return Err(InvalidRegistryArchive {
                archive: url.to_owned(),
                error: format!("HTTP status code: {}", response.status()),
            });
        }

        // Parse the URL to get the file name
        let parsed_url = Url::parse(url).map_err(|e| InvalidRegistryArchive {
            archive: url.to_owned(),
            error: e.to_string(),
        })?;
        let file_name = parsed_url
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .ok_or("Failed to extract file name from URL")
            .map_err(|e| InvalidRegistryArchive {
                archive: url.to_owned(),
                error: e.to_owned(),
            })?;

        // Create the full path to the save file
        let save_path = tmp_path.join(file_name);

        // Open a file in write mode
        let mut file = File::create(save_path.clone()).map_err(|e| InvalidRegistryArchive {
            archive: url.to_owned(),
            error: e.to_string(),
        })?;

        // Write the response body to the file.
        // The number of bytes written is ignored as the `try_from_local_archive` function
        // will handle the archive extraction and return an error if the archive is invalid.
        _ = io::copy(&mut response.into_reader(), &mut file).map_err(|e| {
            InvalidRegistryArchive {
                archive: url.to_owned(),
                error: e.to_string(),
            }
        })?;

        Self::try_from_local_archive(
            id,
            save_path.to_str().unwrap_or_default(),
            sub_folder,
            target_dir,
            registry_path,
        )
    }

    /// Returns the local path to the semconv registry.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Returns the registry path textual representation.
    #[must_use]
    pub fn registry_path_repr(&self) -> &str {
        &self.registry_path
    }

    /// Returns the path to the `registry_manifest.yaml` file (if any).
    #[must_use]
    pub fn manifest_path(&self) -> Option<PathBuf> {
        let manifest_path = self.path.join(REGISTRY_MANIFEST);
        if manifest_path.exists() {
            Some(manifest_path)
        } else {
            None
        }
    }

    /// Creates a temporary directory for the registry repository and returns the path.
    /// The temporary directory is created in the `.weaver/semconv_registry_cache`.
    fn create_tmp_repo() -> Result<TempDir, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        let cache_path = home.join(".weaver/semconv_registry_cache");

        create_dir_all(cache_path.as_path()).map_err(|e| Error::CacheDirNotCreated {
            message: e.to_string(),
        })?;

        let tmp_dir = TempDir::new_in(cache_path.as_path(), "repo").map_err(|e| {
            Error::CacheDirNotCreated {
                message: e.to_string(),
            }
        })?;
        Ok(tmp_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_common::test::ServeStaticFiles;

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
        let registry_path = RegistryPath::LocalFolder {
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

    fn check_archive(registry_path: RegistryPath, file_to_check: Option<&str>) {
        let repo = RegistryRepo::try_new("main", &registry_path).unwrap();
        let repo_path = repo.path().to_path_buf();
        // At this point, the repo should be cloned into a temporary directory.
        assert!(repo_path.exists());
        assert!(
            count_yaml_files(&repo_path) > 0,
            "There should be at least one `.yaml` file in the repo"
        );
        if let Some(file_to_check) = file_to_check {
            let file_path = repo_path.join(file_to_check);
            assert!(file_path.exists());
        }
        // Simulate a RegistryRepo going out of scope.
        drop(repo);
        // The temporary directory should be deleted automatically.
        assert!(!repo_path.exists());
    }

    #[test]
    fn test_semconv_registry_git_repo() {
        let registry_path = RegistryPath::GitRepo {
            // This git repo is expected to be available.
            url: "https://github.com/open-telemetry/semantic-conventions.git".to_owned(),
            sub_folder: Some("model".to_owned()),
            refspec: None,
        };
        check_archive(registry_path, None);
    }

    #[test]
    fn test_semconv_registry_local_tar_gz_archive() {
        let registry_path = "../../test_data/semantic-conventions-1.26.0.tar.gz[model]"
            .parse::<RegistryPath>()
            .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_local_zip_archive() {
        let registry_path = "../../test_data/semantic-conventions-1.26.0.zip[model]"
            .parse::<RegistryPath>()
            .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_remote_tar_gz_archive() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();
        let registry_path = format!(
            "{}[model]",
            server.relative_path_to_url("semconv_registry_v1.26.0.tar.gz")
        )
        .parse::<RegistryPath>()
        .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_remote_zip_archive() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();
        let registry_path = format!(
            "{}[model]",
            server.relative_path_to_url("semconv_registry_v1.26.0.zip")
        )
        .parse::<RegistryPath>()
        .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }
}
