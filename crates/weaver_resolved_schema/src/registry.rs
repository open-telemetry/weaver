// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! A semantic convention registry.

use schemars::JsonSchema;
use std::collections::{BTreeMap, HashMap, HashSet};
use weaver_semconv::any_value::AnyValueSpec;

use crate::attribute::{Attribute, AttributeRef};
use crate::catalog::Catalog;
use crate::error::{handle_errors, Error};
use crate::lineage::GroupLineage;
use crate::registry::GroupStats::{
    AttributeGroup, Event, Metric, MetricGroup, Resource, Scope, Span,
};
use serde::{Deserialize, Serialize};
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::group::{GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::provenance::Provenance;
use weaver_semconv::stability::Stability;
use weaver_semconv::YamlValue;

/// A semantic convention registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    /// A list of semantic convention groups.
    pub groups: Vec<Group>,
}

/// Statistics on a registry.
#[derive(Debug, Serialize)]
#[must_use]
pub struct Stats {
    /// Url of the registry.
    pub url: String,
    /// Total number of groups.
    pub group_count: usize,
    /// Breakdown of group statistics by type.
    pub group_breakdown: HashMap<GroupType, GroupStats>,
}

/// Group specification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct Group {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the group including the specific fields for each type.
    pub r#type: GroupType,
    /// A brief description of the semantic convention.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// A more elaborate description of the semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Prefix for the attributes for this semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// Reference another semantic convention id. It inherits the prefix,
    /// constraints, and all attributes defined in the specified semantic
    /// convention.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<AttributeRef>,

    /// Specifies the kind of the span.
    /// Note: only valid if type is span (the default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span (the default)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<InstrumentSpec>,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The name of the event. If not specified, the prefix is used.
    /// If prefix is empty (or unspecified), name is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The lineage of the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<GroupLineage>,
    /// The readable name for attribute groups used when generating registry tables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The body of the event.
    /// This fields is only used for event groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<AnyValueSpec>,
    /// Annotations for the group.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, YamlValue>>,
}

/// Common statistics for a group.
#[derive(Debug, Serialize, Default)]
pub struct CommonGroupStats {
    /// Number of instances in this type of group.
    pub count: usize,
    /// Total number of attributes.
    pub total_attribute_count: usize,
    /// Total number of groups with a prefix.
    pub total_with_prefix: usize,
    /// Total number of groups with a note.
    pub total_with_note: usize,
    /// Stability breakdown.
    pub stability_breakdown: HashMap<Stability, usize>,
    /// Number of deprecated groups.
    pub deprecated_count: usize,
    /// Attribute cardinality breakdown.
    pub attribute_card_breakdown: BTreeMap<usize, usize>,
}

/// Statistics on a group.
#[derive(Debug, Serialize)]
pub enum GroupStats {
    /// Statistics for an attribute group.
    AttributeGroup {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
    },
    /// Statistics for a metric.
    Metric {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
        /// Metric names.
        metric_names: HashSet<String>,
        /// Instrument breakdown.
        instrument_breakdown: HashMap<InstrumentSpec, usize>,
        /// Unit breakdown.
        unit_breakdown: HashMap<String, usize>,
    },
    /// Statistics for a metric group.
    MetricGroup {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
    },
    /// Statistics for an event.
    Event {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
    },
    /// Statistics for a resource.
    Resource {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
    },
    /// Statistics for a scope.
    Scope {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
    },
    /// Statistics for a span.
    Span {
        /// Common statistics for this type of group.
        common_stats: CommonGroupStats,
        /// Span kind breakdown.
        span_kind_breakdown: HashMap<SpanKindSpec, usize>,
    },
}

/// Allow to define additional requirements on the semantic convention.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    /// any_of accepts a list of sequences. Each sequence contains a list of
    /// attribute ids that are required. any_of enforces that all attributes
    /// of at least one of the sequences are set.
    #[serde(default)]
    pub any_of: Vec<String>,
    /// include accepts a semantic conventions id. It includes as part of this
    /// semantic convention all constraints and required attributes that are
    /// not already defined in the current semantic convention.
    pub include: Option<String>,
}

impl CommonGroupStats {
    /// Update the statistics with the provided group.
    pub fn update_stats(&mut self, group: &Group) {
        self.count += 1;
        self.total_attribute_count += group.attributes.len();
        self.total_with_prefix += !group.prefix.is_empty() as usize;
        self.total_with_note += !group.note.is_empty() as usize;
        if let Some(stability) = group.stability.as_ref() {
            *self
                .stability_breakdown
                .entry(stability.clone())
                .or_insert(0) += 1;
        }
        self.deprecated_count += group.deprecated.is_some() as usize;
        *self
            .attribute_card_breakdown
            .entry(group.attributes.len())
            .or_insert(0) += 1;
    }
}

impl Registry {
    /// Creates a new registry.
    #[must_use]
    pub fn new<S: AsRef<str>>(registry_url: S) -> Self {
        Self {
            registry_url: registry_url.as_ref().to_owned(),
            groups: Vec::new(),
        }
    }

