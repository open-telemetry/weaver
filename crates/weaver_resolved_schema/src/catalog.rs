// SPDX-License-Identifier: Apache-2.0

//! Defines the catalog of attributes, metrics, and other telemetry items
//! that are shared across multiple signals in the Resolved Telemetry Schema.

use crate::attribute::{Attribute, AttributeRef};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use weaver_semconv::attribute::{AttributeType, BasicRequirementLevelSpec, RequirementLevel};
use weaver_semconv::stability::Stability;

/// A catalog of indexed attributes shared across semconv groups, or signals.
/// Attribute references are used to refer to attributes in the catalog.
///
/// Note : In the future, this catalog could be extended with other entities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct Catalog {
    /// Catalog of attributes used in the schema.
    attributes: Vec<Attribute>,
    /// Attribute definitions available in this registry (including those
    /// from dependencies). Used for cross-registry attribute lookup.
    /// Not serialized — populated only for freshly resolved schemas.
    root_attributes: HashMap<String, (Attribute, String)>,
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
    #[cfg(test)]
    pub fn from_attributes(attributes: Vec<Attribute>) -> Self {
        Self {
            attributes,
            root_attributes: HashMap::new(),
        }
    }

    /// Creates a catalog from a list of attributes and root attribute definitions.
    pub fn from_attributes_and_root(
        attributes: Vec<Attribute>,
        root_attributes: HashMap<String, (Attribute, String)>,
    ) -> Self {
        Self {
            attributes,
            root_attributes,
        }
    }

    /// Looks up an attribute by name in the root attribute definitions.
    #[must_use]
    pub fn root_attribute(&self, name: &str) -> Option<(&Attribute, &str)> {
        self.root_attributes
            .get(name)
            .map(|(attr, group_id)| (attr, group_id.as_str()))
    }

    /// Counts the number of attributes in the catalog.
    #[must_use]
    pub fn count_attributes(&self) -> usize {
        self.attributes.len()
    }

    /// Return an iterator over the attributes in the catalog.
    pub fn attributes(&self) -> impl Iterator<Item = &Attribute> {
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
