// SPDX-License-Identifier: Apache-2.0

//! Project-level template settings (`[template]` in `.weaver.toml`).
//!
//! These settings apply on top of every template package used by the project,
//! combining with the package's own `weaver.yaml` (aka `weaver_template.yaml`)
//! defaults. They let a project standardize template behavior — such as the
//! list of acronyms used by the `acronym` filter, or the `text_maps` used by
//! the `map_text` filter — without editing each package.
//!
//! Only `acronyms` and `text_maps` are wired today. Additional settings from
//! the design (`template_syntax`, `whitespace_control`, `params`) can be added
//! here as they are implemented.

use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

/// Project-level template settings shared across all template packages.
///
/// A value left unset (`None`) does not override the package configuration;
/// the package's own `weaver.yaml` value is kept.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct TemplateConfig {
    /// List of acronyms treated as unmodifiable words during case conversion.
    pub acronyms: Option<Vec<String>>,

    /// Named text mappings used by the `map_text` filter (e.g. a
    /// `namespace_mapping` from `CICD` to `CI/CD`).
    pub text_maps: Option<HashMap<String, HashMap<String, String>>>,
}

#[cfg(test)]
mod tests {
    use crate::WeaverConfig;

    #[test]
    fn test_parse_template_config() {
        let toml = r#"
[template]
acronyms = ["API", "HTTP", "SDK", "iOS"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert_eq!(
            config.template.acronyms.as_deref(),
            Some(
                &[
                    "API".to_owned(),
                    "HTTP".to_owned(),
                    "SDK".to_owned(),
                    "iOS".to_owned()
                ][..]
            )
        );
    }

    #[test]
    fn test_parse_template_text_maps() {
        let toml = r#"
[template.text_maps.namespace_mapping]
CICD = "CI/CD"
"CICD Pipeline" = "CI/CD Pipeline"
"CICD Pipeline Run" = "CI/CD Pipeline Run"
"CICD Worker" = "CI/CD Worker"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let text_maps = config.template.text_maps.expect("text_maps should be set");
        let namespace_mapping = text_maps
            .get("namespace_mapping")
            .expect("namespace_mapping should be set");
        assert_eq!(
            namespace_mapping.get("CICD").map(String::as_str),
            Some("CI/CD")
        );
        assert_eq!(
            namespace_mapping.get("CICD Pipeline").map(String::as_str),
            Some("CI/CD Pipeline")
        );
        assert_eq!(namespace_mapping.len(), 4);
    }

    #[test]
    fn test_template_absent() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert!(config.template.acronyms.is_none());
        assert!(config.template.text_maps.is_none());
    }

    #[test]
    fn test_template_empty_section() {
        let config: WeaverConfig = toml::from_str("[template]").expect("Failed to parse TOML");
        assert!(config.template.acronyms.is_none());
    }
}
