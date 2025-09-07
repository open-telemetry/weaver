// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_checker::violation::AdviceLevel;
use weaver_forge::registry::ResolvedGroup;
use weaver_semconv::group::SpanKindSpec;

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Advisable, Error,
    LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, SampleRef,
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
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
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
        parent_group: Option<Rc<ResolvedGroup>>,
        advice_level: Option<AdviceLevel>,
    ) -> Result<(), Error> {
        self.live_check_result = Some(self.run_advisors(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?);
        self.attributes.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?;
        self.span_events.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?;
        self.span_links.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?;
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
        parent_group: Option<Rc<ResolvedGroup>>,
        advice_level: Option<AdviceLevel>,
    ) -> Result<(), Error> {
        self.live_check_result = Some(self.run_advisors(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?);
        self.attributes.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?;
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
        parent_group: Option<Rc<ResolvedGroup>>,
        advice_level: Option<AdviceLevel>,
    ) -> Result<(), Error> {
        self.live_check_result = Some(self.run_advisors(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?);
        self.attributes.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            advice_level.clone(),
        )?;
        Ok(())
    }
}
