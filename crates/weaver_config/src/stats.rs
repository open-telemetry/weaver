// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry stats` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Stats-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct StatsConfig {
    /// Output format for the stats (e.g. text, json, yaml, jsonl, mute).
    pub format: String,
    /// Path to the directory where the stats templates are located.
    pub templates: PathBuf,
    /// Path to the directory where the generated artifacts will be saved.
    pub output: Option<PathBuf>,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            format: "text".to_owned(),
            templates: PathBuf::from("stats_templates"),
            output: None,
        }
    }
}
