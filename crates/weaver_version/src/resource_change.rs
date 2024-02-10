// SPDX-License-Identifier: Apache-2.0

//! Changes to apply to the resources for a specific version.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the resources for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResourceChange {
    /// Changes to apply to the resource attributes for a specific version.
    pub rename_attributes: RenameAttributes,
}

/// Changes to apply to the resource attributes for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the resource attributes.
    pub attribute_map: HashMap<String, String>,
}
