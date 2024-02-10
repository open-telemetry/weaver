// SPDX-License-Identifier: Apache-2.0

//! Logs version.

use crate::logs_change::LogsChange;
use serde::{Deserialize, Serialize};

/// Changes to apply to the logs for a specific version.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct LogsVersion {
    /// Changes to apply to the logs for a specific version.
    pub changes: Vec<LogsChange>,
}
