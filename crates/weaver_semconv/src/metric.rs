// SPDX-License-Identifier: Apache-2.0

//! Metric specification.

use crate::attribute::AttributeSpec;
use crate::group::InstrumentSpec;
use serde::{Deserialize, Serialize};

/// A metric specification.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MetricSpec {
    /// Metric name.
    pub name: String,
    /// Brief description of the metric.
    pub brief: String,
    /// Note on the metric.
    pub note: String,
    /// Set of attribute ids attached to the metric.
    #[serde(default)]
    pub attributes: Vec<AttributeSpec>,
    /// Type of the metric (e.g. gauge, histogram, ...).
    pub instrument: InstrumentSpec,
    /// Unit of the metric.
    pub unit: Option<String>,
}

impl MetricSpec {
    /// Returns the name of the metric.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the brief description of the metric.
    #[must_use]
    pub fn brief(&self) -> &str {
        &self.brief
    }

    /// Returns the note on the metric.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Returns the unit of the metric.
    #[must_use]
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
}

/// A metric definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct MetricSpecWithProvenance {
    /// The metric definition.
    pub metric: MetricSpec,
    /// The provenance of the metric (path or URL).
    pub provenance: String,
}
