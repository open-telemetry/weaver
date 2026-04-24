// SPDX-License-Identifier: Apache-2.0

//! Provides a mechanism to represent and access content from various sources as a unified
//! "virtual directory".
//!
//! This module handles resolving paths that can point to:
//! - A local filesystem directory.
//! - A local archive file (`.tar.gz` or `.zip`).
//! - A remote archive file (`.tar.gz` or `.zip`) accessible via HTTP(S).
//! - A remote individual file accessible via HTTP(S) (e.g. a published registry manifest).
//! - A Git repository accessible via HTTP(S).
//!
//! It handles the fetching, extraction, and temporary storage management transparently.
//!
//! # HTTP Authentication
//!
//! Remote downloads support per-URL Bearer-token authentication via an
//! [`HttpAuthResolver`] built from `[[auth]]` entries in `.weaver.toml` and
//! passed to [`VirtualDirectory::try_new_with_auth`]. A matching rule adds
//! `Authorization: Bearer <token>` and `User-Agent: weaver` headers.
//!
//! GitHub browser-style release-asset URLs
//! (`https://github.com/{owner}/{repo}/releases/download/{tag}/{file}`) are
//! transparently resolved to their API asset URLs, since the browser URLs do
//! not accept Bearer auth. Release metadata is cached per release so multiple
//! assets from one release cost a single API call.
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
//! - Git repo without `.git` suffix (inferred from `@refspec` or `[sub_folder]`):
//!   `https://github.com/user/repo@v1.0[schemas]`
//! - Remote archive: `https://example.com/archive.tar.gz`
//! - Remote archive with sub-folder: `https://example.com/archive.zip[data/files]`
//! - Remote file: `https://example.com/registry/manifest.yaml`
//! - GitHub release asset: `https://github.com/org/repo/releases/download/v1.0.0/manifest.yaml`
//!
//! # Disambiguating HTTP(S) URLs
//!
//! An HTTP(S) `source` is classified as follows (in order):
//! 1. `.zip` or `.tar.gz` suffix → remote archive (may carry a `[sub_folder]`).
//! 2. `.git` suffix, or presence of `@refspec` or `[sub_folder]` → Git repo. Once
//!    archives are ruled out, a `@refspec` or `[sub_folder]` is a reliable signal
//!    of a Git repo, so the `.git` suffix is not required.
//! 3. Otherwise → remote file.

use crate::http_auth::HttpAuthResolver;
use crate::vdir::VirtualDirectoryPath::{
    GitRepo, LocalArchive, LocalFolder, RemoteArchive, RemoteFile,
};
use crate::Error;
use crate::Error::{
    GitError, InvalidRegistryArchive, RemoteFileDownloadFailed, UnsupportedRegistryArchive,
};
use gix::clone::PrepareFetch;
use gix::create::Kind;
use gix::remote::fetch::Shallow;
use gix::{create, open, progress};
use once_cell::sync::Lazy;
use regex::Regex;
use rouille::url::Url;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use ureq::config::{Config, RedirectAuthHeaders};
use ureq::Agent;

/// When true, git clone operations use `open::Options::default()` which reads
/// global/system git config and enables credential helpers for private repos.
/// When false (default), uses `open::Options::isolated()` for hermetic clones.
static ALLOW_GIT_CREDENTIALS: AtomicBool = AtomicBool::new(false);

