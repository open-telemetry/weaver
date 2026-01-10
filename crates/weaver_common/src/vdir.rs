// SPDX-License-Identifier: Apache-2.0

//! Provides a mechanism to represent and access content from various sources as a unified
//! "virtual directory".
//!
//! This module handles resolving paths that can point to:
//! - A local filesystem directory.
//! - A local archive file (`.tar.gz` or `.zip`).
//! - A remote archive file (`.tar.gz` or `.zip`) accessible via HTTP(S).
//! - A Git repository accessible via HTTP(S).
//!
//! It handles the fetching, extraction, and temporary storage management transparently.
//!
//! It uses a specific string format to represent these sources, potentially including
//! Git refspecs (tags/branches/commits) or sub-folders within archives/repositories.
//!
//! # String Format
//!
//! The format allows specifying the source, an optional Git refspec, and an optional sub-folder:
//! `source[@refspec][\[sub_folder]]`
//!
//! - `source`: Can be a local path (`/path/to/dir`, `./archive.zip`) or a URL (`https://...`).
//! - `@refspec`: (Optional) For Git repositories, specifies a tag, branch, or commit hash.
//!   *(Note: Currently, fetching specific refspecs is not fully implemented)*.
//! - `[sub_folder]`: (Optional) Specifies a directory *within* the source (archive or Git repo)
//!   that should become the root of the virtual directory.
//!
//! # Examples
//!
//! - Local folder: `/path/to/my/files`
//! - Local archive: `data.tar.gz`
//! - Local archive with sub-folder: `data.zip[specific_dir]`
//! - Git repo (default branch): `https://github.com/user/repo.git`
//! - Git repo (tag `v1.0`, sub-folder `schemas`): `https://github.com/user/repo.git@v1.0[schemas]`
//! - Remote archive: `https://example.com/archive.tar.gz`
//! - Remote archive with sub-folder: `https://example.com/archive.zip[data/files]`

use crate::vdir::VirtualDirectoryPath::{GitRepo, LocalArchive, LocalFolder, RemoteArchive};
use crate::Error;
use crate::Error::{GitError, InvalidRegistryArchive, UnsupportedRegistryArchive};
use gix::clone::PrepareFetch;
use gix::create::Kind;
use gix::remote::fetch::Shallow;
use gix::{create, open, progress};
use once_cell::sync::Lazy;
use regex::Regex;
use rouille::url::Url;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

/// The extension for a tar gz archive.
pub const TAR_GZ_EXT: &str = ".tar.gz";
/// The extension for a zip archive.
pub const ZIP_EXT: &str = ".zip";

/// Regex to parse a virtual directory path string.
///
/// Supports the following general format: `source[@refspec][\[sub_folder]]`
/// - `source`: The main path or URL.
/// - `refspec`: Optional Git refspec (tag, branch, commit).
/// - `sub_folder`: Optional path within the source (for archives/repos).
///
/// Examples:
/// - `source`
/// - `source@tag`
/// - `source\[sub_folder]`
/// - `source@tag\[sub_folder]`
static REGISTRY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<source>.+?)(?:@(?P<refspec>.+?))?(?:\[(?P<sub_folder>.+?)])?$")
        .expect("Invalid regex")
});

