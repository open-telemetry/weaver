// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::collections::HashMap;

use live_checker::LiveChecker;
use miette::Diagnostic;
use sample_attribute::SampleAttribute;
use sample_resource::SampleResource;
use sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink};
use serde::{Deserialize, Serialize};
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_common::{
    diagnostic::{DiagnosticMessage, DiagnosticMessages},
    Logger,
};
use weaver_forge::registry::ResolvedRegistry;

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
    fn ingest(
        &self,
        logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = Sample>>, Error>;
}

/// Represents a sample entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

/// Represents a sample entity with a reference to the inner type
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
}

// Dispatch the live check to the sample type
impl LiveCheckRunner for Sample {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        match self {
            Sample::Attribute(attribute) => attribute.run_live_check(live_checker, stats),
            Sample::Span(span) => span.run_live_check(live_checker, stats),
            Sample::SpanEvent(span_event) => span_event.run_live_check(live_checker, stats),
            Sample::SpanLink(span_link) => span_link.run_live_check(live_checker, stats),
            Sample::Resource(resource) => resource.run_live_check(live_checker, stats),
        }
    }
}

/// Represents a live check result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The number of attributes with each highest advice level
    pub highest_advice_level_counts: HashMap<AdviceLevel, usize>,
    /// The number of attributes with no advice
    pub no_advice_count: usize,
    /// The number of attributes with each advice type
    pub advice_type_counts: HashMap<String, usize>,
    /// The number of each attribute seen from the registry
    pub seen_registry_attributes: HashMap<String, usize>,
    /// The number of each non-registry attribute seen
    pub seen_non_registry_attributes: HashMap<String, usize>,
    /// Fraction of the registry covered by the attributes
    pub registry_coverage: f32,
}

impl LiveCheckStatistics {
    /// Create a new empty LiveCheckStatistics
    #[must_use]
    pub fn new(registry: &ResolvedRegistry) -> Self {
        let mut seen_attributes = HashMap::new();
        for group in &registry.groups {
            for attribute in &group.attributes {
                if attribute.deprecated.is_none() {
                    let _ = seen_attributes.insert(attribute.name.clone(), 0);
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

    /// Are there any violations in the statistics?
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.highest_advice_level_counts
            .contains_key(&AdviceLevel::Violation)
    }

    /// Finalize the statistics
    pub fn finalize(&mut self) {
        // Calculate the registry coverage
        // non-zero attributes / total attributes
        let non_zero_attributes = self
            .seen_registry_attributes
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_attributes = self.seen_registry_attributes.len();
        if total_registry_attributes > 0 {
            self.registry_coverage =
                (non_zero_attributes as f32) / (total_registry_attributes as f32);
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
    ) -> Result<(), Error>;
}
