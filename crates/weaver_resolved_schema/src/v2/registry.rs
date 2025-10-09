//! A semantic convention registry.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::{
    entity::Entity,
    event::{Event, EventRefinement},
    metric::{Metric, MetricRefinement},
    span::{Span, SpanRefinement},
};

/// A semantic convention registry.
///
/// The semantic convention is composed of two major components:
///
/// - Signals: Definitions of metrics, logs, etc. that will be sent over the wire (e.g. OTLP).
/// - Refinements: Specialization of a signal that can be used to optimise documentation,
///   or code generation. A refinement will *always* match the conventions defined by the
///   signal it refines. Refinements cannot be inferred from signals over the wire (e.g. OTLP).
///   This is because any identifying feature of a refinement is used purely for codgen but has
///   no storage location in OTLP.
///
/// Note: Refinements will always include a "base" refinement for every signal definition.
///       For example, if a Metric signal named `my_metric` is defined, there will be
///       a metric refinement named `my_metric` as well.
///       This allows codegen to *only* interact with refinements, if desired, to
///       provide optimised methods for generating telemetry signals.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The semantic convention registry url.
    ///
    /// This is the base URL, under which this registry can be found.
    pub registry_url: String,

    /// A  list of span signal definitions.
    pub spans: Vec<Span>,

    /// A  list of metric signal definitions.
    pub metrics: Vec<Metric>,

    /// A  list of event signal definitions.
    pub events: Vec<Event>,

    /// A  list of entity signal definitions.
    pub entities: Vec<Entity>,

    /// A  list of span refinements.
    pub span_refinements: Vec<SpanRefinement>,

    /// A  list of metric refinements.
    pub metric_refinements: Vec<MetricRefinement>,

    /// A  list of event refinements.
    pub event_refinements: Vec<EventRefinement>,
}
