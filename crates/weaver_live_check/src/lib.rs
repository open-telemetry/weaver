// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::rc::Rc;

use live_checker::LiveChecker;
use miette::Diagnostic;
use sample_attribute::SampleAttribute;
use sample_log::SampleLog;
use sample_metric::{
    SampleExemplar, SampleExponentialHistogramDataPoint, SampleHistogramDataPoint, SampleMetric,
    SampleNumberDataPoint,
};
use sample_resource::SampleResource;
use sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::{
    registry::{ResolvedGroup, ResolvedRegistry},
    v2::registry::ForgeResolvedRegistry,
};
use weaver_semconv::{
    attribute::AttributeType, deprecated::Deprecated, group::InstrumentSpec, stability::Stability,
};

/// Advisors for live checks
pub mod advice;
/// An ingester that reads samples from a JSON file.
pub mod json_file_ingester;
/// An ingester that reads samples from standard input.
pub mod json_stdin_ingester;
/// Live checker
pub mod live_checker;
/// OTLP logger for emitting policy findings as log records
pub mod otlp_logger;
/// The intermediary format for attributes
pub mod sample_attribute;
/// The intermediary format for logs
pub mod sample_log;
/// The intermediary format for metrics
pub mod sample_metric;
/// An intermediary format for resources
pub mod sample_resource;
/// The intermediary format for spans
pub mod sample_span;
/// Statistics tracking for live check reports
mod stats;
/// An ingester that reads attribute names from a text file.
pub mod text_file_ingester;
/// An ingester that reads attribute names from standard input.
pub mod text_stdin_ingester;

// Re-export statistics types from stats module
pub use stats::{CumulativeStatistics, DisabledStatistics, LiveCheckStatistics};

/// Missing Attribute advice type
pub const MISSING_ATTRIBUTE_ADVICE_TYPE: &str = "missing_attribute";
/// Template Attribute advice type
pub const TEMPLATE_ATTRIBUTE_ADVICE_TYPE: &str = "template_attribute";
/// Missing Metric advice type
pub const MISSING_METRIC_ADVICE_TYPE: &str = "missing_metric";
/// Missing Event advice type
pub const MISSING_EVENT_ADVICE_TYPE: &str = "missing_event";
/// Deprecated advice type
pub const DEPRECATED_ADVICE_TYPE: &str = "deprecated";
/// Type Mismatch advice type
pub const TYPE_MISMATCH_ADVICE_TYPE: &str = "type_mismatch";
/// Unstable advice type
pub const NOT_STABLE_ADVICE_TYPE: &str = "not_stable";
/// Unit mismatch advice type
pub const UNIT_MISMATCH_ADVICE_TYPE: &str = "unit_mismatch";
/// Instrument mismatch advice type
pub const UNEXPECTED_INSTRUMENT_ADVICE_TYPE: &str = "unexpected_instrument";
/// Undefined enum variant advice type
pub const UNDEFINED_ENUM_VARIANT_ADVICE_TYPE: &str = "undefined_enum_variant";

/// Attribute name key in advice context
pub const ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: &str = "attribute_name";
/// Attribute value key in advice context
pub const ATTRIBUTE_VALUE_ADVICE_CONTEXT_KEY: &str = "attribute_value";
///Attribute type key in advice context
pub const ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY: &str = "attribute_type";
/// Deprecation reason key in advice context
pub const DEPRECATION_REASON_ADVICE_CONTEXT_KEY: &str = "deprecation_reason";
/// Deprecation note key in advice context
pub const DEPRECATION_NOTE_ADVICE_CONTEXT_KEY: &str = "deprecation_note";
/// Stability key in advice context
pub const STABILITY_ADVICE_CONTEXT_KEY: &str = "stability";
/// Unit key in advice context
pub const UNIT_ADVICE_CONTEXT_KEY: &str = "unit";
/// Instrument key in advice context
pub const INSTRUMENT_ADVICE_CONTEXT_KEY: &str = "instrument";
/// Expected value key in advice context
pub const EXPECTED_VALUE_ADVICE_CONTEXT_KEY: &str = "expected";
/// Event name key in advice context
pub const EVENT_NAME_ADVICE_CONTEXT_KEY: &str = "event_name";
/// Metric name key in advice context
pub const METRIC_NAME_ADVICE_CONTEXT_KEY: &str = "metric_name";

