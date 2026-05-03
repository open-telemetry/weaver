// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry mcp` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// MCP-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct McpConfig {
    /// Advice policies directory.
    pub advice_policies: Option<PathBuf>,
    /// Advice preprocessor jq script.
    pub advice_preprocessor: Option<PathBuf>,
    /// Namespace separator used in attribute keys.
    pub namespace_separator: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            advice_policies: None,
            advice_preprocessor: None,
            namespace_separator: ".".to_owned(),
        }
    }
}
