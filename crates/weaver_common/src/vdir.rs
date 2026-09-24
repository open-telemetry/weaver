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
use std::sync::{Arc, Mutex, RwLock};
use tempfile::TempDir;
use ureq::config::{Config, RedirectAuthHeaders};
use ureq::tls::{RootCerts, TlsConfig};
use ureq::Agent;
use url::Url;

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
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
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

/// Returns `true` if `s` is a full-length hex object id (commit SHA).
///
/// Delegates to [`gix::ObjectId::from_hex`] so it accepts exactly the hash
/// kinds gix is actually built with (currently SHA-1, i.e. 40 hex chars; it
/// will pick up SHA-256 automatically if/when that feature is enabled).
///
///  Note that a branch or tag whose name is itself a full-length hex string is
/// indistinguishable from an object id and will be treated as a SHA.
fn is_commit_sha(s: &str) -> bool {
    gix::ObjectId::from_hex(s.as_bytes()).is_ok()
}

/// Configuration for the on-disk registry cache, set from the CLI at startup.
#[derive(Debug, Clone, Default)]
struct GitCacheConfig {
    /// Cache root directory, or `None` when caching is disabled.
    root: Option<PathBuf>,
    /// When true, a cache miss for a cacheable source errors instead of fetching.
    offline: bool,
    /// When true, re-fetch and replace a cached entry even on a hit.
    refresh: bool,
}

/// Whether a cloned refspec can later point at different content.
///
/// Only [`RefStability::Immutable`] sources are cached; caching a moving ref
/// would serve its first snapshot indefinitely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefStability {
    /// A commit SHA or a tag.
    Immutable,
    /// A branch, or the remote default branch when no refspec is given.
    Moving,
}

/// A cloned repository on disk, either a cache entry that outlives the process
/// or a directory deleted when it is dropped.
#[derive(Debug)]
enum GitCheckout {
    /// A cache entry, kept after the command exits.
    Cached(PathBuf),
    /// A directory deleted when the returned [`VirtualDirectory`] is dropped.
    Throwaway(TempDir),
}

impl GitCheckout {
    /// The repository root on disk.
    fn path(&self) -> &Path {
        match self {
            GitCheckout::Cached(path) => path,
            GitCheckout::Throwaway(tmp_dir) => tmp_dir.path(),
        }
    }

    /// The directory to delete on drop, if this checkout owns one.
    fn into_tmp_dir(self) -> Option<TempDir> {
        match self {
            GitCheckout::Cached(_) => None,
            GitCheckout::Throwaway(tmp_dir) => Some(tmp_dir),
        }
    }
}

/// Process-wide git-registry cache configuration, set once at startup by the CLI
/// layer via [`configure_git_cache`]. Defaults to disabled (`root: None`).
static GIT_CACHE_CONFIG: Lazy<RwLock<GitCacheConfig>> =
    Lazy::new(|| RwLock::new(GitCacheConfig::default()));

/// Configures the on-disk registry cache for this process.
///
/// Called once from the CLI layer, mirroring [`enable_git_credentials`].
/// `cache_dir = None` leaves the cache disabled, the default.
pub fn configure_git_cache(cache_dir: Option<PathBuf>, offline: bool, refresh: bool) {
    let mut cfg = GIT_CACHE_CONFIG
        .write()
        .expect("git cache config lock poisoned");
    *cfg = GitCacheConfig {
        root: cache_dir,
        offline,
        refresh,
    };
}

/// Returns a snapshot of the current git-registry cache configuration.
fn git_cache_config() -> GitCacheConfig {
    GIT_CACHE_CONFIG
        .read()
        .expect("git cache config lock poisoned")
        .clone()
}

/// Derives a filesystem-safe cache directory name for a pinned Git source.
///
/// The key is a function of `(url, refspec)` only — the sub-folder is applied
/// after checkout, so registries that differ only by sub-folder share one clone.
/// A short human-readable slug is prefixed for debuggability, and a truncated
/// SHA-1 of the two fields makes the name unique. The digest is stable across
/// platforms and toolchain versions, so the key is reproducible in CI caches.
///
/// Returns `None` when Git's SHA-1 collision detection flags the input, so a
/// crafted source cannot share another source's cache entry.
fn git_cache_key(url: &str, refspec: &str) -> Option<String> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(url.as_bytes());
    hasher.update(&[0]);
    hasher.update(refspec.as_bytes());
    let digest = hasher.try_finalize().ok()?;
    let hash = digest.to_hex_with_len(16);

    let sanitize = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .take(32)
            .collect()
    };

    let repo_slug = sanitize(
        url.rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("registry")
            .trim_end_matches(".git"),
    );
    let ref_slug = sanitize(refspec);

    Some(format!("{repo_slug}-{ref_slug}-{hash}"))
}

