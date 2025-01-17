// SPDX-License-Identifier: Apache-2.0

//! Logs change definitions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the logs for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogsChanges {
    /// Changes to apply to the logs for a specific version.
    pub changes: Vec<LogsChange>,
}

/// Changes to apply to the logs for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogsChange {
    /// A collection of rename operations to apply to the log attributes.
    pub rename_attributes: RenameAttributes,
}

/// A collection of rename operations to apply to the log attributes.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the log attributes.
    pub attribute_map: HashMap<String, String>,
}