/// Represents a virtual path pointing to a directory-like resource.
///
/// Supported formats include:
/// - **Local directories** (`/path/to/directory`)
/// - **Local archives** (`/path/to/archive.zip` or `/path/to/archive.tar.gz`)
/// - **Remote archives** (`https://example.com/archive.zip` or `.tar.gz`)
/// - **Git repositories** (`https://github.com/user/repo.git`)
///
/// Paths may optionally specify:
/// - A sub-folder within the archive or repository via `[sub_folder]`
/// - [Not Yet Implemented] A specific Git refspec (branch, tag, or commit) via `@refspec`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub enum VirtualDirectoryPath {
    /// A virtual directory representing a local folder.
    LocalFolder {
        /// Path to a local folder
        path: String,
    },
    /// A virtual directory representing a local archive.
    LocalArchive {
        /// Path to a local archive
        path: String,
        /// Sub-folder within the archive containing the content of interest.
        sub_folder: Option<String>,
    },
    /// A virtual directory representing a remote archive containing the content of interest.
    RemoteArchive {
        /// URL of the remote archive
        url: String,

        /// Sub-folder within the archive containing the content of interest.
        sub_folder: Option<String>,
    },
    /// A virtual directory representing a git repository containing the content of interest.
    GitRepo {
        /// The URL of the Git repository to clone (supports HTTP(S) URLs).
        url: String,

        /// Specific tag, branch, or commit hash to checkout.
        ///
        /// **Note:** Specifying this field is currently not implemented and has no effect.
        refspec: Option<String>,

        /// Optional sub-folder path within the cloned repository to use as the root directory.
        /// If omitted, the repository root is used.
        sub_folder: Option<String>,
    },
}

/// Enables parsing a [`VirtualDirectoryPath`] from a string representation.
///
/// This implementation allows easy deserialization from strings (e.g. configuration files, command-line arguments).
///
/// # Errors
///
/// Returns [`Error::InvalidRegistryPath`] if the provided string does not match any valid format.
impl TryFrom<String> for VirtualDirectoryPath {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// Implement `From<VirtualDirectoryPath>` for String, so that it can be serialized to a
/// string via serde.
impl From<VirtualDirectoryPath> for String {
    fn from(path: VirtualDirectoryPath) -> Self {
        path.to_string()
    }
}

/// Implement the `FromStr` trait for `VirtualDirectoryPath`, allowing parsing from a string.
///
/// This enables using the path string directly, e.g. as a command-line argument.
/// See the module documentation or `REGISTRY_REGEX` comment for the expected string format.
impl FromStr for VirtualDirectoryPath {
    type Err = Error;

    /// Parses a string representation into a `VirtualDirectoryPath`.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidRegistryPath` if the string does not conform to the expected
    /// format `source[@refspec][\[sub_folder]]`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let captures = REGISTRY_REGEX
            .captures(s)
            .ok_or(Error::InvalidRegistryPath {
                path: s.to_owned(),
                error: "Invalid registry path".to_owned(),
            })?;
        let source = captures
            .name("source")
            .ok_or(Error::InvalidRegistryPath {
                path: s.to_owned(),
                error: "Invalid virtual directory path. No local path or URL found".to_owned(),
            })?
            .as_str();
        let refspec = captures.name("refspec").map(|m| m.as_str().to_owned());
        let sub_folder = captures.name("sub_folder").map(|m| m.as_str().to_owned());

        if source.starts_with("http://") || source.starts_with("https://") {
            if source.ends_with(".zip") || source.ends_with(".tar.gz") {
                Ok(Self::RemoteArchive {
                    url: source.to_owned(),
                    sub_folder,
                })
            } else {
                Ok(Self::GitRepo {
                    url: source.to_owned(),
                    refspec,
                    sub_folder,
                })
            }
        } else if source.ends_with(".zip") || source.ends_with(".tar.gz") {
            Ok(Self::LocalArchive {
                path: source.to_owned(),
                sub_folder,
            })
        } else {
            Ok(Self::LocalFolder {
                path: source.to_owned(),
            })
        }
    }
}

/// Implement the `Display` trait for `VirtualDirectoryPath`, so that it can be printed
/// to the console.
impl Display for VirtualDirectoryPath {
    /// Format the `VirtualDirectoryPath` as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalFolder { path } => write!(f, "{path}"),
            LocalArchive { path, sub_folder } => {
                if let Some(sub_folder) = sub_folder {
                    write!(f, "{path}[{sub_folder}]")
                } else {
                    write!(f, "{path}")
                }
            }
            RemoteArchive { url, sub_folder } => {
                if let Some(sub_folder) = sub_folder {
                    write!(f, "{url}[{sub_folder}]")
                } else {
                    write!(f, "{url}")
                }
            }
            GitRepo {
                url,
                refspec,
                sub_folder,
            } => match (refspec, sub_folder) {
                (Some(refspec), Some(folder)) => write!(f, "{url}@{refspec}[{folder}]"),
                (Some(refspec), None) => write!(f, "{url}@{refspec}"),
                (None, Some(folder)) => write!(f, "{url}[{folder}]"),
                (None, None) => write!(f, "{url}"),
            },
        }
    }
}