/// Embedded default live check rego policies
pub const DEFAULT_LIVE_CHECK_REGO: &str =
    include_str!("../../../defaults/policies/live_check_advice/otel.rego");

/// Default live check rego policy path - used in error messages
pub const DEFAULT_LIVE_CHECK_REGO_POLICY_PATH: &str =
    "defaults/policies/live_check_advice/otel.rego";

/// Embedded default live check jq preprocessor
pub const DEFAULT_LIVE_CHECK_JQ: &str = include_str!("../../../defaults/jq/advice.jq");

/// Versioned enum for the registry
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum VersionedRegistry {
    /// v1 ResolvedRegistry
    V1(ResolvedRegistry),
    /// v2 ForgeResolvedRegistry
    V2(ForgeResolvedRegistry),
}

/// Versioned enum for the attribute
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum VersionedAttribute {
    /// v1 Attribute
    V1(weaver_resolved_schema::attribute::Attribute),
    /// v2 Attribute
    V2(weaver_forge::v2::attribute::Attribute),
}

impl VersionedAttribute {
    /// Get the name/key of the attribute
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            VersionedAttribute::V1(attr) => &attr.name,
            VersionedAttribute::V2(attr) => &attr.key,
        }
    }

    /// Get the type of the attribute
    #[must_use]
    pub fn r#type(&self) -> &AttributeType {
        match self {
            VersionedAttribute::V1(attr) => &attr.r#type,
            VersionedAttribute::V2(attr) => &attr.r#type,
        }
    }

    /// Get the deprecated field of the attribute
    #[must_use]
    pub fn deprecated(&self) -> &Option<Deprecated> {
        match self {
            VersionedAttribute::V1(attr) => &attr.deprecated,
            VersionedAttribute::V2(attr) => &attr.common.deprecated,
        }
    }

    /// Get the stability field of the attribute
    #[must_use]
    pub fn stability(&self) -> Option<&Stability> {
        match self {
            VersionedAttribute::V1(attr) => attr.stability.as_ref(),
            VersionedAttribute::V2(attr) => Some(&attr.common.stability),
        }
    }
}

/// Versioned enum for the signal
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum VersionedSignal {
    /// v1 ResolvedGroup
    Group(Box<ResolvedGroup>),
    /// v2 Signal Metric
    Metric(weaver_forge::v2::metric::Metric),
    /// v2 Signal Span
    Span(weaver_forge::v2::span::Span),
    /// v2 Signal Event
    Event(weaver_forge::v2::event::Event),
}

impl VersionedSignal {
    /// Get the deprecated field of the signal
    #[must_use]
    pub fn deprecated(&self) -> &Option<Deprecated> {
        match self {
            VersionedSignal::Group(group) => &group.as_ref().deprecated,
            VersionedSignal::Metric(metric) => &metric.common.deprecated,
            VersionedSignal::Span(span) => &span.common.deprecated,
            VersionedSignal::Event(event) => &event.common.deprecated,
        }
    }

    /// Get the stability field of the signal
    #[must_use]
    pub fn stability(&self) -> Option<&Stability> {
        match self {
            VersionedSignal::Group(group) => group.as_ref().stability.as_ref(),
            VersionedSignal::Metric(metric) => Some(&metric.common.stability),
            VersionedSignal::Span(span) => Some(&span.common.stability),
            VersionedSignal::Event(event) => Some(&event.common.stability),
        }
    }

    /// Get the instrument field of the signal, if applicable
    #[must_use]
    pub fn instrument(&self) -> Option<&InstrumentSpec> {
        match self {
            VersionedSignal::Group(group) => group.as_ref().instrument.as_ref(),
            VersionedSignal::Metric(metric) => Some(&metric.instrument),
            VersionedSignal::Span(_) => None,
            VersionedSignal::Event(_) => None,
        }
    }

