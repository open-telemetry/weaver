// SPDX-License-Identifier: Apache-2.0

//! Define the concept of signal.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attribute::AttributeRef;
use crate::body::Body;
use crate::metric::MetricRef;
use crate::tags::Tags;

/// A univariate metric signal.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnivariateMetric {
    /// References to attributes defined in the catalog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<AttributeRef>,
    /// Reference to a metric defined in the catalog.
    metric: MetricRef,
    /// A set of tags for the univariate metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
}

/// A multivariate metric signal.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultivariateMetric {
    /// The name of the multivariate metric.
    name: String,
    /// References to attributes defined in the catalog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<AttributeRef>,
    /// The metrics of the multivariate metric.
    metrics: Vec<MetricRef>,
    /// Brief description of the multivariate metric.
    brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    note: Option<String>,
    /// A set of tags for the multivariate metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
}

/// An event specification, used for both Span and Log events.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The name of the event
    name: String,
    /// References to attributes defined in the catalog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<AttributeRef>,
    /// Brief description of the event.
    brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    note: Option<String>,
    /// A set of tags for the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
    /// The body of the event, not used for Span events.
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Body>,
}

/// A span signal.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The name of the span.
    name: String,
    /// References to attributes defined in the catalog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<AttributeRef>,
    /// The kind of the span.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<SpanKind>,
    /// The events of the span.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<Event>,
    /// The links of the span.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    links: Vec<SpanLink>,
    /// Brief description of the span.
    brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    note: Option<String>,
    /// A set of tags for the span.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
}

/// The span kind.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub enum SpanKind {
    /// An internal span.
    Internal,
    /// A client span.
    Client,
    /// A server span.
    Server,
    /// A producer span.
    Producer,
    /// A consumer span.
    Consumer,
}

/// A span link specification.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpanLink {
    /// The name of the span link.
    pub link_name: String,
    /// The attributes of the span link.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeRef>,
    /// Brief description of the span link.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the span link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}
