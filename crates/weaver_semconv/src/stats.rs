// SPDX-License-Identifier: Apache-2.0

//! Statistics about the semantic convention registry.

use std::collections::HashMap;
use crate::group::GroupType;

/// Statistics about the semantic convention registry.
#[must_use]
pub struct Stats {
    /// Number of semconv files.
    pub file_count: usize,
    /// Number of semconv groups.
    pub group_count: usize,
    /// Breakdown of group statistics by type.
    pub group_breakdown: HashMap<GroupType, usize>,
    /// Number of attributes.
    pub attribute_count: usize,
    /// Number of metrics.
    pub metric_count: usize,
}
