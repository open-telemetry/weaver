// SPDX-License-Identifier: Apache-2.0

//! Defines the catalog of attributes, metrics, and other telemetry items
//! that are shared across multiple signals in the Resolved Telemetry Schema.

use crate::v1::attribute::{Attribute, AttributeRef};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use weaver_semconv::stability::Stability;
use weaver_semconv::v1::attribute::{AttributeType, BasicRequirementLevelSpec, RequirementLevel};

/// A catalog of indexed attributes shared across semconv groups, or signals.
/// Attribute references are used to refer to attributes in the catalog.
///
/// Note : In the future, this catalog could be extended with other entities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct Catalog {
    /// Catalog of attribute definitions and refinements used in the schema.
    /// Contains elements with the same attribute key.
    /// Use root_attributes for original attribute definitions.
    attributes: Vec<Attribute>,
    /// Attribute definitions available in this registry (including those
    /// from dependencies). Used for cross-registry attribute lookup.
    /// Not serialized — populated only for freshly resolved schemas.
    root_attributes: HashMap<String, RootAttribute>,
}

/// What is known about an attribute key beyond the individual, possibly
/// refined, copies held in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootAttribute {
    /// The attribute.
    pub attribute: Attribute,
    /// The group, or dependency, the attribute came from.
    pub source_group: String,
    /// Whether this is a definition a reference may resolve against. Attributes
    /// inherited through a refinement are recorded here for their provenance,
    /// but they are instances of a definition owned elsewhere, not definitions.
    pub is_definition: bool,
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
    /// Creates a catalog from a list of attributes and root attribute definitions.
    pub fn new(
        attributes: Vec<Attribute>,
        root_attributes: HashMap<String, RootAttribute>,
    ) -> Self {
        Self {
            attributes,
            root_attributes,
        }
    }

    /// Looks up an attribute by name, whether or not this registry defines it.
    /// Use this to answer "where did this attribute come from"; use
    /// [`Catalog::root_attribute_definition`] to resolve a reference.
    #[must_use]
    pub fn root_attribute(&self, name: &str) -> Option<(&Attribute, &str)> {
        self.root_attributes
            .get(name)
            .map(|root| (&root.attribute, root.source_group.as_str()))
    }

    /// Looks up the definition of an attribute, ignoring attributes that merely
    /// passed through this registry as part of a refinement.
    #[must_use]
    pub fn root_attribute_definition(&self, name: &str) -> Option<(&Attribute, &str)> {
        self.root_attributes
            .get(name)
            .filter(|root| root.is_definition)
            .map(|root| (&root.attribute, root.source_group.as_str()))
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

#[cfg(test)]
/// Test utilities for building [`Catalog`] instances.
pub mod test_utils {
    use super::*;

    /// A builder for constructing a [`Catalog`] in tests.
    #[derive(Default)]
    pub struct CatalogBuilder {
        attributes: Vec<Attribute>,
        root_attributes: HashMap<String, RootAttribute>,
    }

    impl CatalogBuilder {
        /// Creates a builder pre-populated with all attributes from an existing catalog.
        /// Root attributes are not copied — use [`CatalogBuilder::add`] with a `group_id` for that.
        #[must_use]
        pub fn from_catalog(catalog: &Catalog) -> Self {
            let mut builder = Self::default();
            for attr in catalog.attributes() {
                let _ = builder.add(attr.clone(), None);
            }
            builder
        }

        /// Adds an attribute to the catalog. If `group_id` is `Some`, the attribute
        /// is also registered as a root definition for cross-registry lookup.
        pub fn add(&mut self, attr: Attribute, group_id: Option<&str>) -> AttributeRef {
            if let Some(gid) = group_id {
                let _ = self.root_attributes.insert(
                    attr.name.clone(),
                    RootAttribute {
                        attribute: attr.clone(),
                        source_group: gid.to_owned(),
                        is_definition: true,
                    },
                );
            }
            let idx = self.attributes.len();
            self.attributes.push(attr);
            AttributeRef(idx as u32)
        }

        /// Builds the [`Catalog`].
        pub fn build(self) -> Catalog {
            Catalog::new(self.attributes, self.root_attributes)
        }
    }
}
