//! A semantic convention registry.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::span::{Span, SpanRefinement};

/// A semantic convention registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The semantic convention registry url.
    pub registry_url: String,

    /// A  list of span definitions.
    pub spans: Vec<Span>,

    /// A  list of span refinements.
    pub span_refinements: Vec<SpanRefinement>,
    // TODO - Signal types.
}