/// Enable git credential helper support for clone operations.
/// When enabled, git operations will read global/system git config,
/// allowing credential helpers (e.g., osxkeychain, git-credential-manager)
/// to authenticate with private repositories.
pub fn enable_git_credentials() {
    ALLOW_GIT_CREDENTIALS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Returns true if git credential helper support is enabled.
#[must_use]
pub fn is_git_credentials_enabled() -> bool {
    ALLOW_GIT_CREDENTIALS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Shared ureq [`Agent`] configured for authenticated HTTP downloads.
///
/// Uses `RedirectAuthHeaders::SameHost` so that the `Authorization` header
/// is preserved across same-host redirects (needed for GitHub API asset
/// downloads that redirect within `*.github.com`) but stripped on
/// cross-origin redirects. The agent is shared so that connection pooling
/// benefits multiple downloads in the same run.
static HTTP_AGENT: Lazy<Agent> = Lazy::new(|| {
    Config::builder()
        .max_redirects(10)
        .redirect_auth_headers(RedirectAuthHeaders::SameHost)
        .build()
        .into()
});

/// Attach User-Agent and, if the resolver yields a token for `url`, a Bearer
/// `Authorization` header. `url` must be the original user-supplied URL, not
/// the GitHub-API-normalized one, so rules keyed on `https://github.com/...`
/// still match when the download hits `https://api.github.com/...`.
fn attach_auth<B>(
    request: ureq::RequestBuilder<B>,
    auth: &HttpAuthResolver,
    url: &str,
) -> ureq::RequestBuilder<B> {
    let request = request.header("User-Agent", "weaver");
    match auth.resolve(url) {
        Some(token) => request.header("Authorization", &format!("Bearer {token}")),
        None => request,
    }
}

/// Download `url` into `save_path`. GitHub browser-style release URLs are
/// transparently normalized to API asset URLs so Bearer auth works for private
/// repos.
fn download_to_file(
    url: &str,
    save_path: &Path,
    auth: &HttpAuthResolver,
    map_err: impl Fn(String) -> Error,
) -> Result<(), Error> {
    let resolved_url = normalize_github_url(url, auth)?;

    let mut request = attach_auth(HTTP_AGENT.get(&resolved_url), auth, url);
    // For GitHub API asset downloads, `Accept: application/octet-stream`
    // triggers the redirect to the actual file content.
    if resolved_url.starts_with("https://api.github.com/") {
        request = request.header("Accept", "application/octet-stream");
    }
    let response = request.call().map_err(|e| map_err(e.to_string()))?;

    let mut file = File::create(save_path).map_err(|e| map_err(e.to_string()))?;
    _ = io::copy(&mut response.into_body().into_reader(), &mut file)
        .map_err(|e| map_err(e.to_string()))?;
    Ok(())
}

/// Cache for GitHub release API responses, keyed by `(owner, repo, tag)`.
/// Avoids duplicate API calls when multiple files are downloaded from the same release
/// (e.g. manifest.yaml then resolved.yaml).
static GITHUB_RELEASE_CACHE: Lazy<Mutex<HashMap<String, serde_json::Value>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// If `url` is a GitHub browser-style release asset URL, resolve it to the
/// API asset URL (which accepts Bearer token auth). Any other URL is returned
/// unchanged. Release metadata is cached so that downloading multiple assets
/// from the same release only makes one API call.
///
/// Browser form: `https://github.com/{owner}/{repo}/releases/download/{tag}/{filename}`
/// API form:     `https://api.github.com/repos/{owner}/{repo}/releases/assets/{id}`
fn normalize_github_url(url: &str, auth: &HttpAuthResolver) -> Result<String, Error> {
    normalize_github_url_with_api_base(url, "https://api.github.com", auth)
}

/// Variant of [`normalize_github_url`] with a configurable API base URL for testing.
/// The `api_base` must not end with a trailing slash.
fn normalize_github_url_with_api_base(
    url: &str,
    api_base: &str,
    auth: &HttpAuthResolver,
) -> Result<String, Error> {
    let Some((owner, repo, tag, filename)) = parse_github_release_url(url) else {
        return Ok(url.to_owned());
    };
    let err = |msg: String| RemoteFileDownloadFailed {
        url: url.to_owned(),
        error: msg,
    };

    let cache_key = format!("{owner}/{repo}/{tag}");
    let release = {
        let cache = GITHUB_RELEASE_CACHE
            .lock()
            .expect("GitHub release cache lock poisoned");
        cache.get(&cache_key).cloned()
    };
    let release = if let Some(cached) = release {
        cached
    } else {
        let api_url = format!("{api_base}/repos/{owner}/{repo}/releases/tags/{tag}");
        // Match auth against the original browser-style URL so users can key
        // `[[auth]]` rules on `https://github.com/owner/repo/...`.
        let req = attach_auth(
            HTTP_AGENT
                .get(&api_url)
                .header("Accept", "application/vnd.github+json"),
            auth,
            url,
        );
        let body: String = req
            .call()
            .map_err(|e| err(format!("GitHub API request failed: {e}")))?
            .into_body()
            .read_to_string()
            .map_err(|e| err(format!("Failed to read GitHub API response: {e}")))?;
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| err(format!("Failed to parse GitHub API response: {e}")))?;
        _ = GITHUB_RELEASE_CACHE
            .lock()
            .expect("GitHub release cache lock poisoned")
            .insert(cache_key, parsed.clone());
        parsed
    };

    find_asset_url(&release, filename, tag, url)
}

/// Parse a GitHub browser-style release asset URL into its components.
/// Returns `None` if the URL does not match the expected pattern.
fn parse_github_release_url(url: &str) -> Option<(&str, &str, &str, &str)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = rest.splitn(6, '/').collect();
    if parts.len() != 6 || parts[2] != "releases" || parts[3] != "download" {
        return None;
    }
    Some((parts[0], parts[1], parts[4], parts[5]))
}

