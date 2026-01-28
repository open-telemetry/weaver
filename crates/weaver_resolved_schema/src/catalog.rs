// SPDX-License-Identifier: Apache-2.0

//! Defines the catalog of attributes, metrics, and other telemetry items
//! that are shared across multiple signals in the Resolved Telemetry Schema.

use crate::attribute::{Attribute, AttributeRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use weaver_semconv::attribute::{AttributeType, BasicRequirementLevelSpec, RequirementLevel};
use weaver_semconv::stability::Stability;

/// A catalog of indexed attributes shared across semconv groups, or signals.
/// Attribute references are used to refer to attributes in the catalog.
///
/// Note : In the future, this catalog could be extended with other entities.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[must_use]
pub struct Catalog {
    /// Catalog of attributes used in the schema.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attributes: Vec<Attribute>,
}

/// Statistics on a catalog.
#[derive(Debug, Serialize)]
#[must_use]
pub struct Stats {
    /// Total number of attributes.
    pub attribute_count: usize,
    /// Breakdown of attribute types.
    pub attribute_type_breakdown: BTreeMap<String, usize>,
    /// Breakdown of requirement levels.
    pub requirement_level_breakdown: BTreeMap<String, usize>,
    /// Breakdown of stability levels.
    pub stability_breakdown: HashMap<Stability, usize>,
    /// Number of deprecated attributes.
    pub deprecated_count: usize,
}

impl Catalog {
    /// Creates a catalog from a list of attributes.
    pub fn from_attributes(attributes: Vec<Attribute>) -> Self {
        Self { attributes }
    }

    /// Adds attributes to the catalog and returns a list of attribute references.
    #[must_use]
    pub fn add_attributes<const N: usize>(
        &mut self,
        attributes: [Attribute; N],
    ) -> Vec<AttributeRef> {
        let start_index = self.attributes.len();
        self.attributes.extend(attributes.iter().cloned());
        (start_index..self.attributes.len())
            .map(|i| AttributeRef(i as u32))
            .collect::<Vec<_>>()
    }

    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    #[must_use]
    pub fn attribute_name(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.name.as_ref())
    }

    /// Counts the number of attributes in the catalog.
    #[must_use]
    pub fn count_attributes(&self) -> usize {
        self.attributes.len()
    }

    /// Return an iterator over the attributes in the catalog.
    pub fn iter(&self) -> impl Iterator<Item = &Attribute> {
        self.attributes.iter()
    }

    /// Returns the attribute from an attribute ref if it exists.
    #[must_use]
    pub fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.attributes.get(attribute_ref.0 as usize)
    }

    /// Statistics on the catalog.
    pub fn stats(&self) -> Stats {
        Stats {
            attribute_count: self.attributes.len(),
            attribute_type_breakdown: self
                .attributes
                .iter()
                .map(|attr| {
                    if let AttributeType::Enum { members, .. } = &attr.r#type {
                        (format!("enum(card:{:03})", members.len()), 1)
                    } else {
                        (format!("{:#}", attr.r#type), 1)
                    }
                })
                .fold(BTreeMap::new(), |mut acc, (k, v)| {
                    *acc.entry(k).or_insert(0) += v;
                    acc
                }),
            requirement_level_breakdown: self
                .attributes
                .iter()
                .map(|attr| {
                    let requirement_level = match &attr.requirement_level {
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => "required",
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => {
                            "recommended"
                        }
                        RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => "opt_in",
                        RequirementLevel::Recommended { .. } => "recommended",
                        RequirementLevel::ConditionallyRequired { .. } => "conditionally_required",
                        RequirementLevel::OptIn { .. } => "opt_in",
                    };
                    (requirement_level.to_owned(), 1)
                })
                .fold(BTreeMap::new(), |mut acc, (k, v)| {
                    *acc.entry(k).or_insert(0) += v;
                    acc
                }),
            stability_breakdown: self
                .attributes
                .iter()
                .filter_map(|attr| attr.stability.as_ref())
                .map(|stability| (stability.clone(), 1))
                .fold(HashMap::new(), |mut acc, (k, v)| {
                    *acc.entry(k).or_insert(0) += v;
                    acc
                }),
            deprecated_count: self
                .attributes
                .iter()
                .filter(|attr| attr.deprecated.is_some())
                .count(),
        }
    }
}
