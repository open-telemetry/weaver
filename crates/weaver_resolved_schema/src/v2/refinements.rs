//! A semantic convention refinements.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::{event::EventRefinement, metric::MetricRefinement, span::SpanRefinement};

/// Semantic convention refinements.
///
/// Refinements are a specialization of a signal that can be used to optimise documentation,
/// or code generation. A refinement will *always* match the conventions defined by the
/// signal it refines. Refinements cannot be inferred from signals over the wire (e.g. OTLP).
/// This is because any identifying feature of a refinement is used purely for codgen but has
/// no storage location in OTLP.
///
/// Note: Refinements will always include a "base" refinement for every signal definition.
///       For example, if a Metric signal named `my_metric` is defined, there will be
///       a metric refinement named `my_metric` as well.
///       This allows codegen to *only* interact with refinements, if desired, to
///       provide optimised methods for generating telemetry signals.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Refinements {
    /// A  list of span refinements.
    pub spans: Vec<SpanRefinement>,

    /// A  list of metric refinements.
    pub metrics: Vec<MetricRefinement>,

    /// A  list of event refinements.
    pub events: Vec<EventRefinement>,
}