/// Represents a resolved virtual directory, providing access to its content on the local filesystem.
///
/// This struct is created from a [`VirtualDirectoryPath`]. Depending on the source type,
/// it might involve:
/// - Simply pointing to an existing local directory.
/// - Cloning a Git repository into a temporary cache directory.
/// - Downloading and extracting an archive into a temporary cache directory.
///
/// Temporary directories are managed and automatically cleaned up when this struct goes out of scope.
#[derive(Default, Debug)]
pub struct VirtualDirectory {
    /// The original string representation used to create this virtual directory.
    vdir_path: String,

    /// The actual path on the local filesystem where the virtual directory's content resides.
    /// This might be the original path (for `LocalFolder`) or a path within a temporary cache directory.
    path: PathBuf,

    /// Holds the `TempDir` instance, ensuring the temporary directory (if created)
    /// persists for the lifetime of `VirtualDirectory` and is cleaned up afterwards.
    #[allow(dead_code)]
    tmp_dir: Option<TempDir>,
}

impl VirtualDirectory {
    /// Attempts to construct a new [`VirtualDirectory`] from a given [`VirtualDirectoryPath`].
    ///
    /// Depending on the variant, this may involve operations such as:
    /// - Cloning Git repositories.
    /// - Downloading and extracting remote archives.
    /// - Extracting local archives.
    ///
    /// Returns an [`Error`] if any operation fails (e.g. network issues, invalid paths, extraction failures).    
    pub fn try_new(vdir_path: &VirtualDirectoryPath) -> Result<Self, Error> {
        let vdir_path_repr = vdir_path.to_string();
        let vdir = match vdir_path {
            LocalFolder { path } => Ok(Self {
                vdir_path: vdir_path_repr,
                path: path.into(),
                tmp_dir: None,
            }),
            GitRepo {
                url, sub_folder, ..
            } => Self::try_from_git_url(url, sub_folder, vdir_path_repr),
            LocalArchive { path, sub_folder } => {
                // Create a temporary directory for the virtual directory that will be deleted
                // when the `VirtualDirectory` goes out of scope.
                let tmp_dir = Self::create_tmp_repo()?;
                Self::try_from_local_archive(path, sub_folder.as_ref(), tmp_dir, vdir_path_repr)
            }
            RemoteArchive { url, sub_folder } => {
                // Create a temporary directory for the virtual directory that will be deleted
                // when the `VirtualDirectory` goes out of scope.
                let tmp_dir = Self::create_tmp_repo()?;
                Self::try_from_remote_archive(url, sub_folder.as_ref(), tmp_dir, vdir_path_repr)
            }
        };
        vdir
    }

