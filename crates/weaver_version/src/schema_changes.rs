// SPDX-License-Identifier: Apache-2.0

//! Data structures and utilities for tracking schema changes between versions.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The type of schema item.
#[derive(Debug, Serialize, Hash, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SchemaItemType {
    /// Attributes
    Attributes,
    /// Metrics
    Metrics,
    /// Events
    Events,
    /// Spans
    Spans,
    /// Resources
    Resources,
}

/// A summary of schema changes between two versions of a schema.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SchemaChanges {
    /// Information on the registry manifest for the most recent version of the schema.
    head: RegistryManifest,

    /// Information of the registry manifest for the baseline version of the schema.
    baseline: RegistryManifest,

    /// A map where the key is the type of schema item (e.g., "attributes", "metrics",
    /// "events, "spans", "resources"), and the value is a list of changes associated
    /// with that item type.
    changes: HashMap<SchemaItemType, Vec<SchemaItemChange>>,
}

/// Represents the information of a semantic convention registry manifest.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistryManifest {
    /// The version of the registry which will be used to define the semconv package version.
    pub semconv_version: String,
}

/// Represents the different types of changes that can occur between
/// two versions of a schema. This covers changes such as adding, removing,
/// renaming, and deprecating schema items (attributes, metrics, etc.).
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum SchemaItemChange {
    /// An item (e.g. attribute, metric, ...) has been added
    /// into the most recent version of the schema.
    Added {
        /// The name of the added item.
        name: String,
    },
    /// One or more items have been renamed into a new item.
    RenamedToNew {
        /// The old names of the items that have been renamed.
        old_names: HashSet<String>,
        /// The new name of the items that have been renamed.
        new_name: String,
    },
    /// One or more items have been renamed into an existing item.
    RenamedToExisting {
        /// The old names of the items that have been renamed.
        old_names: HashSet<String>,
        /// The current name of the items that have been renamed.
        current_name: String,
    },
    /// An item has been deprecated.
    Deprecated {
        /// The name of the deprecated item.
        name: String,
        /// A deprecation note providing further context.
        note: String,
    },
    /// An item has been removed.
    Removed {
        /// The name of the removed item.
        name: String,
    },
}

impl SchemaChanges {
    /// Create a new instance of `SchemaChanges`.
    #[must_use]
    pub fn new() -> Self {
        let mut schema_changes = SchemaChanges {
            head: RegistryManifest::default(),
            baseline: RegistryManifest::default(),
            changes: HashMap::new(),
        };
        let _ = schema_changes
            .changes
            .insert(SchemaItemType::Attributes, Vec::new());
        let _ = schema_changes
            .changes
            .insert(SchemaItemType::Metrics, Vec::new());
        let _ = schema_changes
            .changes
            .insert(SchemaItemType::Events, Vec::new());
        let _ = schema_changes
            .changes
            .insert(SchemaItemType::Spans, Vec::new());
        let _ = schema_changes
            .changes
            .insert(SchemaItemType::Resources, Vec::new());

        schema_changes
    }

    /// Returns true if there are no changes in the schema.
    /// Otherwise, it returns false.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.values().all(|v| v.is_empty())
    }

    /// Counts the number of changes in the schema.
    #[must_use]
    pub fn count_changes(&self) -> usize {
        self.changes.values().map(|v| v.len()).sum()
    }

    /// Counts the number of attribute changes in the schema.
    #[must_use]
    pub fn count_attribute_changes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Counts the number of added attributes in the schema.
    #[must_use]
    pub fn count_added_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Added { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of deprecated attributes in the schema.
    #[must_use]
    pub fn count_deprecated_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Deprecated { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of renamed to new attributes in the schema.
    #[must_use]
    pub fn count_renamed_to_new_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::RenamedToNew { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of renamed to existing attributes in the schema.
    #[must_use]
    pub fn count_renamed_to_existing_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::RenamedToExisting { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of removed attributes in the schema.
    #[must_use]
    pub fn count_removed_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Removed { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Returns all the renamed to existing attributes changes.
    #[must_use]
    pub fn renamed_to_existing_attributes(&self) -> Vec<&SchemaItemChange> {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::RenamedToExisting { .. }))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns all the renamed to new attributes changes.
    #[must_use]
    pub fn renamed_to_new_attributes(&self) -> Vec<&SchemaItemChange> {
        self.changes
            .get(&SchemaItemType::Attributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::RenamedToNew { .. }))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Add a `SchemaChange` to the list of changes for the specified schema item type.
    pub fn add_change(&mut self, item_type: SchemaItemType, change: SchemaItemChange) {
        self.changes
            .get_mut(&item_type)
            .expect("All the possible schema item types should be initialized.")
            .push(change);
    }

    /// Set the baseline manifest for the schema changes.
    pub fn set_head_manifest(&mut self, head: RegistryManifest) {
        self.head = head;
    }

    /// Set the baseline manifest for the schema changes.
    pub fn set_baseline_manifest(&mut self, baseline: RegistryManifest) {
        self.baseline = baseline;
    }

    /// Return a string representation of the statistics on the schema changes.
    #[must_use]
    pub fn dump_stats(&self) -> String {
        fn print_changes(
            changes: Option<&Vec<SchemaItemChange>>,
            item_type: &str,
            result: &mut String,
        ) {
            if let Some(changes) = changes {
                result.push_str(&format!("{}:\n", item_type));
                result.push_str(&format!(
                    "  Added: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::Added { .. }))
                        .count()
                ));
                result.push_str(&format!(
                    "  Renamed to new: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::RenamedToNew { .. }))
                        .count()
                ));
                result.push_str(&format!(
                    "  Renamed to existing: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::RenamedToExisting { .. }))
                        .count()
                ));
                result.push_str(&format!(
                    "  Deprecated: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::Deprecated { .. }))
                        .count()
                ));
                result.push_str(&format!(
                    "  Removed: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::Removed { .. }))
                        .count()
                ));
            }
        }

        let mut result = String::new();

        result.push_str("Schema Changes:\n");

        print_changes(
            self.changes.get(&SchemaItemType::Attributes),
            "Attributes",
            &mut result,
        );
        print_changes(
            self.changes.get(&SchemaItemType::Metrics),
            "Metrics",
            &mut result,
        );
        print_changes(
            self.changes.get(&SchemaItemType::Events),
            "Events",
            &mut result,
        );
        print_changes(
            self.changes.get(&SchemaItemType::Spans),
            "Spans",
            &mut result,
        );
        print_changes(
            self.changes.get(&SchemaItemType::Resources),
            "Resources",
            &mut result,
        );

        result
    }
}
