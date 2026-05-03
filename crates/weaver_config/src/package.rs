// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry package` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Package-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct PackageConfig {
    /// Path to the directory where the package will be written.
    pub output: PathBuf,
    /// URI where the resolved schema will eventually be published.
    pub resolved_schema_uri: Option<String>,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            output: PathBuf::from("output"),
            resolved_schema_uri: None,
        }
    }
}
