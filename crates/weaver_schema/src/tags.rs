// SPDX-License-Identifier: Apache-2.0

//! Tags for telemetry schemas.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
#[serde(deny_unknown_fields)]
#[must_use]
pub struct Tags {
    /// The tags.
    pub tags: BTreeMap<String, String>,
}

impl Tags {
    /// Checks if the tags contain a specific tag.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains_key(tag)
    }

    /// Gets a specific tag value from the tags if it exists or `None` otherwise.
    #[must_use]
    pub fn get_tag(&self, tag: &str) -> Option<&String> {
        self.tags.get(tag)
    }

    /// Gets an iterator over the tags.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.tags.iter()
    }

    /// Checks if the tags are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Merges the tags with another set of tags. If a tag exists in both sets of tags, the tag
    /// from the current set of tags is used (i.e. self).
    pub fn merge_with_override(&self, other: &Tags) -> Tags {
        let mut tags = other.tags.clone();
        for (key, value) in self.tags.iter() {
            _ = tags.insert(key.clone(), value.clone());
        }
        Tags { tags }
    }
}

/// Merges two sets of tags. If a tag exists in both sets of tags, the tag from `tags`
/// is used to override the tag from `parent_tags`.
#[must_use]
pub fn merge_with_override(tags: Option<&Tags>, parent_tags: Option<&Tags>) -> Option<Tags> {
    match (tags, parent_tags) {
        (Some(tags), Some(parent_tags)) => Some(tags.merge_with_override(parent_tags)),
        (Some(tags), None) => Some(tags.clone()),
        (None, Some(parent_tags)) => Some(parent_tags.clone()),
        (None, None) => None,
    }
}
