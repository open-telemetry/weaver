// SPDX-License-Identifier: Apache-2.0

//! This crate provides the weaver_live_check library

use std::rc::Rc;

use finding_modifier::FindingModifier;
use live_checker::LiveChecker;
use miette::Diagnostic;
use sample_attribute::SampleAttribute;
use sample_instrumentation_scope::SampleInstrumentationScope;
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
    v2::{
        attribute::Attribute as ForgeAttribute,
        registry::{ForgeResolvedRegistry, Registry},
    },
};
use weaver_semconv::{
    attribute::AttributeType, deprecated::Deprecated, group::InstrumentSpec, stability::Stability,
};

/// Advisors for live checks
pub mod advice;
/// Finding modifier engine (overrides and filters).
pub mod finding_modifier;
/// Generated types, constants, and log record builders for live check findings
pub mod generated;
pub use generated::attributes::{FindingId, SampleType, SignalType};
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
/// An intermediary format for instrumentation scope metadata.
pub mod sample_instrumentation_scope;
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

/// Attribute key in advice context
pub const ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: &str = "attribute_key";
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
/// Entity type key in advice context
pub const ENTITY_TYPE_ADVICE_CONTEXT_KEY: &str = "entity_type";

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
    V1(Box<ResolvedRegistry>),
    /// v2 ForgeResolvedRegistry
    V2(Box<ForgeResolvedRegistry>),
}

/// Where the definition used to check a sample attribute came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeSource {
    /// No definition was found for the attribute.
    NotFound,
    /// The matched signal, or the registry under check.
    Registry,
    /// Only a dependency of the registry under check defines the attribute.
    Dependency,
}

/// A registry and its dependency closure, nearest first.
///
/// Depth-first: the registry, then each dependency, then that dependency's own
/// dependencies.
#[must_use]
pub fn registries_nearest_first(registry: &ForgeResolvedRegistry) -> Vec<&ForgeResolvedRegistry> {
    fn collect<'a>(registry: &'a ForgeResolvedRegistry, out: &mut Vec<&'a ForgeResolvedRegistry>) {
        out.push(registry);
        for dependency in &registry.dependencies {
            collect(dependency, out);
        }
    }

    let mut out = Vec::new();
    collect(registry, &mut out);
    out
}

