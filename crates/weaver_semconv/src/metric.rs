// SPDX-License-Identifier: Apache-2.0

//! Metric specification.

use std::fmt::{Display, Formatter};

use crate::attribute::AttributeSpec;
use crate::group::InstrumentSpec;
use schemars::JsonSchema;
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
    /// Number type of the metric's value.
    pub value_type: Option<MetricValueTypeSpec>,
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

/// Primitive or array types.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricValueTypeSpec {
    /// An integer value (signed 64 bit integer).
    Int,
    /// A double value (double precision floating point (IEEE 754-1985)).
    Double,
}

/// Implements a human readable display for MetricValueTypeSpec.
impl Display for MetricValueTypeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricValueTypeSpec::Int => write!(f, "int"),
            MetricValueTypeSpec::Double => write!(f, "double"),
        }
    }
}
