// SPDX-License-Identifier: Apache-2.0

//! Spans change specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Changes to apply to the spans specification for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct SpansChange {
    /// Changes to apply to the span attributes for a specific version.
    pub rename_attributes: RenameAttributes,
}

/// Changes to apply to the span attributes for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct RenameAttributes {
    /// A collection of rename operations to apply to the span attributes.
    pub attribute_map: HashMap<String, String>,
}