/// The attributes of every attribute group of the registry.
///
/// A group is in a resolved registry only when it is public. An internal group
/// exists to compose a signal, and its attributes reach a sample through that
/// signal alone.
pub fn attribute_group_attributes(registry: &Registry) -> impl Iterator<Item = &ForgeAttribute> {
    registry
        .attribute_groups
        .iter()
        .flat_map(|group| group.attributes.iter().map(|attribute| &attribute.base))
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

    /// Get the definition this signal declares for the attribute `key`,
    /// including any refinement the signal applies for itself. Step 1 of the
    /// lookup; see `docs/attribute_lookup.md`.
    #[must_use]
    pub fn find_attribute(&self, key: &str) -> Option<VersionedAttribute> {
        match self {
            VersionedSignal::Group(group) => group
                .attributes
                .iter()
                .find(|attribute| attribute.name == key)
                .map(|attribute| VersionedAttribute::V1(attribute.clone())),
            VersionedSignal::Metric(metric) => find_v2_attribute(
                metric.attributes.iter().map(|attribute| &attribute.base),
                key,
            ),
            VersionedSignal::Span(span) => {
                find_v2_attribute(span.attributes.iter().map(|attribute| &attribute.base), key)
            }
            VersionedSignal::Event(event) => find_v2_attribute(
                event.attributes.iter().map(|attribute| &attribute.base),
                key,
            ),
        }
    }

    /// Get the template definition this signal declares that `key` is an
    /// instance of, longest first. Step 2 of the lookup, and the templated
    /// counterpart of [`Self::find_attribute`].
    #[must_use]
    pub fn find_template(&self, key: &str) -> Option<VersionedAttribute> {
        match self {
            VersionedSignal::Group(group) => group
                .attributes
                .iter()
                .filter(|attribute| is_template_for(&attribute.r#type, &attribute.name, key))
                .max_by_key(|attribute| attribute.name.len())
                .map(|attribute| VersionedAttribute::V1(attribute.clone())),
            VersionedSignal::Metric(metric) => find_v2_template(
                metric.attributes.iter().map(|attribute| &attribute.base),
                key,
            ),
            VersionedSignal::Span(span) => {
                find_v2_template(span.attributes.iter().map(|attribute| &attribute.base), key)
            }
            VersionedSignal::Event(event) => find_v2_template(
                event.attributes.iter().map(|attribute| &attribute.base),
                key,
            ),
        }
    }
}

/// Whether `key` is an instance of the template declared as `template_key`.
fn is_template_for(attribute_type: &AttributeType, template_key: &str, key: &str) -> bool {
    matches!(attribute_type, AttributeType::Template(_)) && key.starts_with(template_key)
}

/// The declared v2 attribute whose key is exactly `key`.
fn find_v2_attribute<'a>(
    mut attributes: impl Iterator<Item = &'a ForgeAttribute>,
    key: &str,
) -> Option<VersionedAttribute> {
    attributes
        .find(|attribute| attribute.key == key)
        .map(|attribute| VersionedAttribute::V2(attribute.clone()))
}

/// The longest declared v2 template that `key` is an instance of.
fn find_v2_template<'a>(
    attributes: impl Iterator<Item = &'a ForgeAttribute>,
    key: &str,
) -> Option<VersionedAttribute> {
    attributes
        .filter(|attribute| is_template_for(&attribute.r#type, &attribute.key, key))
        .max_by_key(|attribute| attribute.key.len())
        .map(|attribute| VersionedAttribute::V2(attribute.clone()))
}

/// Versioned enum for an entity definition
#[derive(Debug, Clone, PartialEq)]
pub enum VersionedEntity {
    /// v1 entity — a ResolvedGroup with GroupType::Entity
    V1(Box<ResolvedGroup>),
    /// v2 entity
    V2(Box<weaver_forge::v2::entity::Entity>),
}

/// Weaver live check errors
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Configuration error.
    #[error("Configuration error. {error}")]
    ConfigError {
        /// The error that occurred.
        error: String,
    },

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
    /// An instrumentation scope that produced telemetry signals
    InstrumentationScope(SampleInstrumentationScope),
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
    /// An instrumentation scope that produced telemetry signals
    InstrumentationScope(&'a SampleInstrumentationScope),
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
    /// Returns the sample name, if available for this sample type.
    ///
    /// For attributes this is the attribute key, for instrumentation scopes
    /// it is the scope name, and for spans/metrics/events it is the signal
    /// name. Sub-signal types (data points, exemplars, span links, resources)
    /// do not carry a name.
    #[must_use]
    pub fn sample_name(&self) -> Option<&str> {
        match self {
            SampleRef::Attribute(attr) => Some(&attr.name),
            SampleRef::Span(span) => Some(&span.name),
            SampleRef::SpanEvent(event) => Some(&event.name),
            SampleRef::InstrumentationScope(scope) => Some(&scope.name),
            SampleRef::Metric(metric) => Some(&metric.name),
            SampleRef::Log(log) => Some(&log.event_name),
            _ => None,
        }
    }

    /// Returns the sample type.
    #[must_use]
    pub fn sample_type(&self) -> SampleType {
        match self {
            SampleRef::Attribute(_) => SampleType::Attribute,
            SampleRef::Span(_) => SampleType::Span,
            SampleRef::SpanEvent(_) => SampleType::SpanEvent,
            SampleRef::SpanLink(_) => SampleType::SpanLink,
            SampleRef::Resource(_) => SampleType::Resource,
            SampleRef::InstrumentationScope(_) => SampleType::InstrumentationScope,
            SampleRef::Metric(_) => SampleType::Metric,
            SampleRef::NumberDataPoint(_) => SampleType::NumberDataPoint,
            SampleRef::HistogramDataPoint(_) => SampleType::HistogramDataPoint,
            SampleRef::ExponentialHistogramDataPoint(_) => {
                SampleType::ExponentialHistogramDataPoint
            }
            SampleRef::Exemplar(_) => SampleType::Exemplar,
            SampleRef::Log(_) => SampleType::Log,
        }
    }
}

impl Sample {
    /// Returns the signal type or None if sample
    /// does not capture a whole signal.
    #[must_use]
    pub fn signal_type(&self) -> Option<String> {
        match self {
            Sample::Attribute(_) => None, // not a signal
            Sample::Span(_) => Some(SignalType::Span.to_string()),
            Sample::SpanEvent(_) => None,
            Sample::SpanLink(_) => None,
            Sample::Resource(_) => Some(SignalType::Resource.to_string()),
            Sample::InstrumentationScope(_) => None,
            Sample::Metric(_) => Some(SignalType::Metric.to_string()),
            Sample::Log(_) => Some(SignalType::Log.to_string()),
        }
    }

    /// Returns a reference to the parent resource, if available.
    #[must_use]
    pub fn resource(&self) -> Option<&SampleResource> {
        match self {
            Sample::Span(s) => s.resource.as_deref(),
            Sample::Metric(m) => m.resource.as_deref(),
            Sample::Log(l) => l.resource.as_deref(),
            _ => None,
        }
    }

    /// Returns the instrumentation scope that produced the signal, if available.
    #[must_use]
    pub fn instrumentation_scope(&self) -> Option<&SampleInstrumentationScope> {
        match self {
            Sample::Span(s) => s.instrumentation_scope.as_deref(),
            Sample::Metric(m) => m.instrumentation_scope.as_deref(),
            Sample::Log(l) => l.instrumentation_scope.as_deref(),
            _ => None,
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
            Sample::InstrumentationScope(_) => None,
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
            Sample::InstrumentationScope(scope) => {
                scope.run_live_check(live_checker, stats, parent_group, parent_signal)
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

    /// Add an advice to the result and update the highest advice level.
    ///
    /// When a `FindingModifier` is provided, the finding may be dropped
    /// (filter exclusion) before being stored.
    ///
    /// `sample` is the sample that produced this finding, used by
    /// `exclude_samples` filters to inspect and match on it.
    pub fn add_advice(
        &mut self,
        advice: PolicyFinding,
        modifier: Option<&FindingModifier>,
        sample: &SampleRef<'_>,
    ) {
        let advice = if let Some(modifier) = modifier {
            match modifier.apply(advice, sample) {
                Some(kept) => kept,
                None => return, // Excluded by filter
            }
        } else {
            advice
        };
        let level = advice.level;
        self.highest_advice_level = Some(
            self.highest_advice_level
                .map_or(level, |prev| prev.max(level)),
        );
        self.all_advice.push(advice);
    }

    /// Add a list of advice to the result and update the highest advice level.
    ///
    /// When a `FindingModifier` is provided, each finding may be dropped
    /// (filter exclusion) before being stored.
    pub fn add_advice_list(
        &mut self,
        advice: Vec<PolicyFinding>,
        modifier: Option<&FindingModifier>,
        sample: &SampleRef<'_>,
    ) {
        for advice in advice {
            self.add_advice(advice, modifier, sample);
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
            result.add_advice_list(
                advice_list,
                live_checker.finding_modifier.as_ref(),
                &self.as_sample_ref(),
            );
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
