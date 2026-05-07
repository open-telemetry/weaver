// SPDX-License-Identifier: Apache-2.0

//! Effective config types — concrete resolved settings for registry, policy, and diagnostics.
//!
//! Each type is built via three-layer merge: defaults → `.weaver.toml` → CLI overrides.

use std::path::PathBuf;

use weaver_common::vdir::VirtualDirectoryPath;

use crate::registry::{DiagnosticsConfig, PolicyConfig, RegistryConfig};

/// Default registry URL used when no registry is specified.
pub const DEFAULT_REGISTRY: &str =
    "https://github.com/open-telemetry/semantic-conventions.git[model]";

/// Default diagnostic format.
pub const DEFAULT_DIAGNOSTIC_FORMAT: &str = "ansi";

/// Default diagnostic template directory name.
pub const DEFAULT_DIAGNOSTIC_TEMPLATE: &str = "diagnostic_templates";

/// Effective registry settings — every field has a concrete value.
///
/// Built by layering: defaults → `.weaver.toml` → CLI overrides.
#[derive(Debug, Clone)]
pub struct EffectiveRegistryConfig {
    /// The registry path (local folder or Git URL).
    pub registry: VirtualDirectoryPath,
    /// Whether to follow symlinks when loading the registry.
    pub follow_symlinks: bool,
    /// Whether to include unreferenced signals from dependency registries.
    pub include_unreferenced: bool,
    /// Whether to use v2 schema.
    pub v2: bool,
}

impl Default for EffectiveRegistryConfig {
    fn default() -> Self {
        Self {
            registry: DEFAULT_REGISTRY
                .parse()
                .expect("DEFAULT_REGISTRY is a valid VirtualDirectoryPath"),
            follow_symlinks: false,
            include_unreferenced: false,
            v2: false,
        }
    }
}

impl EffectiveRegistryConfig {
    /// Apply `.weaver.toml` registry section onto this effective config (layer 2).
    pub fn layer_config(&mut self, cfg: &RegistryConfig) {
        if let Some(path) = &cfg.path {
            if let Ok(parsed) = path.parse() {
                self.registry = parsed;
            }
        }
        if let Some(v) = cfg.follow_symlinks {
            self.follow_symlinks = v;
        }
        if let Some(v) = cfg.include_unreferenced {
            self.include_unreferenced = v;
        }
        if let Some(v) = cfg.v2 {
            self.v2 = v;
        }
    }
}

/// Effective policy settings — every field has a concrete value.
///
/// Built by layering: defaults → `.weaver.toml` → CLI overrides.
#[derive(Debug, Clone, Default)]
pub struct EffectivePolicyConfig {
    /// Policy file or directory paths.
    pub policies: Vec<VirtualDirectoryPath>,
    /// Whether to skip policy checks.
    pub skip_policies: bool,
    /// Whether to display the policy coverage report.
    pub display_policy_coverage: bool,
}

impl EffectivePolicyConfig {
    /// Returns an effective policy config that always skips policy checks.
    #[must_use]
    pub fn skip_all() -> Self {
        Self {
            skip_policies: true,
            ..Default::default()
        }
    }

    /// Apply `.weaver.toml` policy section onto this effective config (layer 2).
    pub fn layer_config(&mut self, cfg: &PolicyConfig) {
        if let Some(paths) = &cfg.paths {
            self.policies = paths.iter().filter_map(|p| p.parse().ok()).collect();
        }
        if let Some(v) = cfg.skip {
            self.skip_policies = v;
        }
        if let Some(v) = cfg.display_policy_coverage {
            self.display_policy_coverage = v;
        }
    }
}

/// Effective diagnostic settings — every field has a concrete value.
///
/// Built by layering: defaults → `.weaver.toml` → CLI overrides.
#[derive(Debug, Clone)]
pub struct EffectiveDiagnosticConfig {
    /// The diagnostic format (e.g. `ansi`, `json`, `gh_workflow_command`).
    pub diagnostic_format: String,
    /// Path to the diagnostic templates directory.
    pub diagnostic_template: PathBuf,
    /// Whether to send diagnostics to stdout instead of stderr.
    pub diagnostic_stdout: bool,
}

impl Default for EffectiveDiagnosticConfig {
    fn default() -> Self {
        Self {
            diagnostic_format: DEFAULT_DIAGNOSTIC_FORMAT.to_owned(),
            diagnostic_template: PathBuf::from(DEFAULT_DIAGNOSTIC_TEMPLATE),
            diagnostic_stdout: false,
        }
    }
}

