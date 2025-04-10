// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};

use crate::sample_attribute::SampleAttribute;

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpan {
    /// The span's attributes
    pub attributes: Vec<SampleAttribute>,
}
