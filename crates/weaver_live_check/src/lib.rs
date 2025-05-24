// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::{collections::HashMap, rc::Rc};

use live_checker::LiveChecker;
use miette::Diagnostic;
use sample_attribute::SampleAttribute;
use sample_metric::{
    SampleExemplar, SampleExponentialHistogramDataPoint, SampleHistogramDataPoint, SampleMetric,
    SampleNumberDataPoint,
};
use sample_resource::SampleResource;
use sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
use weaver_semconv::group::GroupType;

/// Advisors for live checks
pub mod advice;
/// An ingester that reads samples from a JSON file.
pub mod json_file_ingester;
/// An ingester that reads samples from standard input.
pub mod json_stdin_ingester;
/// Live checker
pub mod live_checker;
/// The intermediary format for attributes
pub mod sample_attribute;
/// The intermediary format for metrics
pub mod sample_metric;
/// An intermediary format for resources
pub mod sample_resource;
/// The intermediary format for spans
pub mod sample_span;
/// An ingester that reads attribute names from a text file.
pub mod text_file_ingester;
/// An ingester that reads attribute names from standard input.
pub mod text_stdin_ingester;

/// Missing Attribute advice type
pub const MISSING_ATTRIBUTE_ADVICE_TYPE: &str = "missing_attribute";
/// Template Attribute advice type
pub const TEMPLATE_ATTRIBUTE_ADVICE_TYPE: &str = "template_attribute";
/// Missing Metric advice type
pub const MISSING_METRIC_ADVICE_TYPE: &str = "missing_metric";

/// Weaver live check errors
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Generic ingest error.
    #[error("Fatal error during ingest. {error}")]
    IngestError {
        /// The error that occurred.
        error: String,
    },

    /// Attempt to Ingest an empty line.
    #[error("Attempt to ingest an empty line.")]
    IngestEmptyLine,

    /// Advice error.
    #[error("Fatal error from Advisor. {error}")]
    AdviceError {
        /// The error that occurred.
        error: String,
    },

    /// Output error.
    #[error("Output error. {error}")]
    OutputError {
        /// The error that occurred.
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Ingesters implement a trait that returns an iterator of samples
pub trait Ingester {
    /// Ingest data and return an iterator of the output type
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error>;
}

/// Represents a sample root entity. A root entity has no contextual
/// dependency on a parent entity and can therefore be ingested independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sample {
    /// A sample attribute
    Attribute(SampleAttribute),
    /// A sample span
    Span(SampleSpan),
    /// A sample span event
    SpanEvent(SampleSpanEvent),
    /// A sample span link
    SpanLink(SampleSpanLink),
    /// A sample resource
    Resource(SampleResource),
    /// A sample metric
    Metric(SampleMetric),
}

