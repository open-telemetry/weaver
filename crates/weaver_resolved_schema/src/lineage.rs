// SPDX-License-Identifier: Apache-2.0

//! Data structures used to keep track of the lineage of a semantic convention.

use crate::attribute::AttributeRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Resolution mode.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolutionMode {
    /// Represents the resolution of a reference.
    Reference,
    /// Represents the resolution of an `extends` clause.
    Extends,
}

/// Field id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, Ord, PartialOrd)]
pub enum FieldId {
    /// The group id.
    GroupId,
    /// The group brief.
    GroupBrief,
    /// The group note.
    GroupNote,
    /// The group prefix.
    GroupPrefix,
    /// The group extends.
    GroupExtends,
    /// The group stability.
    GroupStability,
    /// The group deprecated.
    GroupDeprecated,
    /// The group constraints.
    GroupConstraints,
    /// The group attributes.
    GroupAttributes,

    /// The span kind.
    SpanKind,
    /// The span event.
    SpanEvent,

    /// The event name.
    EventName,

    /// The metric name.
    MetricName,
    /// The metric instrument type.
    MetricInstrument,
    /// The metric unit.
    MetricUnit,

    /// The attribute brief.
    AttributeBrief,
    /// The attribute examples.
    AttributeExamples,
    /// The attribute tag.
    AttributeTag,
    /// The attribute requirement level.
    AttributeRequirementLevel,
    /// The attribute sampling relevant.
    AttributeSamplingRelevant,
    /// The attribute note.
    AttributeNote,
    /// The attribute stability.
    AttributeStability,
    /// The attribute deprecated.
    AttributeDeprecated,
    /// The attribute tags.
    AttributeTags,
    /// The attribute value.
    AttributeValue,
}

/// Field lineage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FieldLineage {
    /// The resolution mode used to resolve the field.
    pub resolution_mode: ResolutionMode,
    /// The id of the group where the field is defined.
    pub group_id: String,
}

/// Group lineage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[must_use]
pub struct GroupLineage {
    /// The provenance of the group.
    provenance: String,
    /// The lineage per group field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    fields: BTreeMap<FieldId, FieldLineage>,
    /// The lineage per attribute field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    attributes: BTreeMap<AttributeRef, BTreeMap<FieldId, FieldLineage>>,
}

impl GroupLineage {
    /// Creates a new group lineage.
    pub fn new(provenance: String) -> Self {
        Self {
            provenance,
            fields: BTreeMap::new(),
            attributes: BTreeMap::new(),
        }
    }

    /// Adds a group field lineage.
    pub fn add_group_field_lineage(&mut self, field_id: FieldId, field_lineage: FieldLineage) {
        let prev = self.fields.insert(field_id.clone(), field_lineage.clone());
        if prev.is_some() {
            panic!("Group field `{field_id:?}` lineage already exists (prev: {prev:?}, new: {field_lineage:?}). This is a bug.");
        }
    }

    /// Adds an attribute field lineage.
    pub fn add_attribute_field_lineage(
        &mut self,
        attr_ref: AttributeRef,
        field_id: FieldId,
        field_lineage: FieldLineage,
    ) {
        let attribute_fields = self.attributes.entry(attr_ref).or_default();
        let prev = attribute_fields.insert(field_id.clone(), field_lineage.clone());
        if prev.is_some() {
            panic!("Group attribute `{attr_ref:?}.{field_id:?}` lineage already exists (prev: {prev:?}, new: {field_lineage:?}). This is a bug.");
        }
    }

    /// Returns the provenance of the group.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Returns the lineage of the specified field.
    #[must_use]
    pub fn field_lineage(&self, field_id: &FieldId) -> Option<&FieldLineage> {
        self.fields.get(field_id)
    }
}
