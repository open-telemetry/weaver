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

/// A summary of changes to the registry of signals and attributes.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct RegistryChanges {
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