    /// Clones a Git repository from the specified URL into a temporary directory.
    ///
    /// Performs a shallow clone (depth=1) to optimize disk usage and clone speed.
    /// Optionally selects a sub-folder within the repository as the virtual directory root.
    ///
    /// # Errors
    ///
    /// Returns [`Error::GitError`] if:
    /// - The repository URL is invalid or inaccessible.
    /// - The sub-folder does not exist within the cloned repository.
    fn try_from_git_url(
        url: &str,
        sub_folder: &Option<String>,
        vdir_path: String,
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
                    message: format!("Path `{sub_folder}` not found in repo"),
                });
            }

            path_to_repo
        } else {
            tmp_path
        };

        Ok(Self {
            vdir_path,
            path,
            tmp_dir: Some(tmp_dir),
        })
    }

    /// Create a new `VirtualDirectory` from a local archive.
    /// The archive can be in `.tar.gz` or `.zip` format.
    /// The sub_folder is used to filter the entries inside the archive to unpack.
    /// The temporary directory is created in the `.weaver/vdir_cache`.
    /// The temporary directory is deleted when the `VirtualDirectory` goes out of scope.
    ///
    /// Arguments:
    /// - `archive_filename`: The path to the archive file.
    /// - `sub_folder`: The sub-folder to unpack inside the archive.
    /// - `target_dir`: The temporary target directory where the archive will be unpacked.
    /// - `vdir_path`: The virtual directory path representation (for debug purposes).
    fn try_from_local_archive(
        archive_filename: &str,
        sub_folder: Option<&String>,
        target_dir: TempDir,
        vdir_path: String,
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
            vdir_path,
            path: target_path_buf,
            tmp_dir: Some(target_dir),
        })
    }

    /// Extracts the contents of a `.tar.gz` archive into the specified directory.
    ///
    /// - Skips the top-level directory present in the archive (typically the archive's own folder).
    /// - If a sub-folder is provided, only extracts files within this sub-folder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRegistryArchive`] if extraction fails due to I/O issues or invalid archive contents.
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

    /// Extracts the contents of a `.zip` archive into the specified directory.
    ///
    /// - Skips the top-level directory present in the archive (typically the archive's own folder).
    /// - If a sub-folder is provided, only extracts files within this sub-folder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRegistryArchive`] if extraction fails due to I/O issues or invalid archive contents.
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

    /// Calculates the final destination path for an archive entry based on filtering rules.
    ///
    /// This function:
    /// 1. Strips the first component of the `entry_path` (the archive's root folder).
    /// 2. If `sub_folder` is `Some` and non-empty:
    ///    - Filters out entries not starting with `sub_folder` (after stripping the root).
    ///    - Strips the `sub_folder` component itself from the path.
    /// 3. Joins the remaining components onto the `target_path`.
    ///
    /// Returns `Some(PathBuf)` with the calculated target path if the entry should be unpacked,
    /// or `None` if the entry should be skipped (e.g. outside the `sub_folder`).
    ///
    /// # Arguments
    ///
    /// * `entry_path` - The path of the entry *inside* the archive.
    /// * `sub_folder` - Optional sub-folder filter.
    /// * `target_path` - The base directory where content is being unpacked.
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

    /// Create a new [`VirtualDirectory`] from a remote archive.
    ///
    /// The archive can be in `.tar.gz` or `.zip` format.
    /// The sub_folder is used to filter the entries inside the archive to unpack.
    /// The temporary directory is created in the `.weaver/vdir_cache`.
    /// The temporary directory is deleted when the [`VirtualDirectory`] goes out of scope.
    ///
    /// Arguments:
    /// - `id`: The unique identifier for the registry.
    /// - `url`: The URL of the archive.
    /// - `sub_folder`: The sub-folder to unpack inside the archive.
    /// - `target_dir`: The temporary target directory where the archive will be unpacked.
    /// - `vdir_path`: The virtual directory path representation (for debug purposes).
    fn try_from_remote_archive(
        url: &str,
        sub_folder: Option<&String>,
        target_dir: TempDir,
        vdir_path: String,
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
            .and_then(|mut segments| segments.next_back())
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
        _ = io::copy(&mut response.into_body().into_reader(), &mut file).map_err(|e| {
            InvalidRegistryArchive {
                archive: url.to_owned(),
                error: e.to_string(),
            }
        })?;

        Self::try_from_local_archive(
            save_path.to_str().unwrap_or_default(),
            sub_folder,
            target_dir,
            vdir_path,
        )
    }

    /// Returns the local filesystem path to the resolved virtual directory content.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Returns the original string representation that was used to create this `VirtualDirectory`.
    #[must_use]
    pub fn vdir_path(&self) -> &str {
        &self.vdir_path
    }

    /// Creates and returns a new temporary directory within `.weaver/vdir_cache`.
    ///
    /// The created directory and its contents are automatically deleted when dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::HomeDirNotFound`] if the user's home directory cannot be determined.
    /// Returns [`Error::CacheDirNotCreated`] if the temporary directory cannot be created due to permission or filesystem errors.
    fn create_tmp_repo() -> Result<TempDir, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        let cache_path = home.join(".weaver/vdir_cache");

        create_dir_all(cache_path.as_path()).map_err(|e| Error::CacheDirNotCreated {
            message: e.to_string(),
        })?;

        let tmp_dir = tempfile::Builder::new()
            .prefix("repo")
            .tempdir_in(cache_path.as_path())
            .map_err(|e| Error::CacheDirNotCreated {
                message: e.to_string(),
            })?;
        Ok(tmp_dir)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::ServeStaticFiles;
    use crate::vdir::{VirtualDirectory, VirtualDirectoryPath};
    use std::path::Path;

    #[test]
    fn test_virtual_directory_path() {
        // Local folder
        let registry_path_str = "path/to/registry";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::LocalFolder { path } = &registry_path {
            assert_eq!(path, registry_path_str);
        } else {
            panic!("Expected LocalFolder, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive (zip)
        let registry_path_str = "http://example.com/registry.zip";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, registry_path_str);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive with sub-folder (zip)
        let registry_path_str = "http://example.com/registry.zip[model]";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, "http://example.com/registry.zip");
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive (tar.gz)
        let registry_path_str = "http://example.com/registry.tar.gz";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, registry_path_str);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive with sub-folder (tar.gz)
        let registry_path_str = "http://example.com/registry.tar.gz[model]";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, "http://example.com/registry.tar.gz");
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository
        let registry_path_str = "http://example.com/registry.git";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::GitRepo {
            url,
            refspec,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, registry_path_str);
            assert_eq!(*refspec, None);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with sub-folder
        let registry_path_str = "http://example.com/registry.git[model]";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::GitRepo {
            url,
            refspec,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*refspec, None);
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with tag
        let registry_path_str = "http://example.com/registry.git@v1.0.0";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::GitRepo {
            url,
            refspec,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*refspec, Some("v1.0.0".to_owned()));
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with tag and sub-folder
        let registry_path_str = "http://example.com/registry.git@v1.0.0[model]";
        let registry_path: VirtualDirectoryPath = registry_path_str.parse().unwrap();
        if let VirtualDirectoryPath::GitRepo {
            url,
            refspec,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*refspec, Some("v1.0.0".to_owned()));
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);
    }

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
        // A virtual directory created from a local folder.
        let vdir_path = VirtualDirectoryPath::LocalFolder {
            path: "../../crates/weaver_codegen_test/semconv_registry".to_owned(),
        };
        let repo = VirtualDirectory::try_new(&vdir_path).unwrap();
        let repo_path = repo.path().to_path_buf();
        assert!(repo_path.exists());
        assert!(
            count_yaml_files(&repo_path) > 0,
            "There should be at least one `.yaml` file in the repo"
        );
        // Simulate a virtual directory going out of scope.
        drop(repo);
        // The local folder should not be deleted.
        assert!(repo_path.exists());
    }

    fn check_archive(vdir_path: VirtualDirectoryPath, file_to_check: Option<&str>) {
        let repo = VirtualDirectory::try_new(&vdir_path).unwrap();
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
        // Simulate a virtual directory going out of scope.
        drop(repo);
        // The temporary directory should be deleted automatically.
        assert!(!repo_path.exists());
    }

    #[test]
    fn test_semconv_registry_git_repo() {
        let registry_path = VirtualDirectoryPath::GitRepo {
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
            .parse::<VirtualDirectoryPath>()
            .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_local_zip_archive() {
        let registry_path = "../../test_data/semantic-conventions-1.26.0.zip[model]"
            .parse::<VirtualDirectoryPath>()
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
        .parse::<VirtualDirectoryPath>()
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
        .parse::<VirtualDirectoryPath>()
        .unwrap();
        check_archive(registry_path, Some("general.yaml"));
    }
}
