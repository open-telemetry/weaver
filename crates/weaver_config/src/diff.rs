// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry diff` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Diff-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct DiffConfig {
    /// Format used to render the schema changes (e.g. ansi, json, markdown).
    pub format: String,
    /// Path to the directory where the schema changes templates are located.
    pub templates: PathBuf,
    /// Path to the directory where the generated artifacts will be saved.
    pub output: Option<PathBuf>,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            format: "ansi".to_owned(),
            templates: PathBuf::from("diff_templates"),
            output: None,
        }
    }
}
