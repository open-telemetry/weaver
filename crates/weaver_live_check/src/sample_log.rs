// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample logd

use std::rc::Rc;

use cel::{Context, SerializationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_checker::FindingLevel;

use crate::{
    advice::{add_entity_association_findings, FindingBuilder},
    cel::{attribute_map, bind_signal_context, Matchable},
    live_checker::LiveChecker,
    matcher::SampleMatch,
    sample_attribute::SampleAttribute,
    sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource,
    Error, FindingId, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
    SampleType, VersionedSignal,
};

/// Represents a sample telemetry log parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleLog {
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
    /// Shared instrumentation scope that produced this log record (not serialized).
    #[serde(skip)]
    pub instrumentation_scope: Option<Rc<SampleInstrumentationScope>>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
    /// Reference to the parent resource (not serialized)
    #[serde(skip)]
    pub resource: Option<Rc<SampleResource>>,
    /// Event timestamp from the OTLP log record in RFC3339 format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl LiveCheckRunner for SampleLog {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        // A log with no event_name has no name to match on.
        let natural = if self.event_name.is_empty() {
            None
        } else {
            live_checker.find_event(&self.event_name)
        };
        // The match runs before the advisors, so they check the attributes
        // against the matcher's `signal`.
        let sample_match = live_checker.match_for(self, natural);
        live_checker.record_match(&sample_match);
        let semconv_event = sample_match.signal.clone();
        // Coverage counts against the signal the match resolved, which a
        // matcher can change. A v1 group is the exception: its id is not the
        // event name.
        let coverage_name = match semconv_event.as_deref() {
            Some(VersionedSignal::Event(event)) => event.name.to_string(),
            _ => self.event_name.clone(),
        };
        // Raised only when no matcher named a signal.
        if semconv_event.is_none() && !self.event_name.is_empty() {
            let finding = FindingBuilder::new(FindingId::MissingEvent)
                .message(format!(
                    "Event '{}' does not exist in the registry.",
                    self.event_name
                ))
                .level(FindingLevel::Violation)
                .signal(parent_signal)
                .build_and_emit(
                    &SampleRef::Log(self),
                    live_checker.otlp_emitter.as_ref().map(|rc| rc.as_ref()),
                    parent_signal,
                );
            let sample_ref = SampleRef::Log(self);
            result.add_advice(finding, live_checker.finding_modifier.as_ref(), &sample_ref);
        }
        for advisor in live_checker.advisors.iter_mut() {
            let sample_ref = SampleRef::Log(self);
            let advice_list = advisor.advise(
                sample_ref.clone(),
                parent_signal,
                None,
                semconv_event.clone(),
                live_checker.otlp_emitter.clone(),
            )?;
            result.add_advice_list(
                advice_list,
                live_checker.finding_modifier.as_ref(),
                &sample_ref,
            );
        }
        add_entity_association_findings(
            semconv_event.as_deref(),
            &SampleRef::Log(self),
            &mut result,
            live_checker,
            parent_signal,
        );

        sample_match.add_findings(
            &SampleRef::Log(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        let sample_match = Rc::new(sample_match);

        // Check attributes
        self.attributes
            .run_live_check(live_checker, stats, Some(sample_match), parent_signal)?;

        self.live_check_result = Some(result);
        stats.inc_entity_count("log");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        stats.add_event_name_to_coverage(coverage_name);
        Ok(())
    }
}

impl Matchable for SampleLog {
    fn sample_type(&self) -> SampleType {
        SampleType::Log
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("event_name", &self.event_name)?;
        context.add_variable("severity_text", &self.severity_text)?;
        context.add_variable("severity_number", self.severity_number)?;
        context.add_variable("body", &self.body)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cel::evaluate;

    fn parse(json: &str) -> SampleLog {
        serde_json::from_str(json).expect("the fixture parses")
    }

    #[test]
    fn a_log_binds_its_fields() {
        let log = parse(include_str!(
            "../fixtures/cel/log-common/log-order-placed.json"
        ));
        let when = r#"event_name == "myapp.order.placed" && severity_number == 9
            && severity_text == "INFO" && body.contains("order")
            && attributes["myapp.tenant.code"] == "acme-eu""#;
        assert!(evaluate(when, &log).expect("it evaluates"));
    }

    /// The docs promise this: an optional field the record omits is null, so
    /// an unguarded read errors and a `!= null` guard prevents it.
    #[test]
    fn an_absent_optional_field_is_null() {
        let log = parse(
            r#"{ "event_name": "myapp.order.placed", "attributes": [], "live_check_result": null }"#,
        );
        let when = "severity_number == null && severity_text == null && body == null";
        assert!(evaluate(when, &log).expect("it evaluates"));
        let _ = evaluate(r#"body.contains("x")"#, &log).expect_err("it errors");
        let when = r#"(body != null && body.contains("x"))
            || (severity_number != null && severity_number < 10)"#;
        assert!(!evaluate(when, &log).expect("it evaluates"));
    }
}
