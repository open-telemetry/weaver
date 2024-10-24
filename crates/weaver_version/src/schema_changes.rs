// SPDX-License-Identifier: Apache-2.0

//! Data structures and utilities for tracking schema changes between versions.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The type of schema item.
#[derive(Debug, Serialize, Hash, Eq, PartialEq, Copy, Clone)]
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
    /// A map where the key is the type of schema item (e.g., "attributes", "metrics",
    /// "events, "spans", "resources"), and the value is a list of changes associated
    /// with that item type.
    changes: HashMap<SchemaItemType, Vec<SchemaItemChange>>,
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

    /// Add a `SchemaChange` to the list of changes for the specified schema item type.
    pub fn add_change(&mut self, item_type: SchemaItemType, change: SchemaItemChange) {
        self.changes
            .get_mut(&item_type)
            .expect("All the possible schema item types should be initialized.")
            .push(change);
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