impl EffectiveDiagnosticConfig {
    /// Apply `.weaver.toml` diagnostics section onto this effective config (layer 2).
    pub fn layer_config(&mut self, cfg: &DiagnosticsConfig) {
        if let Some(format) = &cfg.format {
            self.diagnostic_format.clone_from(format);
        }
        if let Some(template) = &cfg.template {
            self.diagnostic_template.clone_from(template);
        }
        if let Some(v) = cfg.stdout {
            self.diagnostic_stdout = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::registry::{DiagnosticsConfig, PolicyConfig, RegistryConfig};

    // ── EffectiveRegistryConfig ───────────────────────────────────────────────

    #[test]
    fn test_registry_default_values() {
        let cfg = EffectiveRegistryConfig::default();
        assert!(!cfg.follow_symlinks);
        assert!(!cfg.include_unreferenced);
        assert!(!cfg.v2);
        // Verify the default registry URL parses correctly (smoke-check).
        assert!(
            format!("{cfg:?}").contains("GitRepo"),
            "default registry should be a GitRepo"
        );
    }

    #[test]
    fn test_registry_layer_config_applies_all_fields() {
        let mut cfg = EffectiveRegistryConfig::default();
        cfg.layer_config(&RegistryConfig {
            path: Some("./local".to_owned()),
            follow_symlinks: Some(true),
            include_unreferenced: Some(true),
            v2: Some(true),
        });
        assert!(cfg.follow_symlinks);
        assert!(cfg.include_unreferenced);
        assert!(cfg.v2);
    }

    #[test]
    fn test_registry_layer_config_skips_invalid_path() {
        let mut cfg = EffectiveRegistryConfig::default();
        let original_registry = cfg.registry.clone();
        // An empty string is not a valid VirtualDirectoryPath — should be ignored.
        cfg.layer_config(&RegistryConfig {
            path: Some(String::new()),
            ..Default::default()
        });
        assert_eq!(
            format!("{:?}", cfg.registry),
            format!("{:?}", original_registry),
            "invalid path should leave registry unchanged"
        );
    }

    #[test]
    fn test_registry_layer_config_partial() {
        let mut cfg = EffectiveRegistryConfig::default();
        cfg.layer_config(&RegistryConfig {
            follow_symlinks: Some(true),
            ..Default::default()
        });
        assert!(cfg.follow_symlinks);
        assert!(!cfg.include_unreferenced);
        assert!(!cfg.v2);
    }

    // ── EffectivePolicyConfig ─────────────────────────────────────────────────

    #[test]
    fn test_policy_default_values() {
        let cfg = EffectivePolicyConfig::default();
        assert!(!cfg.skip_policies);
        assert!(!cfg.display_policy_coverage);
        assert!(cfg.policies.is_empty());
    }

    #[test]
    fn test_policy_skip_all() {
        let cfg = EffectivePolicyConfig::skip_all();
        assert!(cfg.skip_policies);
        assert!(!cfg.display_policy_coverage);
        assert!(cfg.policies.is_empty());
    }

    #[test]
    fn test_policy_layer_config_applies_all_fields() {
        let mut cfg = EffectivePolicyConfig::default();
        cfg.layer_config(&PolicyConfig {
            paths: Some(vec!["./policies".to_owned()]),
            skip: Some(true),
            display_policy_coverage: Some(true),
        });
        assert!(cfg.skip_policies);
        assert!(cfg.display_policy_coverage);
        assert_eq!(cfg.policies.len(), 1);
    }

    #[test]
    fn test_policy_layer_config_ignores_invalid_paths() {
        let mut cfg = EffectivePolicyConfig::default();
        // Empty strings are not valid VirtualDirectoryPath — filtered out.
        cfg.layer_config(&PolicyConfig {
            paths: Some(vec![String::new()]),
            ..Default::default()
        });
        assert!(cfg.policies.is_empty());
    }

    #[test]
    fn test_policy_layer_config_partial() {
        let mut cfg = EffectivePolicyConfig::default();
        cfg.layer_config(&PolicyConfig {
            skip: Some(true),
            ..Default::default()
        });
        assert!(cfg.skip_policies);
        assert!(!cfg.display_policy_coverage);
    }

    // ── EffectiveDiagnosticConfig ─────────────────────────────────────────────

    #[test]
    fn test_diagnostic_default_values() {
        let cfg = EffectiveDiagnosticConfig::default();
        assert_eq!(cfg.diagnostic_format, DEFAULT_DIAGNOSTIC_FORMAT);
        assert_eq!(
            cfg.diagnostic_template,
            PathBuf::from(DEFAULT_DIAGNOSTIC_TEMPLATE)
        );
        assert!(!cfg.diagnostic_stdout);
    }

    #[test]
    fn test_diagnostic_layer_config_applies_all_fields() {
        let mut cfg = EffectiveDiagnosticConfig::default();
        cfg.layer_config(&DiagnosticsConfig {
            format: Some("json".to_owned()),
            template: Some(PathBuf::from("my_templates")),
            stdout: Some(true),
        });
        assert_eq!(cfg.diagnostic_format, "json");
        assert_eq!(cfg.diagnostic_template, PathBuf::from("my_templates"));
        assert!(cfg.diagnostic_stdout);
    }

    #[test]
    fn test_diagnostic_layer_config_partial() {
        let mut cfg = EffectiveDiagnosticConfig::default();
        cfg.layer_config(&DiagnosticsConfig {
            format: Some("gh_workflow_command".to_owned()),
            ..Default::default()
        });
        assert_eq!(cfg.diagnostic_format, "gh_workflow_command");
        assert_eq!(
            cfg.diagnostic_template,
            PathBuf::from(DEFAULT_DIAGNOSTIC_TEMPLATE)
        );
        assert!(!cfg.diagnostic_stdout);
    }
}
