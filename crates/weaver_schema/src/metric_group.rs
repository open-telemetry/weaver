// SPDX-License-Identifier: Apache-2.0

//! Multivariate metrics.

use serde::{Deserialize, Serialize};

use crate::attribute::Attribute;
use crate::tags::Tags;
use weaver_semconv::group::InstrumentSpec;

/// The specification of a metric group.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct MetricGroup {
    /// The name of the metric group.
    pub name: String,
    /// The attributes of the metric group.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
    /// The metrics of the metric group.
    #[serde(default)]
    pub metrics: Vec<Metric>,
    /// Brief description of the metric group.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the metric group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

/// A metric specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Metric {
    /// A reference to a metric defined in a semantic convention catalog.
    Ref {
        /// The reference to the metric.
        r#ref: String,
        /// A set of tags for the metric group.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },

    /// A fully defined metric.
    Metric {
        /// Metric name.
        name: String,
        /// Brief description of the metric.
        brief: String,
        /// Note on the metric.
        note: String,
        /// Attributes of the metric.
        #[serde(default)]
        attributes: Vec<Attribute>,
        /// Type of the metric (e.g. gauge, histogram, ...).
        instrument: InstrumentSpec,
        /// Unit of the metric.
        unit: Option<String>,
        /// A set of tags for the metric.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },
}

impl MetricGroup {
    /// Returns the name of the metric group
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns an attribute by its id.
    #[must_use]
    pub fn attribute(&self, id: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|a| a.id() == id)
    }

    /// Returns the tags of the metric group.
    #[must_use]
    pub fn tags(&self) -> Option<&Tags> {
        self.tags.as_ref()
    }
}
