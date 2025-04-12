// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_checker::violation::{Advice, AdviceLevel};

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, LiveCheckResult, LiveCheckRunner,
    LiveCheckStatistics,
};

/// Represents a sample telemetry span parsed from any source
/// The contained attributes, span_events, and span_links are not serialized to avoid
/// duplication in the live check results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpan {
    /// The name of the span
    pub name: String,
    /// The kind of the span
    pub kind: String,
    /// The span's attributes
    pub attributes: Vec<SampleAttribute>,
    /// SpanEvents
    pub span_events: Vec<SampleSpanEvent>,
    /// SpanLinks
    pub span_links: Vec<SampleSpanLink>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleSpan {
    fn run_live_check(&mut self, live_checker: &mut LiveChecker, stats: &mut LiveCheckStatistics) {
        let mut result = LiveCheckResult::new();
        // TODO Remove this:
        let span_advice = Advice {
            advice_type: "span_info".to_owned(),
            value: Value::String(self.name.clone()),
            message: format!("Has span kind: `{}`", self.kind),
            advice_level: AdviceLevel::Information,
        };
        result.add_advice(span_advice);

        for advisor in live_checker.advisors.iter_mut() {
            if let Ok(advice_list) = advisor.advise_on_span(self, None) {
                result.add_advice_list(advice_list);
            }
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats);
        }
        for span_event in &mut self.span_events {
            span_event.run_live_check(live_checker, stats);
        }
        for span_link in &mut self.span_links {
            span_link.run_live_check(live_checker, stats);
        }
        self.live_check_result = Some(result);
    }
}

/// Represents a span event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanEvent {
    /// The name of the event
    pub name: String,
    /// The attributes of the event
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleSpanEvent {
    fn run_live_check(&mut self, live_checker: &mut LiveChecker, stats: &mut LiveCheckStatistics) {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            if let Ok(advice_list) = advisor.advise_on_span_event(self, None) {
                result.add_advice_list(advice_list);
            }
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats);
        }
        self.live_check_result = Some(result);
    }
}

/// Represents a span link
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanLink {
    /// The attributes of the link
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleSpanLink {
    fn run_live_check(&mut self, live_checker: &mut LiveChecker, stats: &mut LiveCheckStatistics) {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            if let Ok(advice_list) = advisor.advise_on_span_link(self, None) {
                result.add_advice_list(advice_list);
            }
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats);
        }
        self.live_check_result = Some(result);
    }
}
