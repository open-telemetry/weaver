// SPDX-License-Identifier: Apache-2.0

//! Changes to apply to the spans specification for a specific version.

use crate::spans_change::SpansChange;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Changes to apply to the spans specification for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpansVersion {
    /// Changes to apply to the spans specification for a specific version.
    pub changes: Vec<SpansChange>,
}
