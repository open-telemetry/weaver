// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample events

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_checker::{FindingLevel, PolicyFinding};

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Error, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef, VersionedSignal,
    MISSING_EVENT_ADVICE_TYPE,
};

/// Represents a sample telemetry event parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleEvent {
    /// The name of the event
    pub event_name: String,
    /// Severity number (1-24)
    pub severity_number: Option<i32>,
    /// Severity text (e.g., "INFO", "ERROR")
    pub severity_text: Option<String>,
    /// Body of the event (from the log record body)
    pub body: Option<String>,
    /// The event's attributes
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Trace ID if the event is correlated with a trace
    pub trace_id: Option<String>,
    /// Span ID if the event is correlated with a span
    pub span_id: Option<String>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleEvent {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        let semconv_event = if self.event_name.is_empty() {
            // We allow Events wihout event_names to be checked, but they cannot be matched to the registry
            None
        } else {
            // find the event in the registry
            let semconv_event = live_checker.find_event(&self.event_name);
            if semconv_event.is_none() {
                result.add_advice(PolicyFinding {
                    id: MISSING_EVENT_ADVICE_TYPE.to_owned(),
                    context: Value::Null,
                    message: format!(
                        "Event '{}' does not exist in the registry.",
                        self.event_name
                    ),
                    level: FindingLevel::Violation,
                    signal_type: Some("event".to_owned()),
                    signal_name: Some(self.event_name.clone()),
                });
            };
            semconv_event
        };
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(
                SampleRef::Event(self),
                parent_signal,
                None,
                semconv_event.clone(),
            )?;
            result.add_advice_list(advice_list);
        }
        // Check attributes
        self.attributes.run_live_check(
            live_checker,
            stats,
            semconv_event.clone(),
            parent_signal,
        )?;

        self.live_check_result = Some(result);
        stats.inc_entity_count("event");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        stats.add_event_name_to_coverage(self.event_name.clone());
        Ok(())
    }
}
