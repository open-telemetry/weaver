// SPDX-License-Identifier: Apache-2.0

//! Project-level configuration for Weaver via `.weaver.toml`.
//!
//! Discovery walks up from the current working directory to find the first
//! `.weaver.toml` file. The `--config` CLI option can override this.

use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub mod auth;
pub mod live_check;
mod overrides;
pub mod registry;

// Re-export the public API so callers can use `weaver_config::LiveCheckConfig` etc.
pub use auth::{build_resolver as build_auth_resolver, AuthEntry};
pub use weaver_common::http_auth::TokenSource;
pub use live_check::{FindingFilter, LiveCheckConfig, LiveCheckEmitConfig, LiveCheckOtlpConfig};
pub use overrides::{CliOverrides, FieldMapping};
pub use registry::{DiagnosticsConfig, PolicyConfig, RegistryConfig};

/// The filename to search for during discovery.
const CONFIG_FILENAME: &str = ".weaver.toml";

/// Top-level Weaver configuration.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct WeaverConfig {
    /// Shared registry settings (apply to all subcommands that accept them).
    pub registry: RegistryConfig,
    /// Shared policy settings (apply to all subcommands that accept them).
    pub policy: PolicyConfig,
    /// Shared diagnostic output settings (apply to all subcommands that accept them).
    pub diagnostics: DiagnosticsConfig,
    /// Live-check specific configuration.
    pub live_check: LiveCheckConfig,
    /// Per-URL HTTP authentication entries for downloading remote registries.
    /// See [`auth::AuthEntry`] for the schema.
    pub auth: Vec<AuthEntry>,
}

/// Discover a `.weaver.toml` file by walking up from the given directory.
///
/// Returns the path to the first `.weaver.toml` found, or `None` if none exists.
#[must_use]
pub fn discover(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load a `.weaver.toml` from the given path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub fn load(path: &Path) -> Result<WeaverConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Discover and load a `.weaver.toml` starting from the given directory.
///
/// Returns `None` if no config file is found. When a config is found, returns
/// both the parsed config and the path it was loaded from.
///
/// # Errors
///
/// Returns an error if the discovered file cannot be read or parsed.
pub fn discover_and_load(start: &Path) -> Result<Option<(PathBuf, WeaverConfig)>, ConfigError> {
    match discover(start) {
        Some(path) => {
            let config = load(&path)?;
            Ok(Some((path, config)))
        }
        None => Ok(None),
    }
}

/// Errors from config loading.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigError {
    /// IO error reading the config file.
    #[error("Failed to read config '{}': {reason}", path.display())]
    Io {
        /// The path that failed to read.
        path: PathBuf,
        /// The error message.
        reason: String,
    },
    /// Parse error in the TOML config.
    #[error("Failed to parse config '{}': {reason}", path.display())]
    Parse {
        /// The path that failed to parse.
        path: PathBuf,
        /// The error message.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_walks_up() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).expect("Failed to create dirs");

        fs::write(dir.path().join(CONFIG_FILENAME), "[live_check]")
            .expect("Failed to write config");

        let found = discover(&nested);
        assert_eq!(found, Some(dir.path().join(CONFIG_FILENAME)));
    }

    #[test]
    fn test_discover_none() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let found = discover(dir.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_load_and_discover() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let config_content = r#"
[[live_check.finding_overrides]]
id = ["deprecated"]
level = "information"
"#;
        fs::write(dir.path().join(CONFIG_FILENAME), config_content)
            .expect("Failed to write config");

        let (path, _config) = discover_and_load(dir.path())
            .expect("Failed to load config")
            .expect("Config should be found");
        assert_eq!(path, dir.path().join(CONFIG_FILENAME));
    }
}
