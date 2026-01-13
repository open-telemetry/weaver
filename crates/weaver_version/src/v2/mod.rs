//! V2 diffs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// V2 Leverages the same nomenclature for diff as V1.
pub use crate::schema_changes::SchemaItemChange;

/// A summary of schema changes between two versions of a schema.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SchemaChanges {
    /// Changes to the registry.
    pub registry: RegistryChanges,
}

/// The file format version for v2 diff schema.
pub const DIFF_FILE_FORMAT: &str = "2.0.0/diff";

fn diff_file_format() -> String {
    DIFF_FILE_FORMAT.to_string()
}

/// A summary of changes to the registry of signals and attributes.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct RegistryChanges {
    /// The file format version of the diff schema.
    #[serde(default = "diff_file_format")]
    pub file_format: String,
    /// The schema URL that the current (head) schema is published at.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub head_schema_url: String,
    /// The schema URL that the baseline schema is published at.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub baseline_schema_url: String,
    /// Changes across the registry of attributes.
    pub attribute_changes: Vec<SchemaItemChange>,
    /// Changes across the registry of attribute groups.
    pub attribute_group_changes: Vec<SchemaItemChange>,
    /// Changes across the registry of entities.
    pub entity_changes: Vec<SchemaItemChange>,
    /// Changes across the registry of events.
    pub event_changes: Vec<SchemaItemChange>,
    /// Changes across the registry of metrics.
    pub metric_changes: Vec<SchemaItemChange>,
    /// Changes across the registry of spans.
    pub span_changes: Vec<SchemaItemChange>,
}
impl RegistryChanges {
    /// Returns true if there are no changes in the schema.
    /// Otherwise, it returns false.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attribute_changes.is_empty()
            && self.attribute_group_changes.is_empty()
            && self.entity_changes.is_empty()
            && self.event_changes.is_empty()
            && self.metric_changes.is_empty()
            && self.span_changes.is_empty()
    }
}

impl SchemaChanges {
    /// Returns true if there are no changes in the schema.
    /// Otherwise, it returns false.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }
}
