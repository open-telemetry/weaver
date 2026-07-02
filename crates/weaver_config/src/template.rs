// SPDX-License-Identifier: Apache-2.0

//! Project-level template settings (`[template]` in `.weaver.toml`).
//!
//! These settings apply on top of every template package used by the project,
//! combining with the package's own `weaver.yaml` (aka `weaver_template.yaml`)
//! defaults. They let a project standardize template behavior — such as the
//! list of acronyms used by the `acronym` filter — without editing each
//! package. `acronyms` is merged (unioned) with the package's list, with the
//! project taking precedence on conflicts.
//!
//! Only `acronyms` is wired today. Additional settings from the design
//! (`template_syntax`, `whitespace_control`, `params`) can be added here as
//! they are implemented.

use schemars::JsonSchema;
use serde::Deserialize;

/// Project-level template settings shared across all template packages.
///
/// A value left unset (`None`) does not override the package configuration;
/// the package's own `weaver.yaml` value is kept.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct TemplateConfig {
    /// List of acronyms treated as unmodifiable words during case conversion.
    /// When set, these are merged into the package's own `acronyms` list (the
    /// project wins on case-insensitive conflicts).
    pub acronyms: Option<Vec<String>>,
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
    fn test_template_absent() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert!(config.template.acronyms.is_none());
    }

    #[test]
    fn test_template_empty_section() {
        let config: WeaverConfig = toml::from_str("[template]").expect("Failed to parse TOML");
        assert!(config.template.acronyms.is_none());
    }
}
