// SPDX-License-Identifier: Apache-2.0

//! A univariate metric specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};
use weaver_semconv::group::InstrumentSpec;

/// A univariate metric specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum UnivariateMetric {
    /// A reference to a metric.
    Ref {
        /// The reference to the metric.
        r#ref: String,
        /// The attributes of the metric.
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<Attribute>,
        /// A set of tags for the metric.
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

impl UnivariateMetric {
    /// Returns the name of the metric.
    pub fn name(&self) -> String {
        match self {
            UnivariateMetric::Ref { r#ref, .. } => r#ref.clone(),
            UnivariateMetric::Metric { name, .. } => name.clone(),
        }
    }

    /// Returns the brief description of the metric.
    pub fn brief(&self) -> String {
        match self {
            UnivariateMetric::Ref { .. } => String::new(),
            UnivariateMetric::Metric { brief, .. } => brief.clone(),
        }
    }

    /// Returns the note on the metric.
    pub fn note(&self) -> String {
        match self {
            UnivariateMetric::Ref { .. } => String::new(),
            UnivariateMetric::Metric { note, .. } => note.clone(),
        }
    }

    /// Returns the tags of the metric.
    pub fn tags(&self) -> Option<&Tags> {
        match self {
            UnivariateMetric::Ref { tags, .. } => tags.as_ref(),
            UnivariateMetric::Metric { tags, .. } => tags.as_ref(),
        }
    }

    /// Returns an attribute by its id.
    pub fn attribute(&self, id: &str) -> Option<&Attribute> {
        match self {
            UnivariateMetric::Ref { attributes, .. } => attributes.iter().find(|a| a.id() == id),
            UnivariateMetric::Metric { attributes, .. } => attributes.iter().find(|a| a.id() == id),
        }
    }
}
