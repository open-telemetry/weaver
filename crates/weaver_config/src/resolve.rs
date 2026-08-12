// SPDX-License-Identifier: Apache-2.0

//! Resolution configuration (`[resolve]` / `[resolution]` in `.weaver.toml`).
//!
//! Controls dependency resolution and schema URL overrides.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

/// Resolution configuration — how dependencies and schema URLs are resolved.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct ResolveConfig {
    /// Explicit overrides mapping a requested schema URL to an alternative
    /// directory, Git repository, or archive path.
    ///
    /// ```toml
    /// [resolve.schema_url_overrides]
    /// "https://opentelemetry.io/schemas/1.25.0" = "path/to/local/1.25.0"
    /// "https://opentelemetry.io/schemas/1.26.0" = "https://github.com/my-fork/semconv.git[model]"
    /// ```
    #[serde(default, alias = "overrides", alias = "dependency_overrides")]
    pub schema_url_overrides: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeaverConfig;

    #[test]
    fn test_parse_resolve_schema_url_overrides() {
        let toml = r#"
[resolve.schema_url_overrides]
"https://opentelemetry.io/schemas/1.25.0" = "path/to/local/1.25.0"
"https://opentelemetry.io/schemas/1.26.0" = "https://github.com/my-fork/semconv.git[model]"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert_eq!(config.resolve.schema_url_overrides.len(), 2);
        assert_eq!(
            config
                .resolve
                .schema_url_overrides
                .get("https://opentelemetry.io/schemas/1.25.0")
                .map(|s| s.as_str()),
            Some("path/to/local/1.25.0")
        );
        assert_eq!(
            config
                .resolve
                .schema_url_overrides
                .get("https://opentelemetry.io/schemas/1.26.0")
                .map(|s| s.as_str()),
            Some("https://github.com/my-fork/semconv.git[model]")
        );
    }

    #[test]
    fn test_parse_resolve_overrides_alias() {
        let toml = r#"
[resolve.overrides]
"https://opentelemetry.io/schemas/1.25.0" = "path/to/local/1.25.0"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert_eq!(
            config
                .resolve
                .schema_url_overrides
                .get("https://opentelemetry.io/schemas/1.25.0")
                .map(|s| s.as_str()),
            Some("path/to/local/1.25.0")
        );
    }

    #[test]
    fn test_parse_resolution_alias() {
        let toml = r#"
[resolution.schema_url_overrides]
"https://opentelemetry.io/schemas/1.25.0" = "path/to/local/1.25.0"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert_eq!(
            config
                .resolve
                .schema_url_overrides
                .get("https://opentelemetry.io/schemas/1.25.0")
                .map(|s| s.as_str()),
            Some("path/to/local/1.25.0")
        );
    }

    #[test]
    fn test_empty_resolve_config() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert!(config.resolve.schema_url_overrides.is_empty());
    }
}
