//! Lineage tracking for v2 published schemas.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::provenance::PublishedProvenance;

/// Lineage tracking for a v2 published schema.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalLineage {
    /// The provenance of the signal, i.e. where it was originally defined.
    provenance: PublishedProvenance,
}

/// Lineage tracking for attributes in v2 published schema.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributeLineage {
    /// The provenance of the attribute, i.e. where it was originally defined.
    provenance: PublishedProvenance,
}
