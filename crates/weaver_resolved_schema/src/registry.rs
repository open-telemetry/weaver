// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! A semantic convention registry.

use crate::attribute::{AttributeRef, UnresolvedAttribute};
use serde::{Deserialize, Serialize};

use crate::catalog::Stability;
use crate::lineage::GroupLineage;
use crate::metric::Instrument;
use crate::signal::SpanKind;

/// A semantic convention registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    /// A list of semantic convention groups.
    pub groups: Vec<Group>,
}

/// A registry containing unresolved groups.
#[derive(Debug)]
pub struct UnresolvedRegistry {
    /// The semantic convention registry.
    pub registry: Registry,
    /// List of unresolved groups that belong to the registry.
    pub groups: Vec<UnresolvedGroup>,
}

/// Group specification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Group {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the group including the specific fields for each type.
    pub typed_group: TypedGroup,
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
    pub deprecated: Option<String>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<AttributeRef>,
    /// The lineage of the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<GroupLineage>,
}

/// A group containing unresolved attributes.
#[derive(Debug)]
pub struct UnresolvedGroup {
    /// The group specification.
    pub group: Group,
    /// List of unresolved attributes that belong to the semantic convention
    /// group.
    pub attributes: Vec<UnresolvedAttribute>,
    /// The provenance of the group (URL or path).
    pub provenance: String,
}

/// An enum representing the type of the group including the specific fields
/// for each type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum TypedGroup {
    /// A semantic convention group representing an attribute group.
    AttributeGroup {},
    /// A semantic convention group representing a span.
    Span {
        /// Specifies the kind of the span.
        /// Note: only valid if type is span (the default)
        span_kind: Option<SpanKind>,
        /// List of strings that specify the ids of event semantic conventions
        /// associated with this span semantic convention.
        /// Note: only valid if type is span (the default)
        #[serde(default)]
        events: Vec<String>,
    },
    /// A semantic convention group representing an event.
    Event {
        /// The name of the event. If not specified, the prefix is used.
        /// If prefix is empty (or unspecified), name is required.
        name: Option<String>,
    },
    /// A semantic convention group representing a metric.
    Metric {
        /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
        /// Note: This field is required if type is metric.
        metric_name: Option<String>,
        /// The instrument type that should be used to record the metric. Note that
        /// the semantic conventions must be written using the names of the
        /// synchronous instrument types (counter, gauge, updowncounter and
        /// histogram).
        /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
        /// Note: This field is required if type is metric.
        instrument: Option<Instrument>,
        /// The unit in which the metric is measured, which should adhere to the
        /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
        /// Note: This field is required if type is metric.
        unit: Option<String>,
    },
    /// A semantic convention group representing a metric group.
    MetricGroup {},
    /// A semantic convention group representing a resource.
    Resource {},
    /// A semantic convention group representing a scope.
    Scope {},
}

/// Allow to define additional requirements on the semantic convention.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