/// Represents a sample entity with a reference to the inner type.
/// These entities can all be augmented with a live check result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleRef<'a> {
    /// A sample attribute
    Attribute(&'a SampleAttribute),
    /// A sample span
    Span(&'a SampleSpan),
    /// A sample span event
    SpanEvent(&'a SampleSpanEvent),
    /// A sample span link
    SpanLink(&'a SampleSpanLink),
    /// A sample resource
    Resource(&'a SampleResource),
    /// A sample metric
    Metric(&'a SampleMetric),
    /// A sample number data point
    NumberDataPoint(&'a SampleNumberDataPoint),
    /// A sample histogram data point
    HistogramDataPoint(&'a SampleHistogramDataPoint),
    /// A sample Exponential Histogram data point
    ExponentialHistogramDataPoint(&'a SampleExponentialHistogramDataPoint),
    /// A sample exemplar
    Exemplar(&'a SampleExemplar),
}

// Dispatch the live check to the sample type
impl LiveCheckRunner for Sample {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<ResolvedGroup>>,
    ) -> Result<(), Error> {
        match self {
            Sample::Attribute(attribute) => {
                attribute.run_live_check(live_checker, stats, parent_group)
            }
            Sample::Span(span) => span.run_live_check(live_checker, stats, parent_group),
            Sample::SpanEvent(span_event) => {
                span_event.run_live_check(live_checker, stats, parent_group)
            }
            Sample::SpanLink(span_link) => {
                span_link.run_live_check(live_checker, stats, parent_group)
            }
            Sample::Resource(resource) => {
                resource.run_live_check(live_checker, stats, parent_group)
            }
            Sample::Metric(metric) => metric.run_live_check(live_checker, stats, parent_group),
        }
    }
}

/// Represents a live check result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LiveCheckResult {
    /// Advice on the entity
    pub all_advice: Vec<Advice>,
    /// The highest advice level
    pub highest_advice_level: Option<AdviceLevel>,
}

impl LiveCheckResult {
    /// Create a new LiveCheckResult
    #[must_use]
    pub fn new() -> Self {
        LiveCheckResult {
            all_advice: Vec::new(),
            highest_advice_level: None,
        }
    }

    /// Add an advice to the result and update the highest advice level
    pub fn add_advice(&mut self, advice: Advice) {
        let advice_level = advice.advice_level.clone();
        if let Some(previous_highest) = &self.highest_advice_level {
            if previous_highest < &advice_level {
                self.highest_advice_level = Some(advice_level);
            }
        } else {
            self.highest_advice_level = Some(advice_level);
        }
        self.all_advice.push(advice);
    }

    /// Add a list of advice to the result and update the highest advice level
    pub fn add_advice_list(&mut self, advice: Vec<Advice>) {
        for advice in advice {
            self.add_advice(advice);
        }
    }
}

impl Default for LiveCheckResult {
    fn default() -> Self {
        LiveCheckResult::new()
    }
}

/// A live check report for a set of samples
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveCheckReport {
    /// The live check samples
    pub samples: Vec<Sample>,
    /// The statistics for the report
    pub statistics: LiveCheckStatistics,
}

/// The statistics for a live check report
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveCheckStatistics {
    /// The total number of sample entities
    pub total_entities: usize,
    /// The total number of sample entities by type
    pub total_entities_by_type: HashMap<String, usize>,
    /// The total number of advisories
    pub total_advisories: usize,
    /// The number of each advice level
    pub advice_level_counts: HashMap<AdviceLevel, usize>,
    /// The number of entities with each highest advice level
    pub highest_advice_level_counts: HashMap<AdviceLevel, usize>,
    /// The number of entities with no advice
    pub no_advice_count: usize,
    /// The number of entities with each advice type
    pub advice_type_counts: HashMap<String, usize>,
    /// The number of each attribute seen from the registry
    pub seen_registry_attributes: HashMap<String, usize>,
    /// The number of each non-registry attribute seen
    pub seen_non_registry_attributes: HashMap<String, usize>,
    /// The number of each metric seen from the registry
    pub seen_registry_metrics: HashMap<String, usize>,
    /// The number of each non-registry metric seen
    pub seen_non_registry_metrics: HashMap<String, usize>,
    /// Fraction of the registry covered by the attributes and metrics
    pub registry_coverage: f32,
}

impl LiveCheckStatistics {
    /// Create a new empty LiveCheckStatistics
    #[must_use]
    pub fn new(registry: &ResolvedRegistry) -> Self {
        let mut seen_attributes = HashMap::new();
        let mut seen_metrics = HashMap::new();
        for group in &registry.groups {
            for attribute in &group.attributes {
                if attribute.deprecated.is_none() {
                    let _ = seen_attributes.insert(attribute.name.clone(), 0);
                }
            }
            if group.r#type == GroupType::Metric && group.deprecated.is_none() {
                if let Some(metric_name) = &group.metric_name {
                    let _ = seen_metrics.insert(metric_name.clone(), 0);
                }
            }
        }
        LiveCheckStatistics {
            total_entities: 0,
            total_entities_by_type: HashMap::new(),
            total_advisories: 0,
            advice_level_counts: HashMap::new(),
            highest_advice_level_counts: HashMap::new(),
            no_advice_count: 0,
            advice_type_counts: HashMap::new(),
            seen_registry_attributes: seen_attributes,
            seen_non_registry_attributes: HashMap::new(),
            seen_registry_metrics: seen_metrics,
            seen_non_registry_metrics: HashMap::new(),
            registry_coverage: 0.0,
        }
    }

    /// Add a live check result to the stats
    pub fn maybe_add_live_check_result(&mut self, live_check_result: Option<&LiveCheckResult>) {
        if let Some(result) = live_check_result {
            for advice in &result.all_advice {
                // Count of total advisories
                self.add_advice(advice);
            }
            // Count of samples with the highest advice level
            if let Some(highest_advice_level) = &result.highest_advice_level {
                self.add_highest_advice_level(highest_advice_level);
            }

            // Count of samples with no advice
            if result.all_advice.is_empty() {
                self.inc_no_advice_count();
            }
        } else {
            // Count of samples with no advice
            self.inc_no_advice_count();
        }
    }

    /// Increment the total number of entities by type
    pub fn inc_entity_count(&mut self, entity_type: &str) {
        *self
            .total_entities_by_type
            .entry(entity_type.to_owned())
            .or_insert(0) += 1;
        self.total_entities += 1;
    }

    /// Add an advice to the statistics
    fn add_advice(&mut self, advice: &Advice) {
        *self
            .advice_level_counts
            .entry(advice.advice_level.clone())
            .or_insert(0) += 1;
        *self
            .advice_type_counts
            .entry(advice.advice_type.clone())
            .or_insert(0) += 1;
        self.total_advisories += 1;
    }

    /// Add a highest advice level to the statistics
    fn add_highest_advice_level(&mut self, advice: &AdviceLevel) {
        *self
            .highest_advice_level_counts
            .entry(advice.clone())
            .or_insert(0) += 1;
    }

    /// Increment the no advice count in the statistics
    fn inc_no_advice_count(&mut self) {
        self.no_advice_count += 1;
    }

    /// Add attribute name to coverage
    pub fn add_attribute_name_to_coverage(&mut self, seen_attribute_name: String) {
        if let Some(count) = self.seen_registry_attributes.get_mut(&seen_attribute_name) {
            // This is a registry attribute
            *count += 1;
        } else {
            // This is a non-registry attribute
            *self
                .seen_non_registry_attributes
                .entry(seen_attribute_name)
                .or_insert(0) += 1;
        }
    }

    /// Add metric name to coverage
    pub fn add_metric_name_to_coverage(&mut self, seen_metric_name: String) {
        if let Some(count) = self.seen_registry_metrics.get_mut(&seen_metric_name) {
            // This is a registry metric
            *count += 1;
        } else {
            // This is a non-registry metric
            *self
                .seen_non_registry_metrics
                .entry(seen_metric_name)
                .or_insert(0) += 1;
        }
    }

    /// Are there any violations in the statistics?
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.highest_advice_level_counts
            .contains_key(&AdviceLevel::Violation)
    }

    /// Finalize the statistics
    pub fn finalize(&mut self) {
        // Calculate the registry coverage
        // (non-zero attributes + non-zero metrics) / (total attributes + total metrics)
        let non_zero_attributes = self
            .seen_registry_attributes
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_attributes = self.seen_registry_attributes.len();

        let non_zero_metrics = self
            .seen_registry_metrics
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_metrics = self.seen_registry_metrics.len();

        let total_registry_items = total_registry_attributes + total_registry_metrics;

        if total_registry_items > 0 {
            self.registry_coverage =
                ((non_zero_attributes + non_zero_metrics) as f32) / (total_registry_items as f32);
        } else {
            self.registry_coverage = 0.0;
        }
    }
}

/// Samples implement this trait to run live checks on themselves
pub trait LiveCheckRunner {
    /// Run the live check
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<ResolvedGroup>>,
    ) -> Result<(), Error>;
}

// Run checks on all items in a collection that implement LiveCheckRunner
impl<T: LiveCheckRunner> LiveCheckRunner for Vec<T> {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<ResolvedGroup>>,
    ) -> Result<(), Error> {
        for item in self.iter_mut() {
            item.run_live_check(live_checker, stats, parent_group.clone())?;
        }
        Ok(())
    }
}

/// Samples implement this trait to run Advisors on themselves
pub trait Advisable {
    /// Get a reference to this entity as a SampleRef (for advisor calls)
    fn as_sample_ref(&self) -> SampleRef<'_>;

    /// Get entity type for statistics
    fn entity_type(&self) -> &str;

    /// Run advisors on this entity
    fn run_advisors(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<ResolvedGroup>>,
    ) -> Result<LiveCheckResult, Error> {
        let mut result = LiveCheckResult::new();

        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(self.as_sample_ref(), None, parent_group.clone())?;
            result.add_advice_list(advice_list);
        }

        stats.inc_entity_count(self.entity_type());
        stats.maybe_add_live_check_result(Some(&result));

        Ok(result)
    }
}

/// Get the JSON schema for the Sample struct
pub fn get_json_schema() -> Result<String, Error> {
    let schema = schemars::schema_for!(Sample);
    serde_json::to_string_pretty(&schema).map_err(|e| Error::OutputError {
        error: e.to_string(),
    })
}