    /// Get the unit field of the signal, if applicable
    #[must_use]
    pub fn unit(&self) -> Option<&String> {
        match self {
            VersionedSignal::Group(group) => group.as_ref().unit.as_ref(),
            VersionedSignal::Metric(metric) => Some(&metric.unit),
            VersionedSignal::Span(_) => None,
            VersionedSignal::Event(_) => None,
        }
    }
}

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

/// Live-check Sample root items.
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
    /// A sample log
    Log(SampleLog),
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
    /// A sample log
    Log(&'a SampleLog),
}

impl SampleRef<'_> {
    /// Returns the sample type as a string.
    #[must_use]
    pub fn sample_type(&self) -> &str {
        match self {
            SampleRef::Attribute(_) => "attribute",
            SampleRef::Span(_) => "span",
            SampleRef::SpanEvent(_) => "span_event",
            SampleRef::SpanLink(_) => "span_link",
            SampleRef::Resource(_) => "resource",
            SampleRef::Metric(_) => "metric",
            SampleRef::NumberDataPoint(_) => "number_data_point",
            SampleRef::HistogramDataPoint(_) => "histogram_data_point",
            SampleRef::ExponentialHistogramDataPoint(_) => "exponential_histogram_data_point",
            SampleRef::Exemplar(_) => "exemplar",
            SampleRef::Log(_) => "log",
        }
    }
}

impl Sample {
    /// Returns the signal type as a string or None if sample
    /// does not capture a whole signal.
    #[must_use]
    pub fn signal_type(&self) -> Option<String> {
        match self {
            Sample::Attribute(_) => None, // not a signal
            Sample::Span(_) => Some("span".to_owned()),
            Sample::SpanEvent(_) => None,
            Sample::SpanLink(_) => None,
            Sample::Resource(_) => Some("resource".to_owned()),
            Sample::Metric(_) => Some("metric".to_owned()),
            Sample::Log(_) => Some("log".to_owned()),
        }
    }

    /// Returns the signal name as a string or None if sample
    /// does not capture a whole signal.
    #[must_use]
    pub fn signal_name(&self) -> Option<String> {
        match self {
            Sample::Attribute(_) => None,                  // not a signal
            Sample::Span(span) => Some(span.name.clone()), // TODO: update to type once added
            Sample::SpanEvent(_) => None,
            Sample::SpanLink(_) => None,
            Sample::Resource(_) => None,
            Sample::Metric(metric) => Some(metric.name.clone()),
            Sample::Log(log) => Some(log.event_name.clone()),
        }
    }
}

// Dispatch the live check to the sample type
impl LiveCheckRunner for Sample {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        match self {
            Sample::Attribute(attribute) => {
                attribute.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::Span(span) => {
                span.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::SpanEvent(span_event) => {
                span_event.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::SpanLink(span_link) => {
                span_link.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::Resource(resource) => {
                resource.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::Metric(metric) => {
                metric.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
            Sample::Log(log) => {
                log.run_live_check(live_checker, stats, parent_group, parent_signal)
            }
        }
    }
}

/// Represents a live check result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LiveCheckResult {
    /// Advice on the entity
    pub all_advice: Vec<PolicyFinding>,
    /// The highest advice level
    pub highest_advice_level: Option<FindingLevel>,
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
    pub fn add_advice(&mut self, advice: PolicyFinding) {
        let advice_level = advice.level.clone();
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
    pub fn add_advice_list(&mut self, advice: Vec<PolicyFinding>) {
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

/// Samples implement this trait to run live checks on themselves
pub trait LiveCheckRunner {
    /// Run the live check
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error>;
}

// Run checks on all items in a collection that implement LiveCheckRunner
impl<T: LiveCheckRunner> LiveCheckRunner for Vec<T> {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        for item in self.iter_mut() {
            item.run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
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
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<LiveCheckResult, Error> {
        let mut result = LiveCheckResult::new();

        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(
                self.as_sample_ref(),
                parent_signal,
                None,
                parent_group.clone(),
                live_checker.otlp_emitter.clone(),
            )?;
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
