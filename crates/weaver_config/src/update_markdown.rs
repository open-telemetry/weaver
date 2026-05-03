// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry update-markdown` command.

use schemars::JsonSchema;
use serde::Deserialize;

/// Update-markdown-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct UpdateMarkdownConfig {
    /// Path to the directory where the markdown files are located.
    pub markdown_dir: Option<String>,
    /// Whether to run updates in dry-run mode.
    pub dry_run: bool,
    /// Optional path to the attribute registry base URL.
    pub attribute_registry_base_url: Option<String>,
    /// Path to the directory where the templates are located.
    pub templates: String,
    /// Target to generate snippets with.
    pub target: Option<String>,
}

impl Default for UpdateMarkdownConfig {
    fn default() -> Self {
        Self {
            markdown_dir: None,
            dry_run: false,
            attribute_registry_base_url: None,
            templates: "templates".to_owned(),
            target: None,
        }
    }
}
