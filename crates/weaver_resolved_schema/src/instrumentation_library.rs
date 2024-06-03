// SPDX-License-Identifier: Apache-2.0

//! Define an instrumentation library.

use crate::signal::{Event, MultivariateMetric, Span, UnivariateMetric};
use crate::tags::Tags;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An instrumentation library specification.
/// MUST be used both by applications and libraries.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentationLibrary {
    /// An optional name for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// An optional version for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// A set of tags for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
    /// A set of univariate metrics produced by the instrumentation library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    univariate_metrics: Vec<UnivariateMetric>,
    /// A set of multivariate metrics produced by the instrumentation library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    multivariate_metrics: Vec<MultivariateMetric>,
    /// A set of events produced by the instrumentation library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<Event>,
    /// A set of spans produced by the instrumentation library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    spans: Vec<Span>,
}
