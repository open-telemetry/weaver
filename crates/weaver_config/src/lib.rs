// SPDX-License-Identifier: Apache-2.0

//! Project-level configuration for Weaver via `.weaver.toml`.
//!
//! Discovery walks up from the current working directory to find the first
//! `.weaver.toml` file. The `--config` CLI option can override this.

use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use weaver_checker::FindingLevel;

/// The filename to search for during discovery.
const CONFIG_FILENAME: &str = ".weaver.toml";

/// Top-level Weaver configuration.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct WeaverConfig {
    /// Live-check specific configuration.
    pub live_check: Option<LiveCheckConfig>,
}

/// Configuration for the live-check subcommand.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct LiveCheckConfig {
    /// Overrides modify finding levels. Each targets findings by ID (or list of IDs).
    /// Optional `signal_type` scopes the override to a specific signal type.
    #[serde(default)]
    pub finding_overrides: Vec<FindingOverride>,

    /// Filters control which findings are dropped. A filter without `signal_type`
    /// applies globally; a filter with `signal_type` applies only to that signal type.
    #[serde(default)]
    pub finding_filters: Vec<FindingFilter>,
}

/// An override that modifies the level of findings matching the given ID(s).
/// Optional `signal_type` scopes the override to a specific signal type.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
pub struct FindingOverride {
    /// The finding IDs to match.
    pub id: Vec<String>,
    /// The new level to assign to matching findings.
    pub level: FindingLevel,
    /// Optional signal type scope (e.g., "span", "metric", "log").
    pub signal_type: Option<String>,
}

/// A filter that drops findings by ID exclusion or minimum level.
/// Optional `signal_type` scopes the filter to a specific signal type.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
pub struct FindingFilter {
    /// Drop findings with these IDs.
    pub exclude: Option<Vec<String>>,
    /// Drop all findings below this level.
    pub min_level: Option<FindingLevel>,
    /// Optional signal type scope. When set, this filter only applies to
    /// findings with a matching signal_type.
    pub signal_type: Option<String>,
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
/// Returns `None` if no config file is found. Returns an error if a file is found
/// but cannot be parsed.
///
/// # Errors
///
/// Returns an error if the discovered file cannot be read or parsed.
pub fn discover_and_load(start: &Path) -> Result<Option<WeaverConfig>, ConfigError> {
    match discover(start) {
        Some(path) => {
            log::info!("Found config file: {}", path.display());
            load(&path).map(Some)
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
    fn test_parse_full_config() {
        let toml = r#"
[[live_check.finding_overrides]]
id = ["not_stable", "missing_attribute"]
level = "violation"

[[live_check.finding_overrides]]
id = ["not_stable"]
level = "information"
signal_type = "span"

# Global filter (no signal_type)
[[live_check.finding_filters]]
exclude = ["deprecated", "missing_namespace"]
min_level = "improvement"

# Scoped filter (with signal_type)
[[live_check.finding_filters]]
signal_type = "span"
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let live_check = config.live_check.expect("live_check should be present");

        assert_eq!(live_check.finding_overrides.len(), 2);
        assert_eq!(
            live_check.finding_overrides[0].id,
            vec!["not_stable".to_owned(), "missing_attribute".to_owned()]
        );
        assert_eq!(
            live_check.finding_overrides[0].level,
            FindingLevel::Violation
        );
        assert!(live_check.finding_overrides[0].signal_type.is_none());

        assert_eq!(
            live_check.finding_overrides[1].id,
            vec!["not_stable".to_owned()]
        );
        assert_eq!(
            live_check.finding_overrides[1].level,
            FindingLevel::Information
        );
        assert_eq!(
            live_check.finding_overrides[1].signal_type.as_deref(),
            Some("span")
        );

        assert_eq!(live_check.finding_filters.len(), 2);

        // Global filter (no signal_type)
        assert!(live_check.finding_filters[0].signal_type.is_none());
        assert_eq!(
            live_check.finding_filters[0].exclude.as_deref(),
            Some(&["deprecated".to_owned(), "missing_namespace".to_owned()][..])
        );
        assert_eq!(
            live_check.finding_filters[0].min_level,
            Some(FindingLevel::Improvement)
        );

        // Scoped filter (with signal_type)
        assert_eq!(
            live_check.finding_filters[1].signal_type.as_deref(),
            Some("span")
        );
        assert_eq!(
            live_check.finding_filters[1].exclude.as_deref(),
            Some(&["not_stable".to_owned()][..])
        );
    }

    #[test]
    fn test_parse_empty_config() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert!(config.live_check.is_none());
    }

    #[test]
    fn test_parse_partial_config() {
        let toml = r#"
[[live_check.finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let live_check = config.live_check.expect("live_check should be present");
        assert!(live_check.finding_overrides.is_empty());
        assert_eq!(live_check.finding_filters.len(), 1);
        assert_eq!(
            live_check.finding_filters[0].min_level,
            Some(FindingLevel::Violation)
        );
        assert!(live_check.finding_filters[0].exclude.is_none());
        assert!(live_check.finding_filters[0].signal_type.is_none());
    }

    #[test]
    fn test_id_list() {
        let toml = r#"
[[live_check.finding_overrides]]
id = ["a", "b", "c"]
level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let live_check = config.live_check.expect("live_check");
        assert_eq!(
            live_check.finding_overrides[0].id,
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
    }

    #[test]
    fn test_discover_walks_up() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).expect("Failed to create dirs");

        // Place config at the root of the temp dir
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

        let config = discover_and_load(dir.path())
            .expect("Failed to load config")
            .expect("Config should be found");
        assert!(config.live_check.is_some());
    }
}