/// Strips a `user[:password]@` component from `url`, returning `None` when the
/// URL carries no credentials.
fn url_without_userinfo(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    if parsed.username().is_empty() && parsed.password().is_none() {
        return None;
    }
    parsed.set_username("").ok()?;
    parsed.set_password(None).ok()?;
    Some(parsed.to_string())
}

/// Derives a unique sibling path under `git_root` from an existing unique staging
/// directory, replacing the `.staging-` prefix with `prefix`. Used to name the
/// retired copy of a cache entry during an atomic refresh swap, so the sibling is
/// as unique as the staging directory it is derived from.
fn sibling_cache_path(git_root: &Path, staged: &Path, prefix: &str) -> PathBuf {
    let name = staged
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("staging");
    let suffix = name.strip_prefix(".staging-").unwrap_or(name);
    git_root.join(format!("{prefix}{suffix}"))
}

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

/// Errors returned by the individual gix calls in [`VirtualDirectory::checkout_sha`].
///
/// This exists purely so those calls can use `?` directly (via the `#[from]`
/// conversions below) instead of being wrapped in a closure. It is private to
/// this module and gets converted into [`Error::GitError`] — adding the repo URL
/// as context — at the single call site.
#[derive(thiserror::Error, Debug)]
enum CheckoutError {
    #[error("repository has no worktree")]
    NoWorktree,
    #[error(transparent)]
    DecodeSha(#[from] gix::hash::decode::Error),
    #[error(transparent)]
    FindObject(#[from] gix::object::find::existing::Error),
    #[error(transparent)]
    PeelToTree(#[from] gix::object::peel::to_kind::Error),
    #[error(transparent)]
    IndexFromTree(#[from] gix::repository::index_from_tree::Error),
    #[error(transparent)]
    CheckoutOptions(#[from] gix::config::checkout_options::Error),
    #[error(transparent)]
    OpenObjectStore(#[from] io::Error),
    #[error(transparent)]
    Checkout(#[from] gix::worktree::state::checkout::Error),
    #[error(transparent)]
    WriteIndex(#[from] gix::index::file::write::Error),
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

    /// Resolves a Git repository source into a [`VirtualDirectory`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::GitError`] if:
    /// - The repository URL is invalid or inaccessible.
    /// - The sub-folder does not exist within the cloned repository.
    ///
    /// Returns [`Error::RegistryOffline`] if offline mode is enabled and the
    /// source is not already present in the cache.
    fn try_from_git_url(
        url: &str,
        sub_folder: &Option<String>,
        refspec: &Option<String>,
        vdir_path: String,
    ) -> Result<Self, Error> {
        Self::try_from_git_url_with_cache(url, sub_folder, refspec, vdir_path, &git_cache_config())
    }

    /// Resolves a Git source into a [`VirtualDirectory`] backed by `cache`.
    fn try_from_git_url_with_cache(
        url: &str,
        sub_folder: &Option<String>,
        refspec: &Option<String>,
        vdir_path: String,
        cache: &GitCacheConfig,
    ) -> Result<Self, Error> {
        let checkout = Self::checkout_git_repo(url, refspec, cache)?;
        let path = Self::resolve_git_sub_folder(checkout.path(), sub_folder, url)?;
        Ok(Self {
            vdir_path,
            path,
            tmp_dir: Arc::new(checkout.into_tmp_dir()),
        })
    }

    /// Materializes `url` on disk, in the cache when the source is cacheable
    /// and in a throwaway directory otherwise.
    ///
    /// A cache hit is reused with no network access. A miss clones into a
    /// private staging directory, which is installed in the cache only for an
    /// [`RefStability::Immutable`] refspec; a moving one is served from the
    /// staging directory, which is then a throwaway clone. A source with no
    /// refspec tracks the remote default branch, so it never consults `cache`.
    fn checkout_git_repo(
        url: &str,
        refspec: &Option<String>,
        cache: &GitCacheConfig,
    ) -> Result<GitCheckout, Error> {
        let cacheable = cache
            .root
            .as_ref()
            .zip(refspec.as_ref())
            .and_then(|(root, pinned)| git_cache_key(url, pinned).map(|key| (root, pinned, key)));
        let Some((cache_root, pinned, key)) = cacheable else {
            let tmp_dir = Self::create_tmp_repo()?;
            let _ = Self::clone_into(url, refspec, tmp_dir.path())?;
            return Ok(GitCheckout::Throwaway(tmp_dir));
        };

        let git_root = cache_root.join("git");
        let target = git_root.join(key);

        // A refresh needs the network, so offline serves whatever is cached.
        let refresh = cache.refresh && !cache.offline;

        if !refresh && target.exists() {
            return Ok(GitCheckout::Cached(target));
        }
        if cache.offline {
            return Err(Error::RegistryOffline {
                registry: format!("{url}@{pinned}"),
            });
        }

        Ok(
            match Self::populate_git_cache(url, pinned, &git_root, &target)? {
                None => GitCheckout::Cached(target),
                Some(tmp_dir) => GitCheckout::Throwaway(tmp_dir),
            },
        )
    }

    /// Clones `url`@`refspec` into a private staging directory under `git_root`.
    ///
    /// Returns `None` once the clone has been installed at `target`, or
    /// `Some(staging)` when the refspec is [`RefStability::Moving`] and so must
    /// not be cached — the caller serves that throwaway directory instead.
    ///
    /// The clone is completed in staging before any rename, so `target` is only
    /// ever created or replaced as a single complete directory.
    ///
    /// - Fresh entry: one atomic `rename`. A process that loses the race
    ///   discards its staging copy and keeps the winner's clone.
    /// - Refresh: the current entry is retired to a unique sibling and the new
    ///   clone renamed into place, leaving `target` briefly absent; the retired
    ///   copy is then deleted, or restored if the final rename fails.
    fn populate_git_cache(
        url: &str,
        refspec: &str,
        git_root: &Path,
        target: &Path,
    ) -> Result<Option<TempDir>, Error> {
        create_dir_all(git_root).map_err(|e| Error::CacheDirNotCreated {
            message: e.to_string(),
        })?;

        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(git_root)
            .map_err(|e| Error::CacheDirNotCreated {
                message: e.to_string(),
            })?;
        let stability = Self::clone_into(url, &Some(refspec.to_owned()), staging.path())?;
        if stability == RefStability::Moving {
            return Ok(Some(staging));
        }
        Self::strip_url_credentials(staging.path(), url);

        // Disarm the temp-dir guard: we move the directory into place ourselves
        // and clean it up manually on the error/race paths.
        let staged = staging.keep();

        let cache_err = |e: io::Error| Error::CacheEntryNotInstalled {
            registry: format!("{url}@{refspec}"),
            message: e.to_string(),
        };

        // Fast path: no existing entry, so a single atomic rename installs it.
        if !target.exists() {
            return match std::fs::rename(&staged, target) {
                Ok(()) => Ok(None),
                // Lost the race with a concurrent populate: keep the winner's clone.
                Err(_) if target.exists() => {
                    let _ = std::fs::remove_dir_all(&staged);
                    Ok(None)
                }
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staged);
                    Err(cache_err(e))
                }
            };
        }

        // Refresh path: retire the current entry to a unique sibling derived from
        // the (already-unique) staging name, keeping a complete directory visible.
        let aside = sibling_cache_path(git_root, &staged, ".old-");
        match std::fs::rename(target, &aside) {
            Ok(()) => {}
            // Another refresher already retired it: treat as a lost race and keep
            // whatever is now in place.
            Err(_) if !target.exists() => {
                let _ = std::fs::remove_dir_all(&staged);
                return Ok(None);
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staged);
                return Err(cache_err(e));
            }
        }

        match std::fs::rename(&staged, target) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&aside);
                Ok(None)
            }
            Err(e) => {
                // Restore the retired entry so a working cache is never lost.
                let _ = std::fs::rename(&aside, target);
                let _ = std::fs::remove_dir_all(&staged);
                Err(cache_err(e))
            }
        }
    }

    /// Rewrites `remote.origin.url` in a freshly cloned repository so a URL
    /// carrying credentials does not persist in the cache on disk.
    fn strip_url_credentials(dest: &Path, url: &str) {
        let Some(sanitized) = url_without_userinfo(url) else {
            return;
        };
        let config = dest.join(".git").join("config");
        if let Ok(contents) = std::fs::read_to_string(&config) {
            let _ = std::fs::write(&config, contents.replace(url, &sanitized));
        }
    }

    /// Clones `url` (optionally at `refspec`) into the existing empty directory
    /// `dest`, checking out the requested worktree, and reports whether the
    /// checked-out ref is immutable.
    ///
    /// Performs a shallow clone (depth=1) when no specific refspec is given.
    /// When a refspec is provided, we skip shallow clone because gix's
    /// shallow+single-branch code path assumes the ref is a branch
    /// (refs/heads/), which breaks for tags (refs/tags/).
    /// See upstream issue: <https://github.com/GitoxideLabs/gitoxide/issues/2554>
    fn clone_into(url: &str, refspec: &Option<String>, dest: &Path) -> Result<RefStability, Error> {
        let prepare = PrepareFetch::new(
            url,
            dest,
            Kind::WithWorktree,
            create::Options {
                destination_must_be_empty: Some(true),
                fs_capabilities: None,
                object_hash: None,
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
        })?;

        // Determine whether the refspec is a commit SHA. `with_ref_name` only
        // accepts symbolic refs (branches/tags) and panics on raw object IDs.
        let is_sha = refspec.as_ref().is_some_and(|r| is_commit_sha(r));

        let mut fetch = if refspec.is_none() {
            prepare.with_shallow(Shallow::DepthAtRemote(
                NonZeroU32::new(1).expect("1 is not zero"),
            ))
        } else {
            prepare
        }
        // Only pass the refspec to `with_ref_name` when it is a symbolic ref.
        // Commit SHAs are handled via `checkout_sha` after fetching.
        .with_ref_name(if is_sha { None } else { refspec.as_ref() })
        .map_err(|e| GitError {
            repo_url: url.to_owned(),
            message: e.to_string(),
        })?;

        let (mut checkout, _outcome) = fetch
            .fetch_then_checkout(progress::Discard, &AtomicBool::new(false))
            .map_err(|e| GitError {
                repo_url: url.to_owned(),
                message: e.to_string(),
            })?;

        if is_sha {
            // For commit SHAs we skip `main_worktree()` (which would checkout
            // the default branch) and instead checkout the requested commit
            // directly using gix APIs.
            let sha = refspec.as_ref().expect("is_sha implies Some");
            let repo = checkout.persist();
            Self::checkout_sha(&repo, sha).map_err(|e| GitError {
                repo_url: url.to_owned(),
                message: format!("failed to checkout commit {sha}: {e}"),
            })?;
            return Ok(RefStability::Immutable);
        }

        // `main_worktree` checks out the requested ref (or the default branch)
        // onto disk at `dest`, and mutates `checkout` to disarm its
        // delete-clone-on-drop guard, so the worktree files persist.
        let (repo, _) = checkout
            .main_worktree(progress::Discard, &AtomicBool::new(false))
            .map_err(|e| GitError {
                repo_url: url.to_owned(),
                message: e.to_string(),
            })?;

        if refspec.is_none() {
            return Ok(RefStability::Moving);
        }
        // A refspec resolves to exactly one ref, which `main_worktree` points
        // HEAD at: `refs/tags/…` for a tag, `refs/heads/…` for a branch.
        let checked_out_tag = repo
            .head()
            .ok()
            .and_then(|head| head.referent_name().map(|name| name.as_bstr().to_string()))
            .is_some_and(|name| name.starts_with("refs/tags/"));
        Ok(if checked_out_tag {
            RefStability::Immutable
        } else {
            RefStability::Moving
        })
    }

    /// Resolves the final content path within a cloned repository at `base`,
    /// applying `sub_folder` if present and verifying it exists.
    fn resolve_git_sub_folder(
        base: &Path,
        sub_folder: &Option<String>,
        url: &str,
    ) -> Result<PathBuf, Error> {
        match sub_folder {
            Some(sub_folder) => {
                let path_to_repo = base.join(sub_folder);
                if !path_to_repo.exists() {
                    return Err(GitError {
                        repo_url: url.to_owned(),
                        message: format!("Path `{sub_folder}` not found in repo"),
                    });
                }
                Ok(path_to_repo)
            }
            None => Ok(base.to_path_buf()),
        }
    }

    /// Checkout a specific commit SHA in a cloned repository using gix APIs.
    ///
    /// Resolves `sha` to a tree, builds an index, and writes the worktree.
    /// This avoids the `main_worktree()` path which can only checkout HEAD or
    /// a symbolic ref.
    // The gix error types wrapped by `CheckoutError` are large; boxing them would
    // break the `#[from]`/`?` conversions this helper relies on, so allow the lint.
    #[allow(clippy::result_large_err)]
    fn checkout_sha(repo: &gix::Repository, sha: &str) -> Result<(), CheckoutError> {
        let workdir = repo.workdir().ok_or(CheckoutError::NoWorktree)?;
        let id = gix::ObjectId::from_hex(sha.as_bytes())?;
        let tree_id = repo.find_object(id)?.peel_to_tree()?.id;
        let mut index = repo.index_from_tree(&tree_id)?;

        let mut opts =
            repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
        opts.destination_is_initially_empty = true;

        let _outcome = gix::worktree::state::checkout(
            &mut index,
            workdir,
            repo.objects.clone().into_arc()?,
            &progress::Discard,
            &progress::Discard,
            &AtomicBool::new(false),
            opts,
        )?;

        index.write(Default::default())?;
        Ok(())
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
    use super::GitCacheConfig;
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

    fn install_test_crypto_provider() {
        // This test binary is the TLS-using application, so it owns provider
        // selection just as the Weaver and xtask binaries do.
        let _ = rustls::crypto::ring::default_provider().install_default();
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
        install_test_crypto_provider();
        let registry_path = VirtualDirectoryPath::GitRepo {
            // This git repo is expected to be available.
            url: "https://github.com/open-telemetry/semantic-conventions.git".to_owned(),
            sub_folder: Some("model".to_owned()),
            refspec: Some(String::from("v1.26.0")),
        };
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_git_repo_with_commit_sha() {
        install_test_crypto_provider();
        // Regression test for the panic that occurred when a refspec is a raw
        // commit SHA (rather than a branch/tag): `with_ref_name` panics on object
        // IDs, so SHAs must go through the `checkout_sha` path instead.
        //
        // `9adff43…` is the commit tagged `v1.26.0`, an ancestor of the default
        // branch, so a full clone fetches it and `checkout_sha` can resolve it.
        let registry_path = VirtualDirectoryPath::GitRepo {
            url: "https://github.com/open-telemetry/semantic-conventions.git".to_owned(),
            sub_folder: Some("model".to_owned()),
            refspec: Some(String::from("9adff435c76ea3b8cba89babefc832d3dd3c1ab9")),
        };
        check_archive(registry_path, Some("general.yaml"));
    }

    #[test]
    fn test_semconv_registry_git_repo_with_nonexistent_commit_sha() {
        install_test_crypto_provider();
        // A well-formed SHA that does not exist in the repo must fail gracefully
        // (a `GitError`, not a panic) when `checkout_sha` cannot resolve it.
        let url = "https://github.com/open-telemetry/semantic-conventions.git".to_owned();
        let registry_path = VirtualDirectoryPath::GitRepo {
            url: url.clone(),
            sub_folder: Some("model".to_owned()),
            refspec: Some(String::from("0000000000000000000000000000000000000000")),
        };
        let repo = VirtualDirectory::try_new(&registry_path);
        assert!(matches!(repo, Err(GitError { repo_url, .. }) if repo_url == url));
    }

    #[test]
    fn test_semconv_registry_git_repo_with_invalid_refspec() {
        install_test_crypto_provider();
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

    #[test]
    fn test_is_commit_sha() {
        use super::is_commit_sha;

        // Valid SHA-1 (40 hex chars)
        assert!(is_commit_sha("d84341cf20a1fef1a833ef44d318c41a770e6e64"));
        assert!(is_commit_sha("0000000000000000000000000000000000000000"));
        assert!(is_commit_sha("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        // 64 hex chars (SHA-256). gix is built with the `sha1` feature only.
        // If gix gains SHA-256 support this should flip.
        assert!(!is_commit_sha(
            "d84341cf20a1fef1a833ef44d318c41a770e6e64d84341cf20a1fef1a833ef44"
        ));

        // Too short / too long
        assert!(!is_commit_sha("d84341cf"));
        assert!(!is_commit_sha("d84341cf20a1fef1a833ef44d318c41a770e6e6")); // 39 chars
        assert!(!is_commit_sha("d84341cf20a1fef1a833ef44d318c41a770e6e640")); // 41 chars

        // Non-hex characters
        assert!(!is_commit_sha("g84341cf20a1fef1a833ef44d318c41a770e6e64"));

        // Symbolic refs
        assert!(!is_commit_sha("main"));
        assert!(!is_commit_sha("v1.0.0"));
        assert!(!is_commit_sha("refs/heads/main"));
    }

    #[test]
    fn test_git_cache_key_is_stable_and_distinct() {
        use super::git_cache_key;

        let url = "https://github.com/open-telemetry/semantic-conventions.git";
        let key = git_cache_key(url, "v1.41.0").unwrap();

        // Deterministic across calls.
        assert_eq!(key, git_cache_key(url, "v1.41.0").unwrap());
        // Distinct per refspec and per URL.
        assert_ne!(key, git_cache_key(url, "v1.40.0").unwrap());
        assert_ne!(
            key,
            git_cache_key("https://github.com/other/repo.git", "v1.41.0").unwrap()
        );
        // Filesystem-safe: only alphanumerics and `-`, `.`, `_` (no path separators).
        assert!(key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_'));
        // Human-readable prefix aids debugging.
        assert!(
            key.starts_with("semantic-conventions-v1.41.0-"),
            "unexpected key: {key}"
        );

        // A refspec holding path separators stays a single path segment.
        let nested = git_cache_key(url, "refs/heads/my branch").unwrap();
        assert!(
            !nested.contains('/') && !nested.contains(std::path::MAIN_SEPARATOR),
            "key must not contain a path separator: {nested}"
        );
        assert!(nested.starts_with("semantic-conventions-refs_heads_my_branch-"));
        assert_ne!(nested, git_cache_key(url, "refs/heads/my-branch").unwrap());
    }

    #[test]
    fn test_resolve_git_sub_folder() {
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(base.path().join("model")).unwrap();

        assert_eq!(
            VirtualDirectory::resolve_git_sub_folder(base.path(), &None, "url").unwrap(),
            base.path()
        );
        assert_eq!(
            VirtualDirectory::resolve_git_sub_folder(base.path(), &Some("model".to_owned()), "url")
                .unwrap(),
            base.path().join("model")
        );

        let missing = VirtualDirectory::resolve_git_sub_folder(
            base.path(),
            &Some("nope".to_owned()),
            "https://example.com/repo.git",
        );
        assert!(
            matches!(&missing, Err(GitError { repo_url, .. }) if repo_url == "https://example.com/repo.git"),
            "expected GitError, got {missing:?}"
        );
    }

    #[test]
    fn test_git_cache_population_fails_when_root_is_not_a_directory() {
        use crate::Error::CacheDirNotCreated;

        let cache = tempfile::tempdir().unwrap();
        // `<cache>/git` is where entries live; a regular file there makes the
        // cache unusable, and that must surface rather than be ignored.
        std::fs::write(cache.path().join("git"), "not a directory").unwrap();

        let result = VirtualDirectory::try_from_git_url_with_cache(
            "https://example.com/repo.git",
            &Some("model".to_owned()),
            &Some("v1.0.0".to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: false,
                refresh: false,
            },
        );
        assert!(
            matches!(result, Err(CacheDirNotCreated { .. })),
            "expected CacheDirNotCreated, got {result:?}"
        );
    }

    #[test]
    fn test_git_cache_offline_miss_errors() {
        use crate::Error::RegistryOffline;

        let cache = tempfile::tempdir().unwrap();
        // Offline + empty cache must fail fast with `RegistryOffline` and never
        // touch the network.
        let result = VirtualDirectory::try_from_git_url_with_cache(
            "https://github.com/open-telemetry/semantic-conventions.git",
            &Some("model".to_owned()),
            &Some("v1.26.0".to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: true,
                refresh: false,
            },
        );
        assert!(
            matches!(result, Err(RegistryOffline { .. })),
            "expected RegistryOffline, got {result:?}"
        );
    }

    #[test]
    fn test_git_cache_hit_serves_without_network() {
        use super::git_cache_key;

        let cache = tempfile::tempdir().unwrap();
        let url = "https://github.com/open-telemetry/semantic-conventions.git";
        let refspec = "v1.26.0";

        // Pre-seed a cached clone so the resolve needs no network.
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(url, refspec).unwrap());
        let model = entry.join("model");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("general.yaml"), "groups: []\n").unwrap();

        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            url,
            &Some("model".to_owned()),
            &Some(refspec.to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: true,
                refresh: false,
            },
        )
        .expect("cache hit should succeed offline");

        let path = vdir.path().to_path_buf();
        assert_eq!(path, model);
        assert!(path.join("general.yaml").exists());

        // A cached entry must survive the virtual directory going out of scope.
        drop(vdir);
        assert!(path.exists(), "cached registry must not be deleted on drop");
    }

    /// Creates a tiny local git repository containing `model/general.yaml` tagged
    /// `v0.0.1`, so the cache populate path can be exercised via a `file://` URL
    /// with no network access (and fast enough for coverage instrumentation).
    /// Returns the repo dir (kept alive by the caller) and its `file://` URL.
    fn make_local_git_repo() -> (tempfile::TempDir, String) {
        use std::process::Command;

        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .status()
                .expect("failed to run git")
                .success();
            assert!(ok, "git {args:?} failed");
        };

        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::create_dir_all(repo.path().join("model")).unwrap();
        std::fs::write(repo.path().join("model/general.yaml"), "groups: []\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);
        git(&["tag", "v0.0.1"]);

        let url = format!("file://{}", repo.path().display());
        (repo, url)
    }

    #[test]
    fn test_git_cache_populates_and_reuses() {
        use super::git_cache_key;

        let (_repo, url) = make_local_git_repo();
        let refspec = "v0.0.1";
        let cache = tempfile::tempdir().unwrap();
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(&url, refspec).unwrap());
        assert!(!entry.exists());

        // First call populates the cache by cloning the (local) repo.
        let first = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some(refspec.to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: false,
                refresh: false,
            },
        )
        .expect("first call should populate the cache");
        let first_path = first.path().to_path_buf();
        assert!(first_path.join("general.yaml").exists());

        // The populated entry must persist after drop (it is not a temp dir).
        drop(first);
        assert!(entry.exists());
        assert!(first_path.exists());

        // Second call is served from the cache with no fetch.
        let second = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some(refspec.to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: true,
                refresh: false,
            },
        )
        .expect("second call should hit the cache offline");
        assert_eq!(second.path(), first_path);
    }

    #[test]
    fn test_git_cache_refresh_replaces_entry() {
        use super::git_cache_key;

        let (_repo, url) = make_local_git_repo();
        let refspec = "v0.0.1";
        let cache = tempfile::tempdir().unwrap();

        // Pre-seed a stale entry under the real cache key.
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(&url, refspec).unwrap());
        let model = entry.join("model");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("general.yaml"), "stale: true\n").unwrap();

        // Refresh re-clones and atomically replaces the stale entry.
        let refreshed = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some(refspec.to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: false,
                refresh: true,
            },
        )
        .expect("refresh should re-clone and replace the entry");

