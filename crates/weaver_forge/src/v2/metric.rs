//! Metric related definitions structs.

use crate::v2::attribute::Attribute;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    group::InstrumentSpec,
    v2::{signal_id::SignalId, CommonFields},
};

/// The definition of a metric signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// The name of the metric.
    pub name: SignalId,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    pub instrument: InstrumentSpec,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    pub unit: String,
    /// List of attributes that should be included on this metric.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<MetricAttribute>,
    // TODO - Should Entity Associations be "strong" links?
    /// Which resources this metric should be associated with.
    ///
    /// This list is an "any of" list, where a metric may be associated with one or more entities, but should
    /// be associated with at least one in this list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A special type of reference to attributes that remembers metric-specific information.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricAttribute {
    /// Base metric definitions.
    #[serde(flatten)]
    pub base: Attribute,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    ///
    /// Note: For attributes that are "recommended" or "opt-in" - not all metric source will
    /// create timeseries with these attributes, but for any given timeseries instance, the attributes that *were* present
    /// should *remain* present. That is - a metric timeseries cannot drop attributes during its lifetime.
    pub requirement_level: RequirementLevel,
}

/// A refinement of a metric signal, for use in code-gen or specific library application.
///
/// A refinement represents a "view" of a Metric that is highly optimised for a particular implementation.
/// e.g. for HTTP metrics, there may be a refinement that provides only the necessary information for dealing with Java's HTTP
/// client library, and drops optional or extraneous information from the underlying http metric.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct MetricRefinement {
    /// The identity of the refinement.
    pub id: SignalId,

    // TODO - This is a lazy way of doing this.  We use `type` to refer
    // to the underlying metric defintiion, but override all fields here.
    // We probably should copy-paste all the "metric" attributes here
    // including the `ty`
    /// The definition of the metric refinement.
    #[serde(flatten)]
    pub metric: Metric,
}
