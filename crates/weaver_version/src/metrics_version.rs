// SPDX-License-Identifier: Apache-2.0

//! Metrics version.

use crate::metrics_change::MetricsChange;
use serde::{Deserialize, Serialize};

/// Changes to apply to the metrics for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct MetricsVersion {
    /// Changes to apply to the metrics for a specific version.
    pub changes: Vec<MetricsChange>,
}
