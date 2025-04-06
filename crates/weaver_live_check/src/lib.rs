// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::collections::HashMap;

use miette::Diagnostic;
use sample::SampleAttribute;
use serde::Serialize;
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_common::{
    diagnostic::{DiagnosticMessage, DiagnosticMessages},
    Logger,
};

/// Advisors for live checks
pub mod attribute_advice;
/// An ingester that reads attribute names from a text file.
pub mod attribute_file_ingester;
/// An ingester that reads attribute names and values from a JSON file.
pub mod attribute_json_file_ingester;
/// An ingester that reads attribute names and values from standard input.
pub mod attribute_json_stdin_ingester;
/// Attribute live checker
pub mod attribute_live_check;
/// An ingester that reads attribute names from standard input.
pub mod attribute_stdin_ingester;
/// The intermediary format
pub mod sample;

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

/// Ingesters implement a generic trait that returns an iterator
pub trait Ingester<T> {
    /// Ingest data and return an iterator of the output type
    fn ingest(
        &self,
        logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = T>>, Error>;
}

/// Represents a live check attribute parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveCheckAttribute {
    /// The sample attribute
    pub sample_attribute: SampleAttribute,
    /// Advice on the attribute
    pub all_advice: Vec<Advice>,
    /// The highest advice level
    pub highest_advice_level: Option<AdviceLevel>,
}

impl LiveCheckAttribute {
    /// Create a new LiveCheckAttribute
    #[must_use]
    pub fn new(sample_attribute: SampleAttribute) -> Self {
        LiveCheckAttribute {
            sample_attribute,
            all_advice: Vec::new(),
            highest_advice_level: None,
        }
    }

    /// Add an advice to the attribute and update the highest advice level
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

/// A live check report for a set of attributes
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveCheckReport {
    /// The live check attributes
    pub attributes: Vec<LiveCheckAttribute>,
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
}

impl Default for LiveCheckStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveCheckStatistics {
    /// Create a new empty LiveCheckStatistics
    #[must_use]
    pub fn new() -> Self {
        LiveCheckStatistics {
            total_attributes: 0,
            total_advisories: 0,
            advice_level_counts: HashMap::new(),
            highest_advice_level_counts: HashMap::new(),
            no_advice_count: 0,
            advice_type_counts: HashMap::new(),
        }
    }

    /// Update statistics based on a live check attribute
    pub fn update(&mut self, attribute_result: &LiveCheckAttribute) {
        self.total_attributes += 1;

        // Count of advisories by type
        for advice in &attribute_result.all_advice {
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
        }

        // Count of attributes with the highest advice level
        if let Some(highest_advice_level) = &attribute_result.highest_advice_level {
            let highest_advice_level_count = self
                .highest_advice_level_counts
                .entry(highest_advice_level.clone())
                .or_insert(0);
            *highest_advice_level_count += 1;
        }

        // Count of attributes with no advice
        if attribute_result.all_advice.is_empty() {
            self.no_advice_count += 1;
        }
    }
}
