// SPDX-License-Identifier: Apache-2.0

//! Data structures and utilities for tracking schema changes between versions.

use serde::Serialize;
use std::collections::HashMap;

/// The type of schema item.
#[derive(Debug, Serialize, Hash, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SchemaItemType {
    /// Registry attributes
    RegistryAttributes,
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
/// renaming, and deprecating telemetry objects (attributes, metrics, etc.).
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum SchemaItemChange {
    /// A top-level telemetry object (e.g., attribute, metric, etc.) was added to the head registry.
    Added {
        /// The name of the added telemetry object.
        name: String,
    },
    /// A top-level telemetry object from the baseline registry was renamed in the head registry.
    Renamed {
        /// The old name of the telemetry object that has been renamed.
        old_name: String,
        /// The new name of the telemetry object that has been renamed.
        new_name: String,
        /// A note providing further context.
        note: String,
    },
    /// A top-level telemetry object from the baseline registry was marked as deprecated in the head
    /// registry.
    Deprecated {
        /// The name of the deprecated telemetry object.
        name: String,
        /// A deprecation note providing further context.
        note: String,
    },
    /// A top-level telemetry object from the baseline registry was removed in the head registry.
    Removed {
        /// The name of the removed telemetry object.
        name: String,
    },
    /// A placeholder for complex or unclear schema changes that do not fit into existing types.
    /// This type serves as a fallback when no specific category applies, with the expectation that
    /// some of these changes will be reclassified into more precise schema types in the future.
    Uncategorized {
        /// A note providing further context.
        note: String,
    },
    /// One or more fields in a top-level telemetry object have been updated in the head registry.
    /// Note: This is a placeholder for future use.
    Updated {},
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
            .insert(SchemaItemType::RegistryAttributes, Vec::new());
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

    /// Counts the number of registry attribute changes in the schema.
    #[must_use]
    pub fn count_registry_attribute_changes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Counts the number of added registry attributes in the schema.
    #[must_use]
    pub fn count_added_registry_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Added { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of deprecated registry attributes in the schema.
    #[must_use]
    pub fn count_deprecated_registry_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Deprecated { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of renamed registry attributes in the schema.
    #[must_use]
    pub fn count_renamed_registry_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Renamed { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Counts the number of removed registry attributes in the schema.
    #[must_use]
    pub fn count_removed_registry_attributes(&self) -> usize {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Removed { .. }))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Returns all the renamed registry attributes changes.
    #[must_use]
    pub fn renamed_registry_attributes(&self) -> Vec<&SchemaItemChange> {
        self.changes
            .get(&SchemaItemType::RegistryAttributes)
            .map(|v| {
                v.iter()
                    .filter(|c| matches!(c, SchemaItemChange::Renamed { .. }))
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
                        .filter(|c| matches!(c, SchemaItemChange::Renamed { .. }))
                        .count()
                ));
                result.push_str(&format!(
                    "  Renamed to existing: {}\n",
                    changes
                        .iter()
                        .filter(|c| matches!(c, SchemaItemChange::Renamed { .. }))
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
            self.changes.get(&SchemaItemType::RegistryAttributes),
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
