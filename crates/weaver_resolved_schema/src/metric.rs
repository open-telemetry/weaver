// SPDX-License-Identifier: Apache-2.0

//! Specification of a resolved metric.

use crate::tags::Tags;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An internal reference to a metric in the catalog.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, JsonSchema)]
pub struct MetricRef(pub u32);

/// A metric definition.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// Metric name.
    pub name: String,
    /// Brief description of the metric.
    pub brief: String,
    /// Brief description of the metric.
    pub note: String,
    /// Type of the metric (e.g. gauge, histogram, ...).
    pub instrument: Instrument,
    /// Unit of the metric.
    pub unit: Option<String>,
    /// A set of tags for the metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

/// The type of the metric.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub enum Instrument {
    /// An up-down counter metric.
    UpDownCounter,
    /// A counter metric.
    Counter,
    /// A gauge metric.
    Gauge,
    /// A histogram metric.
    Histogram,
}
