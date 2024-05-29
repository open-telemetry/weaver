// SPDX-License-Identifier: Apache-2.0

//! Define the concept of tag.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use schemars::JsonSchema;

/// A set of tags.
///
/// Examples of tags:
/// - sensitivity: pii
/// - sensitivity: phi
/// - data_classification: restricted
/// - semantic_type: email
/// - semantic_type: first_name
/// - owner:
/// - provenance: browser_sensor
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, JsonSchema)]
#[serde(transparent)]
#[serde(deny_unknown_fields)]
pub struct Tags {
    /// The tags.
    pub tags: BTreeMap<String, String>,
}
