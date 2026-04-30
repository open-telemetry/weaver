// SPDX-License-Identifier: Apache-2.0

//! Shared configuration for registry, policy, and diagnostic settings.
//!
//! These sections apply to all subcommands that accept them (check, generate,
//! live-check, etc.). CLI flags always take precedence over config values.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Registry configuration — where to load the semantic convention registry from.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct RegistryConfig {
    /// Local folder, Git repo URL, or Git archive URL.
    pub path: Option<String>,
    /// Follow symlinks when loading the registry.
    pub follow_symlinks: Option<bool>,
    /// Include signals and attributes from dependency registries even if
    /// not explicitly referenced.
    pub include_unreferenced: Option<bool>,
    /// Use version 2 of the schema.
    pub v2: Option<bool>,
}

/// Policy configuration — which policy files to check against.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct PolicyConfig {
    /// Policy file or directory paths. Directories load all `.rego` files.
    pub paths: Option<Vec<String>>,
    /// Skip policy checks entirely.
    pub skip: Option<bool>,
}

/// Diagnostic output configuration.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct DiagnosticsConfig {
    /// Format for diagnostic messages: `ansi`, `json`, `gh_workflow_command`.
    pub format: Option<String>,
    /// Path to the directory where the diagnostic templates are located.
    pub template: Option<PathBuf>,
    /// Send diagnostic output to stdout instead of stderr.
    pub stdout: Option<bool>,
}

#[cfg(test)]
mod tests {
    use crate::WeaverConfig;

    #[test]
    fn test_parse_shared_sections() {
        let toml = r#"
[registry]
path = "https://github.com/open-telemetry/semantic-conventions.git"
follow_symlinks = true
include_unreferenced = true
v2 = true

[policy]
paths = ["./policies", "./extra_policies"]
skip = true

[diagnostics]
format = "json"
template = "my_templates"
stdout = true
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");

        let reg = &config.registry;
        assert_eq!(
            reg.path.as_deref(),
            Some("https://github.com/open-telemetry/semantic-conventions.git")
        );
        assert_eq!(reg.follow_symlinks, Some(true));
        assert_eq!(reg.include_unreferenced, Some(true));
        assert_eq!(reg.v2, Some(true));

        let pol = &config.policy;
        assert_eq!(
            pol.paths.as_deref(),
            Some(&["./policies".to_owned(), "./extra_policies".to_owned()][..])
        );
        assert_eq!(pol.skip, Some(true));

        let diag = &config.diagnostics;
        assert_eq!(diag.format.as_deref(), Some("json"));
        assert_eq!(
            diag.template.as_deref(),
            Some(std::path::Path::new("my_templates"))
        );
        assert_eq!(diag.stdout, Some(true));
    }

    #[test]
    fn test_parse_partial_shared_sections() {
        let toml = r#"
[registry]
v2 = true
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert_eq!(config.registry.v2, Some(true));
        assert!(config.registry.path.is_none());
        assert!(config.registry.follow_symlinks.is_none());
        assert!(config.policy.paths.is_none());
        assert!(config.diagnostics.format.is_none());
    }

    #[test]
    fn test_empty_config_has_default_shared_sections() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert!(config.registry.path.is_none());
        assert!(config.policy.paths.is_none());
        assert!(config.diagnostics.format.is_none());
    }
}
