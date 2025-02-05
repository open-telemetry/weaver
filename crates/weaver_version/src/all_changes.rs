// SPDX-License-Identifier: Apache-2.0

//! Section "all" changes in the OpenTelemetry Schema file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the attributes of resource attributes, span attributes,
/// event attributes, log attributes, and metric attributes.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AllChanges {
    /// Changes to apply to the attributes.
    pub changes: Vec<AllChange>,
}

/// Changes to apply to the attributes for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AllChange {
    /// Changes to apply to the resource attributes for a specific version.
    pub rename_attributes: RenameAttributes,
}

/// Changes to apply to the attributes for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the resource attributes.
    pub attribute_map: HashMap<String, String>,
}
