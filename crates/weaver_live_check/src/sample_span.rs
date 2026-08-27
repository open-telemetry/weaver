// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::group::SpanKindSpec;

use crate::{
    live_checker::LiveChecker, matcher::SampleMatch, sample_attribute::SampleAttribute,
    sample_instrumentation_scope::SampleInstrumentationScope, sample_resource::SampleResource,
    Advisable, Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
    SampleType,
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
        parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match = live_checker.match_for(SampleType::Span, self, None);
        live_checker.record_match(&sample_match);
        // A matched span replaces the match passed down.
        let sample_match = if sample_match.signal.is_some() || !sample_match.applied.is_empty() {
            Rc::new(sample_match)
        } else {
            parent.unwrap_or_else(|| Rc::new(sample_match))
        };
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::Span(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        self.live_check_result = Some(result);
        self.attributes.run_live_check(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        self.span_events.run_live_check(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        self.span_links
            .run_live_check(live_checker, stats, Some(sample_match), parent_signal)?;
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
        parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent.clone(), parent_signal)?;
        Ok(())
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
        parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent.clone(), parent_signal)?;
        Ok(())
    }
}
