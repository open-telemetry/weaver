// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry generate` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Generate-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct GenerateConfig {
    /// Path to the directory where the templates are located.
    pub templates: String,
    /// Target to generate artifacts for.
    pub target: String,
    /// Path to the output directory.
    pub output: PathBuf,
}

impl Default for GenerateConfig {
    fn default() -> Self {
        Self {
            templates: "templates".to_owned(),
            target: String::new(),
            output: PathBuf::from("output"),
        }
    }
}