        assert_eq!(
            std::fs::read_to_string(refreshed.path().join("general.yaml")).unwrap(),
            "groups: []\n",
            "refresh must replace the stale content with the freshly cloned copy"
        );
        // No retired/staging siblings should be left behind.
        let leftovers: Vec<_> = std::fs::read_dir(cache.path().join("git"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with(".old-") || n.starts_with(".staging-")
            })
            .collect();
        assert!(
            leftovers.is_empty(),
            "refresh left temp dirs: {leftovers:?}"
        );
    }

    #[test]
    fn test_git_cache_offline_wins_over_refresh() {
        use super::git_cache_key;

        let cache = tempfile::tempdir().unwrap();
        let url = "https://github.com/open-telemetry/semantic-conventions.git";
        let refspec = "v1.26.0";

        // Pre-seed a cached entry with sentinel content.
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(url, refspec).unwrap());
        let model = entry.join("model");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("general.yaml"), "cached: true\n").unwrap();

        // offline + refresh: offline must win, so the cached entry is served
        // untouched with no network access instead of a re-fetch.
        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            url,
            &Some("model".to_owned()),
            &Some(refspec.to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: true,
                refresh: true,
            },
        )
        .expect("offline should override refresh and serve the cache");

        assert_eq!(vdir.path(), model);
        assert_eq!(
            std::fs::read_to_string(model.join("general.yaml")).unwrap(),
            "cached: true\n",
            "cached content must be served unmodified when offline overrides refresh"
        );
    }

    #[test]
    fn test_git_cache_offline_refresh_miss_errors() {
        use crate::Error::RegistryOffline;

        let cache = tempfile::tempdir().unwrap();
        let result = VirtualDirectory::try_from_git_url_with_cache(
            "https://github.com/open-telemetry/semantic-conventions.git",
            &Some("model".to_owned()),
            &Some("v1.26.0".to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig {
                root: Some(cache.path().to_path_buf()),
                offline: true,
                refresh: true,
            },
        );
        assert!(
            matches!(result, Err(RegistryOffline { .. })),
            "expected RegistryOffline, got {result:?}"
        );
    }

    #[test]
    fn test_source_without_refspec_is_never_cached() {
        let (_repo, url) = make_local_git_repo();
        let cache = tempfile::tempdir().unwrap();
        let config = GitCacheConfig {
            root: Some(cache.path().to_path_buf()),
            offline: false,
            refresh: false,
        };

        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &None,
            "vdir".to_owned(),
            &config,
        )
        .expect("an unpinned source should resolve through a throwaway clone");

        let path = vdir.path().to_path_buf();
        assert!(path.join("general.yaml").exists());
        assert!(
            vdir.tmp_dir.is_some(),
            "an unpinned source must resolve to a temp dir, not a cache entry"
        );
        assert!(
            !cache.path().join("git").exists(),
            "an unpinned source must not create a cache entry"
        );

        drop(vdir);
        assert!(
            !path.exists(),
            "the throwaway clone must be deleted on drop"
        );
    }

    #[test]
    fn test_branch_refspec_is_never_cached() {
        use super::git_cache_key;

        let (_repo, url) = make_local_git_repo();
        let cache = tempfile::tempdir().unwrap();
        let config = GitCacheConfig {
            root: Some(cache.path().to_path_buf()),
            offline: false,
            refresh: false,
        };

        // `main` is a branch: it resolves to different content over time, so it
        // must be served fresh rather than cached.
        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some("main".to_owned()),
            "vdir".to_owned(),
            &config,
        )
        .expect("a branch refspec should resolve through a throwaway clone");

        assert!(vdir.path().join("general.yaml").exists());
        assert!(
            vdir.tmp_dir.is_some(),
            "a branch must resolve to a temp dir, not a cache entry"
        );
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(&url, "main").unwrap());
        assert!(
            !entry.exists(),
            "a branch must not be installed in the cache"
        );
    }

    #[test]
    fn test_tag_refspec_is_cached() {
        use super::git_cache_key;

        let (_repo, url) = make_local_git_repo();
        let cache = tempfile::tempdir().unwrap();
        let config = GitCacheConfig {
            root: Some(cache.path().to_path_buf()),
            offline: false,
            refresh: false,
        };

        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some("v0.0.1".to_owned()),
            "vdir".to_owned(),
            &config,
        )
        .expect("a tag refspec should populate the cache");

        let path = vdir.path().to_path_buf();
        assert!(
            vdir.tmp_dir.is_none(),
            "a tag must resolve to a cache entry"
        );
        let entry = cache
            .path()
            .join("git")
            .join(git_cache_key(&url, "v0.0.1").unwrap());
        assert!(entry.exists());

        drop(vdir);
        assert!(path.exists(), "a cache entry must survive drop");
    }

    #[test]
    fn test_disabled_cache_uses_throwaway_clone() {
        let (_repo, url) = make_local_git_repo();
        let vdir = VirtualDirectory::try_from_git_url_with_cache(
            &url,
            &Some("model".to_owned()),
            &Some("v0.0.1".to_owned()),
            "vdir".to_owned(),
            &GitCacheConfig::default(),
        )
        .expect("the default config disables the cache");

        let path = vdir.path().to_path_buf();
        assert!(vdir.tmp_dir.is_some());
        drop(vdir);
        assert!(!path.exists());
    }

    #[test]
    fn test_configure_git_cache_round_trip() {
        use super::{configure_git_cache, git_cache_config};

        assert!(git_cache_config().root.is_none());
        // A `None` root leaves the cache disabled, so this cannot change how any
        // concurrently running test resolves a git source.
        configure_git_cache(None, true, true);
        let config = git_cache_config();
        assert!(config.root.is_none());
        assert!(config.offline);
        assert!(config.refresh);

        configure_git_cache(None, false, false);
        assert!(!git_cache_config().offline);
        assert!(!git_cache_config().refresh);
    }

    #[test]
    fn test_url_without_userinfo() {
        use super::url_without_userinfo;

        assert_eq!(
            url_without_userinfo("https://x-access-token:secret@github.com/org/repo.git"),
            Some("https://github.com/org/repo.git".to_owned())
        );
        assert_eq!(
            url_without_userinfo("https://user@github.com/org/repo.git"),
            Some("https://github.com/org/repo.git".to_owned())
        );
        assert_eq!(
            url_without_userinfo("https://github.com/org/repo.git"),
            None
        );
        // An `@` in the path is not a credential.
        assert_eq!(
            url_without_userinfo("https://github.com/org/repo@v1.0.git"),
            None
        );
    }

    #[test]
    fn test_strip_url_credentials_rewrites_remote() {
        let dest = tempfile::tempdir().unwrap();
        let url = "https://x-access-token:secret@github.com/org/repo.git";
        let config = dest.path().join(".git").join("config");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, format!("[remote \"origin\"]\n\turl = {url}\n")).unwrap();

        VirtualDirectory::strip_url_credentials(dest.path(), url);

        let contents = std::fs::read_to_string(&config).unwrap();
        assert!(!contents.contains("secret"), "credentials left on disk");
        assert!(contents.contains("https://github.com/org/repo.git"));
    }

    #[test]
    fn test_sibling_cache_path_is_unique_and_prefixed() {
        use super::sibling_cache_path;
        use std::path::Path;

        let git_root = Path::new("/tmp/weaver-cache/git");
        let staged = git_root.join(".staging-abc123");
        let aside = sibling_cache_path(git_root, &staged, ".old-");

        // Sibling under the same root, `.old-` prefix, reusing the unique suffix.
        assert_eq!(aside, git_root.join(".old-abc123"));
        assert_ne!(aside, staged);
    }
}
