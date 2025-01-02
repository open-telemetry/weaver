// SPDX-License-Identifier: Apache-2.0

//! Resource change definitions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the resource for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceChanges {
    /// Changes to apply to the resource for a specific version.
    pub changes: Vec<ResourceChange>,
}

/// Changes to apply to the resources for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceChange {
    /// Changes to apply to the resource attributes for a specific version.
    pub rename_attributes: RenameAttributes,
}

/// Changes to apply to the resource attributes for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the resource attributes.
    pub attribute_map: HashMap<String, String>,
}
