// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::collections::HashMap;

use miette::Diagnostic;
use sample_attribute::SampleAttribute;
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
/// An ingester that reads attribute names and values from a JSON file.
pub mod json_file_ingester;
/// An ingester that reads attribute names and values from standard input.
pub mod json_stdin_ingester;
/// Attribute live checker
pub mod live_checker;
/// The intermediary format for attributes
pub mod sample_attribute;
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

impl LiveCheckReport {
    /// Return true if there are any violations in the report
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.statistics
            .highest_advice_level_counts
            .contains_key(&AdviceLevel::Violation)
    }
}

/// The statistics for a live check report
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveCheckStatistics {
    /// The total number of attributes
    pub total_attributes: usize,
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
            total_attributes: 0,
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

    /// Are there any violations in the statistics?
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.highest_advice_level_counts
            .contains_key(&AdviceLevel::Violation)
    }

    fn update_for_attribute(&mut self, sample_attribute: &mut SampleAttribute) {
        self.total_attributes += 1;
        let mut seen_attribute_name = sample_attribute.name.clone();
        if let Some(result) = &mut sample_attribute.live_check_result {
            for advice in &mut result.all_advice {
                // Count of total advisories
                self.total_advisories += 1;

                let advice_level_count = self
                    .advice_level_counts
                    .entry(advice.advice_level.clone())
                    .or_insert(0);
                *advice_level_count += 1;

                // Count of advisories by type
                let advice_type_count = self
                    .advice_type_counts
                    .entry(advice.advice_type.clone())
                    .or_insert(0);
                *advice_type_count += 1;

                // If the advice is a template, adjust the name
                if advice.advice_type == TEMPLATE_ATTRIBUTE_ADVICE_TYPE {
                    if let Some(template_name) = advice.value.as_str() {
                        seen_attribute_name = template_name.to_owned();
                    }
                }
            }
            // Count of attributes with the highest advice level
            if let Some(highest_advice_level) = &result.highest_advice_level {
                let highest_advice_level_count = self
                    .highest_advice_level_counts
                    .entry(highest_advice_level.clone())
                    .or_insert(0);
                *highest_advice_level_count += 1;
            }

            // Count of attributes with no advice
            if result.all_advice.is_empty() {
                self.no_advice_count += 1;
            }
        } else {
            // Count of attributes with no advice
            self.no_advice_count += 1;
        }
        if let Some(count) = self.seen_registry_attributes.get_mut(&seen_attribute_name) {
            // This is a registry attribute
            *count += 1;
        } else {
            // This is a non-registry attribute
            let seen_non_registry_count = self
                .seen_non_registry_attributes
                .entry(seen_attribute_name.clone())
                .or_insert(0);
            *seen_non_registry_count += 1;
        }
    }

    fn update_for_span_event(&mut self, sample_span_event: &mut SampleSpanEvent) {
        for attribute in &mut sample_span_event.attributes {
            self.update_for_attribute(attribute);
        }
    }

    fn update_for_span_link(&mut self, sample_span_link: &mut SampleSpanLink) {
        for attribute in &mut sample_span_link.attributes {
            self.update_for_attribute(attribute);
        }
    }

    /// Update statistics based on a live check result
    pub fn update(&mut self, sample: &mut Sample) {
        match sample {
            Sample::Attribute(sample_attribute) => {
                self.update_for_attribute(sample_attribute);
            }
            Sample::Span(sample_span) => {
                for attribute in &mut sample_span.attributes {
                    self.update_for_attribute(attribute);
                }
                for span_event in &mut sample_span.span_events {
                    self.update_for_span_event(span_event);
                }
                for span_link in &mut sample_span.span_links {
                    self.update_for_span_link(span_link);
                }
            }
            Sample::SpanEvent(sample_span_event) => {
                self.update_for_span_event(sample_span_event);
            }
            Sample::SpanLink(sample_span_link) => {
                self.update_for_span_link(sample_span_link);
            }
        };
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
