// SPDX-License-Identifier: Apache-2.0

//! A schema specification.

use serde::{Deserialize, Serialize};

use crate::event::Event;
use crate::instrumentation_library::InstrumentationLibrary;
use crate::metric_group::MetricGroup;
use crate::resource::Resource;
use crate::resource_events::ResourceEvents;
use crate::resource_metrics::ResourceMetrics;
use crate::resource_spans::ResourceSpans;
use crate::span::Span;
use crate::tags::Tags;
use crate::univariate_metric::UnivariateMetric;

/// Definition of the telemetry schema for an application or a library.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SchemaSpec {
    /// A set of tags for the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
    /// A common resource specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Resource>,
    /// The instrumentation library specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrumentation_library: Option<InstrumentationLibrary>,
    /// A resource metrics specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_metrics: Option<ResourceMetrics>,
    /// A resource events specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_events: Option<ResourceEvents>,
    /// A resource spans specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_spans: Option<ResourceSpans>,
}

impl SchemaSpec {
    /// Returns the number of metrics.
    #[must_use]
    pub fn metrics_count(&self) -> usize {
        self.resource_metrics
            .as_ref()
            .map_or(0, |resource_metrics| resource_metrics.metrics_count())
    }

    /// Returns the number of metric groups.
    #[must_use]
    pub fn metric_groups_count(&self) -> usize {
        self.resource_metrics
            .as_ref()
            .map_or(0, |resource_metrics| resource_metrics.metric_groups_count())
    }

    /// Returns the number of events.
    #[must_use]
    pub fn events_count(&self) -> usize {
        self.resource_events
            .as_ref()
            .map_or(0, |resource_events| resource_events.events_count())
    }

    /// Returns the number of spans.
    #[must_use]
    pub fn spans_count(&self) -> usize {
        self.resource_spans
            .as_ref()
            .map_or(0, |resource_spans| resource_spans.spans_count())
    }

    /// Returns a metric by name or None if not found.
    #[must_use]
    pub fn metric(&self, name: &str) -> Option<&UnivariateMetric> {
        self.resource_metrics
            .as_ref()
            .and_then(|resource_metrics| resource_metrics.metric(name))
    }

    /// Returns a metric group by name or None if not found.
    #[must_use]
    pub fn metric_group(&self, name: &str) -> Option<&MetricGroup> {
        self.resource_metrics
            .as_ref()
            .and_then(|resource_metrics| resource_metrics.metric_group(name))
    }

    /// Returns a resource or None if not found.
    #[must_use]
    pub fn resource(&self) -> Option<&Resource> {
        self.resource.as_ref()
    }

    /// Returns a vector of metrics.
    #[must_use]
    pub fn metrics(&self) -> Vec<&UnivariateMetric> {
        self.resource_metrics
            .as_ref()
            .map_or(Vec::<&UnivariateMetric>::new(), |resource_metrics| {
                resource_metrics.metrics()
            })
    }

    /// Returns a vector of metric groups.
    #[must_use]
    pub fn metric_groups(&self) -> Vec<&MetricGroup> {
        self.resource_metrics
            .as_ref()
            .map_or(Vec::<&MetricGroup>::new(), |resource_metrics| {
                resource_metrics.metric_groups()
            })
    }

    /// Returns a vector over the events.
    #[must_use]
    pub fn events(&self) -> Vec<&Event> {
        self.resource_events
            .as_ref()
            .map_or(Vec::<&Event>::new(), |resource_events| {
                resource_events.events()
            })
    }

    /// Returns a slice of spans.
    #[must_use]
    pub fn spans(&self) -> Vec<&Span> {
        self.resource_spans
            .as_ref()
            .map_or(Vec::<&Span>::new(), |resource_spans| resource_spans.spans())
    }

    /// Returns an event by name or None if not found.
    #[must_use]
    pub fn event(&self, event_name: &str) -> Option<&Event> {
        self.resource_events
            .as_ref()
            .and_then(|resource_events| resource_events.event(event_name))
    }

    /// Returns a span by name or None if not found.
    #[must_use]
    pub fn span(&self, span_name: &str) -> Option<&Span> {
        self.resource_spans
            .as_ref()
            .and_then(|resource_spans| resource_spans.span(span_name))
    }
}
