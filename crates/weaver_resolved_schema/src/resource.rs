// SPDX-License-Identifier: Apache-2.0

//! Define an OpenTelemetry resource.

use crate::attribute::AttributeRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Definition of attributes associated with the resource.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    /// List of references to attributes present in the shared catalog.
    pub attributes: Vec<AttributeRef>,
}
