// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::group::SpanKindSpec;

use crate::{
    advice::add_entity_association_findings, live_checker::LiveChecker, matcher::SampleMatch,
    sample_attribute::SampleAttribute, sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource, Advisable, Error, LiveCheckResult, LiveCheckRunner,
    LiveCheckStatistics, Sample, SampleRef, SampleType,
};

/// The status code of the span
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatusCode {
    /// The status is unset
    Unset,
    /// The status is ok
    Ok,
    /// The status is error
    Error,
}

/// The status code and message of the span
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    /// The status code
    pub code: StatusCode,
    /// The status message
    pub message: String,
}

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleSpan {
    /// The name of the span
    pub name: String,
    /// The kind of the span
    pub kind: SpanKindSpec,
    /// Status
    pub status: Option<Status>,
    /// The span's attributes
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// SpanEvents
    #[serde(default)]
    pub span_events: Vec<SampleSpanEvent>,
    /// SpanLinks
    #[serde(default)]
    pub span_links: Vec<SampleSpanLink>,
    /// Shared instrumentation scope that produced this span (not serialized).
    #[serde(skip)]
    pub instrumentation_scope: Option<Rc<SampleInstrumentationScope>>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
    /// Reference to the parent resource (not serialized)
    #[serde(skip)]
    pub resource: Option<Rc<SampleResource>>,
}

impl Advisable for SampleSpan {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::Span(self)
    }

    fn entity_type(&self) -> &str {
        "span"
    }
}

impl LiveCheckRunner for SampleSpan {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        // Nothing nests a span, so it always matches on its own.
        let sample_match = Rc::new(live_checker.match_for(SampleType::Span, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        add_entity_association_findings(
            sample_match.signal.as_deref(),
            &SampleRef::Span(self),
            &mut result,
            live_checker,
            parent_signal,
        );
        sample_match.add_findings(
            &SampleRef::Span(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        self.live_check_result = Some(result);
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        self.attributes.run_live_check(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        // A span event and a span link match on their own, so the span's match
        // is not passed down. Only the span holds the resource and scope they
        // read.
        let resource = self.resource.clone();
        let instrumentation_scope = self.instrumentation_scope.clone();
        for span_event in &mut self.span_events {
            span_event.resource.clone_from(&resource);
            span_event
                .instrumentation_scope
                .clone_from(&instrumentation_scope);
        }
        for span_link in &mut self.span_links {
            span_link.resource.clone_from(&resource);
            span_link
                .instrumentation_scope
                .clone_from(&instrumentation_scope);
        }
        self.span_events
            .run_live_check(live_checker, stats, None, parent_signal)?;
        self.span_links
            .run_live_check(live_checker, stats, None, parent_signal)?;
        Ok(())
    }
}

/// Represents a span event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleSpanEvent {
    /// The name of the event
    pub name: String,
    /// The attributes of the event
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
    /// Resource of the span this event belongs to (not serialized)
    #[serde(skip)]
    pub resource: Option<Rc<SampleResource>>,
    /// Instrumentation scope of the span this event belongs to (not serialized)
    #[serde(skip)]
    pub instrumentation_scope: Option<Rc<SampleInstrumentationScope>>,
}

impl Advisable for SampleSpanEvent {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::SpanEvent(self)
    }

    fn entity_type(&self) -> &str {
        "span_event"
    }
}

impl LiveCheckRunner for SampleSpanEvent {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match = Rc::new(live_checker.match_for(SampleType::SpanEvent, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::SpanEvent(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        self.live_check_result = Some(result);
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        self.attributes
            .run_live_check(live_checker, stats, Some(sample_match), parent_signal)
    }
}

/// Represents a span link
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleSpanLink {
    /// The attributes of the link
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
    /// Resource of the span this link belongs to (not serialized)
    #[serde(skip)]
    pub resource: Option<Rc<SampleResource>>,
    /// Instrumentation scope of the span this link belongs to (not serialized)
    #[serde(skip)]
    pub instrumentation_scope: Option<Rc<SampleInstrumentationScope>>,
}

impl Advisable for SampleSpanLink {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::SpanLink(self)
    }

    fn entity_type(&self) -> &str {
        "span_link"
    }
}

impl LiveCheckRunner for SampleSpanLink {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match = Rc::new(live_checker.match_for(SampleType::SpanLink, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::SpanLink(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        self.live_check_result = Some(result);
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        self.attributes
            .run_live_check(live_checker, stats, Some(sample_match), parent_signal)
    }
}
