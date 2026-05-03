// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry emit` command.

use schemars::JsonSchema;
use serde::Deserialize;

/// Emit-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct EmitConfig {
    /// Endpoint for the OTLP receiver.
    pub endpoint: String,
    /// Write to stdout instead of sending via OTLP.
    pub stdout: bool,
}

impl Default for EmitConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:4317".to_owned(),
            stdout: false,
        }
    }
}
