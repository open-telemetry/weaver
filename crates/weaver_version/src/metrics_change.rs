// SPDX-License-Identifier: Apache-2.0

//! Metrics change definitions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the metrics for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricsChange {
    /// A collection of rename operations to apply to the metric attributes.
    #[serde(default)]
    pub rename_attributes: RenameAttributes,
    /// A collection of rename operations to apply to the metric names.
    #[serde(default)]
    pub rename_metrics: HashMap<String, String>,
}

/// A collection of rename operations to apply to the metric attributes.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the metric attributes.
    pub attribute_map: HashMap<String, String>,
    /// A collection of metric references.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub apply_to_metrics: Vec<String>,
}
