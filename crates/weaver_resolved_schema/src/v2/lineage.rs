// SPDX-License-Identifier: Apache-2.0

//! Data structures used to keep track of the lineage of a V2 semantic convention.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use weaver_semconv::provenance::Provenance;

/// Represents the source or origin of an attribute in V2 terminology.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttributeSource {
    /// The attribute is defined as a free-floating attribute not tied to any group.
    RawAttribute(String),
    /// The attribute comes from an included `attribute_group`.
    AttributeGroup(String),
    /// The attribute is defined/inherited directly within a Span model or refinement.
    Span(String),
    /// The attribute is defined/inherited directly within an Event model or refinement.
    Event(String),
    /// The attribute is defined/inherited directly within a Metric model or refinement.
    Metric(String),
    /// The attribute is defined/inherited directly within an Entity model or refinement.
    Entity(String),
}

/// Attribute lineage adapted for V2.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributeLineage {
    /// The origin of this attribute reference.
    pub source: AttributeSource,

    /// Properties that were inherited.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub inherited_fields: BTreeSet<String>,

    /// Properties that were overridden locally on the attribute reference
    /// (e.g. `requirement_level`, `sampling_relevant`).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub locally_overridden_fields: BTreeSet<String>,
}

impl AttributeLineage {
    /// Creates a new attribute lineage.
    pub fn new(source: AttributeSource) -> Self {
        Self {
            source,
            inherited_fields: Default::default(),
            locally_overridden_fields: Default::default(),
        }
    }
}

/// Lineage for a base signal (Metric, Span, Event, AttributeGroup).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalLineage {
    /// The provenance of the source file where the signal was defined.
    pub provenance: Provenance,

    /// Attribute groups included in this signal via `ref_group`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub included_attribute_groups: Vec<String>,

    /// Lineage per attribute, tracking where the attribute originated.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeLineage>,
}

impl SignalLineage {
    /// Creates a new SignalLineage with the given provenance.
    pub fn new(provenance: Provenance) -> Self {
        Self {
            provenance,
            included_attribute_groups: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    /// Records that an attribute group was included.
    pub fn add_included_group(&mut self, group_id: &str) {
        self.included_attribute_groups.push(group_id.to_owned());
    }

    /// Adds lineage for a specific attribute.
    pub fn add_attribute_lineage(&mut self, id: String, lineage: AttributeLineage) {
        let _ = self.attributes.insert(id, lineage);
    }
}

/// Lineage for a signal refinement (MetricRefinement, SpanRefinement, EventRefinement).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefinementLineage {
    /// The provenance of the source file where the refinement is defined.
    pub provenance: Provenance,

    /// The ID of the signal or refinement this refinement refines.
    pub refines: String,

    /// Fields on the base signal that were explicitly overridden by this refinement.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub locally_overridden_fields: BTreeSet<String>,

    /// Fields on the signal that were inherited from the extended signal.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub inherited_fields: BTreeSet<String>,

    /// Attribute groups included in this refinement via `ref_group`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub included_attribute_groups: Vec<String>,

    /// Lineage per attribute, tracking property overrides.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeLineage>,
}

impl RefinementLineage {
    /// Creates a new RefinementLineage.
    pub fn new(provenance: Provenance, refines: String) -> Self {
        Self {
            provenance,
            refines,
            locally_overridden_fields: BTreeSet::new(),
            inherited_fields: BTreeSet::new(),
            included_attribute_groups: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    /// Records that an attribute group was included.
    pub fn add_included_group(&mut self, group_id: &str) {
        self.included_attribute_groups.push(group_id.to_owned());
    }

    /// Adds lineage for a specific attribute.
    pub fn add_attribute_lineage(&mut self, id: String, lineage: AttributeLineage) {
        let _ = self.attributes.insert(id, lineage);
    }
}

impl AttributeLineage {
    /// Converts from a V1 AttributeLineage to a V2 AttributeLineage.
    pub(crate) fn from_v1(
        v1: crate::lineage::AttributeLineage,
        group_type_lookup: &std::collections::HashMap<String, weaver_semconv::group::GroupType>,
    ) -> Self {
        let source = match group_type_lookup.get(&v1.source_group) {
            Some(weaver_semconv::group::GroupType::AttributeGroup) => {
                AttributeSource::AttributeGroup(v1.source_group)
            }
            Some(weaver_semconv::group::GroupType::Span) => AttributeSource::Span(v1.source_group),
            Some(weaver_semconv::group::GroupType::Event) => {
                AttributeSource::Event(v1.source_group)
            }
            Some(weaver_semconv::group::GroupType::Metric) => {
                AttributeSource::Metric(v1.source_group)
            }
            Some(weaver_semconv::group::GroupType::Entity) => {
                AttributeSource::Entity(v1.source_group)
            }
            _ => AttributeSource::RawAttribute(v1.source_group),
        };

        Self {
            source,
            inherited_fields: v1.inherited_fields,
            locally_overridden_fields: v1.locally_overridden_fields,
        }
    }
}

impl SignalLineage {
    /// Converts from a V1 GroupLineage to a V2 SignalLineage.
    pub(crate) fn from_v1(
        v1: crate::lineage::GroupLineage,
        group_type_lookup: &std::collections::HashMap<String, weaver_semconv::group::GroupType>,
    ) -> Self {
        let mut attributes = BTreeMap::new();
        for (id, attr) in v1.attributes() {
            _ = attributes.insert(
                id.clone(),
                AttributeLineage::from_v1(attr.clone(), group_type_lookup),
            );
        }

        Self {
            provenance: v1.provenance().clone(),
            included_attribute_groups: v1.includes_group,
            attributes,
        }
    }
}

impl RefinementLineage {
    /// Converts from a V1 GroupLineage to a V2 RefinementLineage.
    pub(crate) fn from_v1(
        group_id: &str,
        v1: crate::lineage::GroupLineage,
        group_type_lookup: &std::collections::HashMap<String, weaver_semconv::group::GroupType>,
    ) -> Result<Self, crate::error::Error> {
        let mut attributes = BTreeMap::new();
        for (id, attr) in v1.attributes() {
            _ = attributes.insert(
                id.clone(),
                AttributeLineage::from_v1(attr.clone(), group_type_lookup),
            );
        }

        let refines = v1
            .v2_refines
            .clone()
            .or(v1.extends_group.clone())
            .ok_or_else(|| crate::error::Error::RefinementLineageBroken {
                group_id: group_id.to_owned(),
            })?;

        Ok(Self {
            provenance: v1.provenance().clone(),
            refines,
            locally_overridden_fields: v1.v2_locally_overridden_fields,
            inherited_fields: v1.v2_inherited_fields,
            included_attribute_groups: v1.includes_group,
            attributes,
        })
    }
}
