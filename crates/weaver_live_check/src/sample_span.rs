// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};
use weaver_semconv::group::SpanKindSpec;

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Error, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, SampleRef,
};

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpan {
    /// The name of the span
    pub name: String,
    /// The kind of the span
    pub kind: SpanKindSpec,
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

impl LiveCheckRunner for SampleSpan {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(SampleRef::Span(self), None, None)?;
            result.add_advice_list(advice_list);
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats)?;
        }
        for span_event in &mut self.span_events {
            span_event.run_live_check(live_checker, stats)?;
        }
        for span_link in &mut self.span_links {
            span_link.run_live_check(live_checker, stats)?;
        }
        self.live_check_result = Some(result);
        stats.inc_entity_count("span");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        Ok(())
    }
}

/// Represents a span event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanEvent {
    /// The name of the event
    pub name: String,
    /// The attributes of the event
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleSpanEvent {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(SampleRef::SpanEvent(self), None, None)?;
            result.add_advice_list(advice_list);
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats)?;
        }
        self.live_check_result = Some(result);
        stats.inc_entity_count("span_event");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        Ok(())
    }
}

/// Represents a span link
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanLink {
    /// The attributes of the link
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleSpanLink {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(SampleRef::SpanLink(self), None, None)?;
            result.add_advice_list(advice_list);
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats)?;
        }
        self.live_check_result = Some(result);
        stats.inc_entity_count("span_link");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        Ok(())
    }
}