/// Find the API asset URL for `filename` within a GitHub release JSON response.
fn find_asset_url(
    release: &serde_json::Value,
    filename: &str,
    tag: &str,
    url: &str,
) -> Result<String, Error> {
    let err = |msg: String| RemoteFileDownloadFailed {
        url: url.to_owned(),
        error: msg,
    };
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| err("GitHub release has no assets".to_owned()))?;

    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(filename))
        .ok_or_else(|| err(format!("Asset '{filename}' not found in release '{tag}'")))?;

    asset["url"]
        .as_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| err("Asset missing 'url' field".to_owned()))
}

/// The extension for a tar gz archive.
const TAR_GZ_EXT: &str = ".tar.gz";
/// The extension for a zip archive.
const ZIP_EXT: &str = ".zip";

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
        refspec: Option<String>,

        /// Optional sub-folder path within the cloned repository to use as the root directory.
        /// If omitted, the repository root is used.
        sub_folder: Option<String>,
    },
    /// A virtual directory representing a single remote file accessible via HTTP(S).
    /// Used for downloading individual files such as published registry manifests.
    RemoteFile {
        /// URL of the remote file
        url: String,
    },
}

// Helper to allow mapping an Option<String> via a function that works with empty strings.
// Empty is replaced with None and vice versa.
fn map_option<F: FnOnce(String) -> String>(opt: Option<String>, f: F) -> Option<String> {
    let result = f(opt.unwrap_or_default());
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

impl VirtualDirectoryPath {
    /// Converts a virtual directory path by manipulating the "sub folder".
    ///
    /// Returning an empty string means no sub_folder will be used in resulting path.
    ///
    /// Sub folder will be modified as follows:
    ///
    /// - LocalFolder: will see the entire path
    /// - others: will see the path inside the archive or empty string if none.
    pub fn map_sub_folder<F: FnOnce(String) -> String>(self, f: F) -> VirtualDirectoryPath {
        match self {
            LocalFolder { path } => LocalFolder { path: f(path) },
            LocalArchive { path, sub_folder } => LocalArchive {
                path,
                sub_folder: map_option(sub_folder, f),
            },
            RemoteArchive { url, sub_folder } => RemoteArchive {
                url,
                sub_folder: map_option(sub_folder, f),
            },
            GitRepo {
                url,
                refspec,
                sub_folder,
            } => GitRepo {
                url,
                refspec,
                sub_folder: map_option(sub_folder, f),
            },
            RemoteFile { url } => RemoteFile { url: f(url) },
        }
    }
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

/// Enables parsing a [`VirtualDirectoryPath`] from a string representation.
///
/// This implementation allows easy deserialization from strings (e.g. configuration files, command-line arguments).
///
/// # Errors
///
/// Returns [`Error::InvalidRegistryPath`] if the provided string does not match any valid format.
impl TryFrom<&str> for VirtualDirectoryPath {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
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
            } else if source.ends_with(".git") || refspec.is_some() || sub_folder.is_some() {
                // Archives (`.zip` / `.tar.gz`) are already handled above. Of the
                // remaining HTTP(S) sources, only a Git repo can meaningfully carry
                // a `@refspec` or a `[sub_folder]`, so their presence classifies the
                // URL as `GitRepo` even when the `.git` suffix is omitted.
                Ok(Self::GitRepo {
                    url: source.to_owned(),
                    refspec,
                    sub_folder,
                })
            } else {
                Ok(Self::RemoteFile {
                    url: source.to_owned(),
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
            RemoteFile { url } => write!(f, "{url}"),
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
#[derive(Default, Debug, Clone)]
pub struct VirtualDirectory {
    /// The original string representation used to create this virtual directory.
    vdir_path: String,

    /// The actual path on the local filesystem where the virtual directory's content resides.
    /// This might be the original path (for `LocalFolder`) or a path within a temporary cache directory.
    path: PathBuf,

    /// Holds the `TempDir` instance, ensuring the temporary directory (if created)
    /// persists for the lifetime of `VirtualDirectory` and is cleaned up afterwards.
    #[allow(dead_code)]
    tmp_dir: Arc<Option<TempDir>>,
}

impl VirtualDirectory {
    /// Resolve a [`VirtualDirectoryPath`] with no HTTP credentials configured.
    /// For remote paths behind private registries, use [`Self::try_new_with_auth`].
    pub fn try_new(vdir_path: &VirtualDirectoryPath) -> Result<Self, Error> {
        Self::try_new_with_auth(vdir_path, &HttpAuthResolver::empty())
    }

    /// Resolve a [`VirtualDirectoryPath`], using `auth` to look up Bearer
    /// credentials for any remote HTTP fetches.
    pub fn try_new_with_auth(
        vdir_path: &VirtualDirectoryPath,
        auth: &HttpAuthResolver,
    ) -> Result<Self, Error> {
        let vdir_path_repr = vdir_path.to_string();
        let vdir = match vdir_path {
            LocalFolder { path } => Ok(Self {
                vdir_path: vdir_path_repr,
                path: path.into(),
                tmp_dir: Arc::new(None),
            }),
            GitRepo {
                url,
                sub_folder,
                refspec,
            } => Self::try_from_git_url(url, sub_folder, refspec, vdir_path_repr),
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
                Self::try_from_remote_archive(
                    url,
                    sub_folder.as_ref(),
                    tmp_dir,
                    vdir_path_repr,
                    auth,
                )
            }
            RemoteFile { url } => {
                let tmp_dir = Self::create_tmp_repo()?;
                Self::try_from_remote_file(url, tmp_dir, vdir_path_repr, auth)
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
        refspec: &Option<String>,
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
            if is_git_credentials_enabled() {
                open::Options::default()
            } else {
                open::Options::isolated()
            },
        )
        .map_err(|e| GitError {
            repo_url: url.to_owned(),
            message: e.to_string(),
        })?
        .with_shallow(Shallow::DepthAtRemote(
            NonZeroU32::new(1).expect("1 is not zero"),
        ))
        .with_ref_name(refspec.as_ref())
        .map_err(|e| GitError {
            repo_url: url.to_owned(),
            message: e.to_string(),
        })?;

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
            tmp_dir: Arc::new(Some(tmp_dir)),
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
            tmp_dir: Arc::new(Some(target_dir)),
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
    /// GitHub browser-style release archive URLs are automatically normalized to API
    /// asset URLs so that Bearer token auth works for private repositories.
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
        auth: &HttpAuthResolver,
    ) -> Result<Self, Error> {
        let tmp_path = target_dir.path().to_path_buf();
        let err = |msg: String| InvalidRegistryArchive {
            archive: url.to_owned(),
            error: msg,
        };

        // Use the original URL for the filename, not the (possibly GitHub-API-normalized)
        // download URL, so the archive extension is preserved for `try_from_local_archive`.
        let parsed_url = Url::parse(url).map_err(|e| err(e.to_string()))?;
        let file_name = parsed_url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .ok_or_else(|| err("Failed to extract file name from URL".to_owned()))?;

        let save_path = tmp_path.join(file_name);
        download_to_file(url, &save_path, auth, err)?;

        Self::try_from_local_archive(
            save_path.to_str().unwrap_or_default(),
            sub_folder,
            target_dir,
            vdir_path,
        )
    }

    /// Downloads a single remote file via HTTP(S) into a temporary directory.
    ///
    /// GitHub browser-style release URLs are automatically normalized to API
    /// URLs so that Bearer token auth works for private repositories.
    ///
    /// The resulting `VirtualDirectory` path points to the downloaded file itself,
    /// enabling callers such as `RegistryRepo::try_new` to treat it as a manifest.
    fn try_from_remote_file(
        url: &str,
        target_dir: TempDir,
        vdir_path: String,
        auth: &HttpAuthResolver,
    ) -> Result<Self, Error> {
        let tmp_path = target_dir.path().to_path_buf();
        let err = |msg: String| RemoteFileDownloadFailed {
            url: url.to_owned(),
            error: msg,
        };

        // Use the original URL for the filename (not the resolved API URL, which
        // has an opaque numeric asset ID).
        let parsed_url = Url::parse(url).map_err(|e| err(e.to_string()))?;
        let file_name = parsed_url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("downloaded_file");

        let save_path = tmp_path.join(file_name);
        download_to_file(url, &save_path, auth, err)?;

        Ok(Self {
            vdir_path,
            path: save_path,
            tmp_dir: Arc::new(Some(target_dir)),
        })
    }

    /// Returns the local filesystem path to the resolved virtual directory content.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Returns the original string representation that was used to create this `VirtualDirectory`.
    #[must_use]
    pub fn vdir_path_str(&self) -> &str {
        &self.vdir_path
    }

    /// Returns the original `VirtualDirectoryRef` that was used to create this `VirtualDirectory`.
    #[must_use]
    pub fn vdir_path(&self) -> VirtualDirectoryPath {
        self.vdir_path_str()
            .try_into()
            .expect("VirtualDirectory should not have invalid `vdir_path`.")
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
    use crate::Error::GitError;
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
        check_archive_with_auth(
            vdir_path,
            file_to_check,
            &crate::http_auth::HttpAuthResolver::empty(),
        );
    }

    fn check_archive_with_auth(
        vdir_path: VirtualDirectoryPath,
        file_to_check: Option<&str>,
        auth: &crate::http_auth::HttpAuthResolver,
    ) {
        let repo = VirtualDirectory::try_new_with_auth(&vdir_path, auth).unwrap();
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
            refspec: Some(String::from("v1.26.0")),
        };
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_git_repo_with_invalid_refspec() {
        // This git repo is expected to be available.
        let url = "https://github.com/open-telemetry/semantic-conventions.git".to_owned();
        let registry_path = VirtualDirectoryPath::GitRepo {
            url: url.clone(),
            sub_folder: Some("model".to_owned()),
            refspec: Some(String::from("invalid")),
        };
        let repo = VirtualDirectory::try_new(&registry_path);
        assert!(repo.is_err());
        assert!(matches!(repo, Err(GitError { repo_url, .. }) if repo_url == url ));
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

    #[test]
    fn test_git_credentials_flag() {
        use super::{enable_git_credentials, is_git_credentials_enabled, ALLOW_GIT_CREDENTIALS};

        // Reset to known state (tests may run in any order)
        ALLOW_GIT_CREDENTIALS.store(false, std::sync::atomic::Ordering::Relaxed);

        assert!(!is_git_credentials_enabled());
        enable_git_credentials();
        assert!(is_git_credentials_enabled());

        // Reset for other tests
        ALLOW_GIT_CREDENTIALS.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Tests that remote archive downloads work with and without Bearer auth.
    #[test]
    fn test_remote_archive_auth() {
        use crate::http_auth::{AuthMatchRule, HttpAuthResolver, TokenSource};
        use crate::test::ServeStaticFilesWithAuth;

        let token = "secret-test-token";
        let server = ServeStaticFilesWithAuth::from("tests/test_data", token)
            .expect("failed to start auth server");
        let url = server.relative_path_to_url("semconv_registry_v1.26.0.tar.gz");
        let registry_path = format!("{url}[model]")
            .parse::<VirtualDirectoryPath>()
            .expect("failed to parse registry path");

        // No rule matches → no auth → server rejects.
        let result =
            VirtualDirectory::try_new_with_auth(&registry_path, &HttpAuthResolver::empty());
        assert!(
            result.is_err(),
            "expected error when no auth resolver rule matches"
        );

        // Rule matches and materializes the correct token → download succeeds.
        let resolver = HttpAuthResolver::new(vec![AuthMatchRule {
            url_prefix: server.base_url(),
            name: None,
            source: TokenSource::Token(token.to_owned()),
        }]);
        check_archive_with_auth(registry_path, Some("general.yaml"), &resolver);
    }

    #[test]
    fn test_remote_file_parsing() {
        // A URL without .git, .zip, or .tar.gz suffix should be parsed as RemoteFile
        let path_str = "https://example.com/registry/manifest.yaml";
        let path: VirtualDirectoryPath = path_str.parse().expect("failed to parse");
        assert!(
            matches!(&path, VirtualDirectoryPath::RemoteFile { url } if url == path_str),
            "Expected RemoteFile, got {path:?}"
        );
        assert_eq!(path.to_string(), path_str);

        // GitHub API release asset URL
        let path_str = "https://api.github.com/repos/org/repo/releases/assets/12345678";
        let path: VirtualDirectoryPath = path_str.parse().expect("failed to parse");
        assert!(
            matches!(&path, VirtualDirectoryPath::RemoteFile { url } if url == path_str),
            "Expected RemoteFile, got {path:?}"
        );

        // .git suffix should still be GitRepo
        let path_str = "https://github.com/org/repo.git";
        let path: VirtualDirectoryPath = path_str.parse().expect("failed to parse");
        assert!(
            matches!(&path, VirtualDirectoryPath::GitRepo { .. }),
            "Expected GitRepo, got {path:?}"
        );

        // A `@refspec` without `.git` is still a git repo.
        let path: VirtualDirectoryPath = "https://github.com/org/repo@v1.0.0"
            .parse()
            .expect("failed to parse");
        assert!(
            matches!(
                &path,
                VirtualDirectoryPath::GitRepo { url, refspec: Some(r), sub_folder: None }
                    if url == "https://github.com/org/repo" && r == "v1.0.0"
            ),
            "Expected GitRepo with refspec, got {path:?}"
        );

        // A `[sub_folder]` without `.git` is still a git repo.
        let path: VirtualDirectoryPath = "https://github.com/org/repo[model]"
            .parse()
            .expect("failed to parse");
        assert!(
            matches!(
                &path,
                VirtualDirectoryPath::GitRepo { url, refspec: None, sub_folder: Some(s) }
                    if url == "https://github.com/org/repo" && s == "model"
            ),
            "Expected GitRepo with sub_folder, got {path:?}"
        );

        // Both refspec and sub_folder, no `.git` — still a git repo.
        let path: VirtualDirectoryPath = "https://github.com/org/repo@v1.0.0[model]"
            .parse()
            .expect("failed to parse");
        assert!(
            matches!(
                &path,
                VirtualDirectoryPath::GitRepo { url, refspec: Some(r), sub_folder: Some(s) }
                    if url == "https://github.com/org/repo" && r == "v1.0.0" && s == "model"
            ),
            "Expected GitRepo with refspec and sub_folder, got {path:?}"
        );
    }

    #[test]
    fn test_remote_file_download() {
        let server = ServeStaticFiles::from("tests/test_data").expect("failed to start server");
        let url = server.relative_path_to_url("file_a.yaml");
        let vdir_path = VirtualDirectoryPath::RemoteFile { url };
        let vdir = VirtualDirectory::try_new(&vdir_path).expect("failed to download remote file");
        let content = std::fs::read_to_string(vdir.path()).expect("failed to read downloaded file");
        assert_eq!(content, "file: A");
    }

    #[test]
    fn test_parse_github_release_url() {
        use super::parse_github_release_url;

        // Canonical browser-style release asset URL.
        assert_eq!(
            parse_github_release_url(
                "https://github.com/owner/repo/releases/download/v1.0.0/manifest.yaml"
            ),
            Some(("owner", "repo", "v1.0.0", "manifest.yaml"))
        );

        // Filename containing a slash is preserved intact (splitn keeps the tail).
        assert_eq!(
            parse_github_release_url("https://github.com/o/r/releases/download/tag/sub/file.yaml"),
            Some(("o", "r", "tag", "sub/file.yaml"))
        );

        // Non-GitHub host passes through.
        assert_eq!(
            parse_github_release_url("https://example.com/owner/repo/releases/download/v1/f"),
            None
        );

        // GitHub URL that isn't a release asset download.
        assert_eq!(
            parse_github_release_url("https://github.com/owner/repo/blob/main/README.md"),
            None
        );

        // Too few path segments.
        assert_eq!(
            parse_github_release_url("https://github.com/owner/repo/releases/download/v1"),
            None
        );

        // Already-resolved API URL passes through (not a browser URL).
        assert_eq!(
            parse_github_release_url(
                "https://api.github.com/repos/owner/repo/releases/assets/12345"
            ),
            None
        );
    }

    #[test]
    fn test_normalize_github_url_passthrough() {
        use super::normalize_github_url;
        use crate::http_auth::HttpAuthResolver;

        let auth = HttpAuthResolver::empty();
        // Non-matching URLs must not trigger network calls and must come back unchanged.
        for url in [
            "https://example.com/file.yaml",
            "https://github.com/owner/repo/blob/main/README.md",
            "https://api.github.com/repos/owner/repo/releases/assets/12345",
            "http://127.0.0.1:8080/manifest.yaml",
        ] {
            assert_eq!(
                normalize_github_url(url, &auth).expect("should pass through"),
                url
            );
        }
    }

    #[test]
    fn test_normalize_github_url_resolves_asset() {
        use super::normalize_github_url_with_api_base;
        use crate::http_auth::HttpAuthResolver;
        use crate::test::{MockGitHubApi, MockRelease};

        let api = MockGitHubApi::start(vec![MockRelease {
            owner: "owner_a".to_owned(),
            repo: "repo_a".to_owned(),
            tag: "v1.0.0".to_owned(),
            assets: vec![
                ("manifest.yaml".to_owned(), b"manifest body".to_vec()),
                ("resolved.yaml".to_owned(), b"resolved body".to_vec()),
            ],
        }])
        .expect("mock API failed to start");

        let browser_url =
            "https://github.com/owner_a/repo_a/releases/download/v1.0.0/manifest.yaml";
        let auth = HttpAuthResolver::empty();
        let resolved = normalize_github_url_with_api_base(browser_url, &api.base_url(), &auth)
            .expect("normalize should succeed");
        assert_eq!(resolved, format!("{}/assets/manifest.yaml", api.base_url()));
    }

    #[test]
    fn test_normalize_github_url_caches_release() {
        use super::normalize_github_url_with_api_base;
        use crate::http_auth::HttpAuthResolver;
        use crate::test::{MockGitHubApi, MockRelease};

        let api = MockGitHubApi::start(vec![MockRelease {
            owner: "owner_b".to_owned(),
            repo: "repo_b".to_owned(),
            tag: "v2.0.0".to_owned(),
            assets: vec![
                ("manifest.yaml".to_owned(), b"m".to_vec()),
                ("resolved.yaml".to_owned(), b"r".to_vec()),
            ],
        }])
        .expect("mock API failed to start");

        let auth = HttpAuthResolver::empty();
        // Two different assets from the same release should hit the tags endpoint once.
        for filename in ["manifest.yaml", "resolved.yaml"] {
            let url =
                format!("https://github.com/owner_b/repo_b/releases/download/v2.0.0/{filename}");
            _ = normalize_github_url_with_api_base(&url, &api.base_url(), &auth)
                .expect("normalize should succeed");
        }
        assert_eq!(
            api.request_count(),
            1,
            "release metadata should be cached across asset lookups"
        );
    }

    #[test]
    fn test_normalize_github_url_missing_asset() {
        use super::normalize_github_url_with_api_base;
        use crate::http_auth::HttpAuthResolver;
        use crate::test::{MockGitHubApi, MockRelease};
        use crate::Error::RemoteFileDownloadFailed;

        let api = MockGitHubApi::start(vec![MockRelease {
            owner: "owner_c".to_owned(),
            repo: "repo_c".to_owned(),
            tag: "v3.0.0".to_owned(),
            assets: vec![("manifest.yaml".to_owned(), b"m".to_vec())],
        }])
        .expect("mock API failed to start");

        let browser_url = "https://github.com/owner_c/repo_c/releases/download/v3.0.0/missing.yaml";
        let auth = HttpAuthResolver::empty();
        let err = normalize_github_url_with_api_base(browser_url, &api.base_url(), &auth)
            .expect_err("missing asset should error");
        assert!(
            matches!(&err, RemoteFileDownloadFailed { error, .. } if error.contains("missing.yaml")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_normalize_github_url_api_404() {
        use super::normalize_github_url_with_api_base;
        use crate::http_auth::HttpAuthResolver;
        use crate::test::{MockGitHubApi, MockRelease};
        use crate::Error::RemoteFileDownloadFailed;

        // Mock serves a release for a different tag, so the requested tag 404s.
        let api = MockGitHubApi::start(vec![MockRelease {
            owner: "owner_d".to_owned(),
            repo: "repo_d".to_owned(),
            tag: "v4.0.0".to_owned(),
            assets: vec![("manifest.yaml".to_owned(), b"m".to_vec())],
        }])
        .expect("mock API failed to start");

        let browser_url =
            "https://github.com/owner_d/repo_d/releases/download/nonexistent/manifest.yaml";
        let auth = HttpAuthResolver::empty();
        let err = normalize_github_url_with_api_base(browser_url, &api.base_url(), &auth)
            .expect_err("unknown tag should error");
        assert!(
            matches!(&err, RemoteFileDownloadFailed { error, .. } if error.contains("GitHub API request failed")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_find_asset_url() {
        use super::find_asset_url;
        use crate::Error::RemoteFileDownloadFailed;

        let release = serde_json::json!({
            "assets": [
                { "name": "manifest.yaml", "url": "https://api.github.com/a/1" },
                { "name": "resolved.yaml", "url": "https://api.github.com/a/2" },
            ]
        });

        assert_eq!(
            find_asset_url(&release, "manifest.yaml", "v1", "orig").expect("found"),
            "https://api.github.com/a/1"
        );

        // Asset missing.
        let err = find_asset_url(&release, "missing.yaml", "v1", "orig").expect_err("not found");
        assert!(
            matches!(&err, RemoteFileDownloadFailed { error, .. } if error.contains("missing.yaml") && error.contains("v1"))
        );

        // Release has no `assets` array.
        let empty = serde_json::json!({});
        let err = find_asset_url(&empty, "manifest.yaml", "v1", "orig").expect_err("no assets");
        assert!(
            matches!(&err, RemoteFileDownloadFailed { error, .. } if error.contains("no assets"))
        );

        // Asset entry missing `url`.
        let no_url = serde_json::json!({ "assets": [{ "name": "manifest.yaml" }] });
        let err = find_asset_url(&no_url, "manifest.yaml", "v1", "orig").expect_err("missing url");
        assert!(matches!(&err, RemoteFileDownloadFailed { error, .. } if error.contains("'url'")));
    }
}
