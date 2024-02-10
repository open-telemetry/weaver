// SPDX-License-Identifier: Apache-2.0

//! A resource metrics specification.

use crate::attribute::Attribute;
use crate::metric_group::MetricGroup;
use crate::tags::Tags;
use crate::univariate_metric::UnivariateMetric;
use serde::{Deserialize, Serialize};

/// A resource metrics specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct ResourceMetrics {
    /// Common attributes shared across metrics and metric groups.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Definitions of all metrics this application or library generates (classic
    /// univariate OTel metrics).
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<UnivariateMetric>,
    /// Definitions of all multivariate metrics this application or library
    /// generates.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metric_groups: Vec<MetricGroup>,
    /// A set of tags for the resource metrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

impl ResourceMetrics {
    /// Returns the number of metrics.
    pub fn metrics_count(&self) -> usize {
        self.metrics.len()
    }

    /// Returns the number of metric groups.
    pub fn metric_groups_count(&self) -> usize {
        self.metric_groups.len()
    }

    /// Returns a metric by name or None if not found.
    /// Note: this is a linear search.
    pub fn metric(&self, name: &str) -> Option<&UnivariateMetric> {
        self.metrics.iter().find(|metric| metric.name() == name)
    }

    /// Returns a vector of metrics.
    pub fn metrics(&self) -> Vec<&UnivariateMetric> {
        self.metrics.iter().collect()
    }

    /// Returns a metric group by name or None if not found.
    /// Note: this is a linear search.
    pub fn metric_group(&self, name: &str) -> Option<&MetricGroup> {
        self.metric_groups
            .iter()
            .find(|metric_group| metric_group.name() == name)
    }

    /// Returns a vector of metric groups.
    pub fn metric_groups(&self) -> Vec<&MetricGroup> {
        self.metric_groups.iter().collect()
    }
}