    /// Returns the groups of the specified type.
    ///
    /// # Arguments
    ///
    /// * `group_type` - The type of the groups to return.
    pub fn groups(&self, group_type: GroupType) -> impl Iterator<Item = &Group> {
        self.groups
            .iter()
            .filter(move |group| group_type == group.r#type)
    }

    /// Statistics on a registry.
    pub fn stats(&self) -> Stats {
        Stats {
            url: self.registry_url.clone(),
            group_count: self.groups.len(),
            group_breakdown: self.groups.iter().fold(HashMap::new(), |mut acc, group| {
                let group_type = group.r#type.clone();

                _ =
                    acc.entry(group_type)
                        .and_modify(|stats| match stats {
                            AttributeGroup { common_stats } => {
                                common_stats.update_stats(group);
                            }
                            Metric {
                                common_stats,
                                metric_names,
                                instrument_breakdown,
                                unit_breakdown,
                            } => {
                                common_stats.update_stats(group);
                                _ =
                                    metric_names.insert(group.metric_name.clone().expect(
                                        "metric_name is required as we are in a metric group",
                                    ));
                                *instrument_breakdown
                                    .entry(group.instrument.clone().expect(
                                        "instrument is required as we are in a metric group",
                                    ))
                                    .or_insert(0) += 1;
                                *unit_breakdown
                                    .entry(
                                        group
                                            .unit
                                            .clone()
                                            .expect("unit is required as we are in a metric group"),
                                    )
                                    .or_insert(0) += 1;
                            }
                            MetricGroup { common_stats } => {
                                common_stats.update_stats(group);
                            }
                            Event { common_stats } => {
                                common_stats.update_stats(group);
                            }
                            Resource { common_stats } => {
                                common_stats.update_stats(group);
                            }
                            Scope { common_stats } => {
                                common_stats.update_stats(group);
                            }
                            Span {
                                common_stats,
                                span_kind_breakdown,
                            } => {
                                common_stats.update_stats(group);
                                if let Some(span_kind) = group.span_kind.clone() {
                                    *span_kind_breakdown.entry(span_kind).or_insert(0) += 1;
                                }
                            }
                        })
                        .or_insert_with(|| match group.r#type {
                            GroupType::AttributeGroup => AttributeGroup {
                                common_stats: CommonGroupStats::default(),
                            },
                            GroupType::Metric => Metric {
                                common_stats: CommonGroupStats::default(),
                                metric_names: HashSet::new(),
                                instrument_breakdown: HashMap::new(),
                                unit_breakdown: HashMap::new(),
                            },
                            GroupType::MetricGroup => MetricGroup {
                                common_stats: CommonGroupStats::default(),
                            },
                            GroupType::Event => Event {
                                common_stats: CommonGroupStats::default(),
                            },
                            GroupType::Resource => Resource {
                                common_stats: CommonGroupStats::default(),
                            },
                            GroupType::Scope => Scope {
                                common_stats: CommonGroupStats::default(),
                            },
                            GroupType::Span => Span {
                                common_stats: CommonGroupStats::default(),
                                span_kind_breakdown: HashMap::new(),
                            },
                        });
                acc
            }),
        }
    }
}

impl Group {
    /// Returns `true` if the group is a registry attribute group.
    ///
    /// Note: Currently, this method relies on the `registry.` prefix to identify  
    /// registry attribute groups. Once issue [#580](https://github.com/open-telemetry/weaver/issues/580)  
    /// is resolved, this method must be updated accordingly.
    #[must_use]
    pub fn is_registry_attribute_group(&self) -> bool {
        matches!(self.r#type, GroupType::AttributeGroup) && self.id.starts_with("registry.")
    }

    /// Returns the fully resolved attributes of the group.
    /// The attribute references are resolved via the provided catalog.
    /// If an attribute reference is not found in the catalog, an error is
    /// returned. The errors are collected and returned as a compound error.
    ///
    /// # Arguments
    ///
    /// * `catalog` - The catalog to resolve the attribute references.
    ///
    /// # Returns
    ///
    /// The fully resolved attributes of the group.
    pub fn attributes<'a>(&'a self, catalog: &'a Catalog) -> Result<Vec<&'a Attribute>, Error> {
        let mut errors = Vec::new();
        let attributes = self
            .attributes
            .iter()
            .filter_map(|attr_ref| {
                if let Some(attr) = catalog.attribute(attr_ref) {
                    Some(attr)
                } else {
                    errors.push(Error::AttributeNotFound {
                        group_id: self.id.clone(),
                        attr_ref: *attr_ref,
                    });
                    None
                }
            })
            .collect();

        handle_errors(errors)?;
        Ok(attributes)
    }

    /// Returns true if the group contains at least one `include` constraint.
    #[must_use]
    pub fn has_include(&self) -> bool {
        self.constraints.iter().any(|c| c.include.is_some())
    }

    /// Import attributes from the provided slice that do not exist in the
    /// current group.
    pub fn import_attributes_from(&mut self, attributes: &[AttributeRef]) {
        for attr in attributes {
            if !self.attributes.contains(attr) {
                self.attributes.push(*attr);
            }
        }
    }

    /// Update the group constraints according to the provided constraints to
    /// add and the `include` constraints to remove.
    pub fn update_constraints(
        &mut self,
        constraints_to_add: Vec<Constraint>,
        include_to_remove: HashSet<String>,
    ) {
        // Add the new constraints
        self.constraints.extend(constraints_to_add);

        // Remove the include constraints
        self.constraints.retain(|c| {
            c.include.is_none()
                || !include_to_remove.contains(c.include.as_ref().expect("include is not none"))
        });
    }

    /// Returns the provenance of the group.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        match &self.lineage {
            Some(lineage) => lineage.provenance().to_owned(),
            None => Provenance::undefined(),
        }
    }
}
