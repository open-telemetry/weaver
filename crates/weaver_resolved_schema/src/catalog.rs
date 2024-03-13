// SPDX-License-Identifier: Apache-2.0

//! Defines the catalog of attributes, metrics, and other telemetry items
//! that are shared across multiple signals in the Resolved Telemetry Schema.

use std::collections::{BTreeMap, HashMap};
use crate::attribute::{Attribute, AttributeRef};
use crate::metric::Metric;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use weaver_semconv::attribute::{AttributeType, BasicRequirementLevelSpec, RequirementLevel};
use weaver_semconv::stability::Stability;

/// A catalog of attributes, metrics, and other telemetry signals that are shared
/// in the Resolved Telemetry Schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// Catalog of attributes used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Catalog of metrics used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
}

/// Statistics on a catalog.
#[derive(Debug, Serialize)]
pub struct Stats {
    /// Total number of attributes.
    pub attribute_count: usize,
    /// Breakdown of attribute types.
    pub attribute_type_breakdown: BTreeMap<String, usize>,
    /// Total number of metrics.
    pub metric_count: usize,
    /// Breakdown of requirement levels.
    pub requirement_level_breakdown: BTreeMap<String, usize>,
    /// Breakdown of stability levels.
    pub stability_breakdown: HashMap<Stability, usize>,
    /// Number of deprecated attributes.
    pub deprecated_count: usize,
}

impl Catalog {
    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    pub fn attribute_name(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.name.as_ref())
    }

    /// Returns the attribute from an attribute ref if it exists.
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
                    if let AttributeType::Enum {members, ..} = &attr.r#type {
                        (format!("enum(card:{:03})",members.len()), 1)
                    } else {
                        (format!("{:#}",attr.r#type), 1)
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
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => "recommended",
                        RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => "opt_in",
                        RequirementLevel::Recommended {..} => "recommended",
                        RequirementLevel::ConditionallyRequired {..} => "conditionally_required",
                    };
                    (requirement_level.to_string(), 1)
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
            metric_count: self.metrics.len(),
        }
    }
}
