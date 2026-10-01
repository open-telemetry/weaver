// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use cel::{Context, SerializationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::v1::group::SpanKindSpec;

use crate::{
    advice::add_entity_association_findings,
    cel::{attribute_map, bind_signal_context, Matchable},
    live_checker::LiveChecker,
    matcher::SampleMatch,
    sample_attribute::SampleAttribute,
    sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource,
    Advisable, Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
    SampleType, VersionedSignal,
};

/// The status code of the span
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatusCode {
    /// The status is unset
    #[default]
    Unset,
    /// The status is ok
    Ok,
    /// The status is error
    Error,
}

/// The status code and message of the span
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// Trace ID from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Span ID from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// Parent span ID from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    /// W3C tracestate from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_state: Option<String>,
    /// Start timestamp from the OTLP span in RFC3339 format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    /// End timestamp from the OTLP span in RFC3339 format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
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
        // A span has no parent sample, so it always matches on its own.
        let sample_match = Rc::new(live_checker.match_for(self, None));
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
        // A span event and a span link match on their own, so they do not get
        // the span's match. They do get its resource and scope, which only the
        // span holds.
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
    /// Event timestamp from the OTLP span in RFC3339 format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
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
        let sample_match = Rc::new(live_checker.match_for(self, None));
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
        // Unlike a log's `event_name`, a span event's name does not identify an
        // event type, so it has no natural match. It counts toward the coverage
        // of the event a matcher resolved it to, and toward nothing otherwise.
        if let Some(VersionedSignal::Event(event)) = sample_match.signal.as_deref() {
            stats.add_event_name_to_coverage(event.name.to_string());
        }
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
    /// Linked trace ID from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Linked span ID from the OTLP span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
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
        let sample_match = Rc::new(live_checker.match_for(self, None));
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

impl Matchable for SampleSpan {
    fn sample_type(&self) -> SampleType {
        SampleType::Span
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("kind", &self.kind)?;
        // OTLP treats a missing status as unset.
        context.add_variable("status", self.status.clone().unwrap_or_default())?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleSpanEvent {
    fn sample_type(&self) -> SampleType {
        SampleType::SpanEvent
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleSpanLink {
    fn sample_type(&self) -> SampleType {
        SampleType::SpanLink
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
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

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).expect("the fixture parses")
    }

    #[test]
    fn a_span_binds_its_name_kind_status_and_attributes() {
        let span: SampleSpan = parse(include_str!("../fixtures/cel/span-status/span-error.json"));
        let when = r#"name == "checkout payment" && kind == "internal"
            && status.code == "error" && status.message.contains("declined")
            && attributes["error.type"] == "card_declined""#;
        assert!(evaluate(when, &span).expect("it evaluates"));
    }

    #[test]
    fn a_span_with_no_status_is_unset() {
        let span: SampleSpan = parse(include_str!(
            "../fixtures/cel/span-status/span-no-status.json"
        ));
        let when = r#"status.code == "unset" && status.message == """#;
        assert!(evaluate(when, &span).expect("it evaluates"));
    }

    #[test]
    fn a_span_event_binds_its_name_and_attributes() {
        let event = SampleSpanEvent {
            name: "exception".to_owned(),
            attributes: vec![
                SampleAttribute::try_from("exception.type=Timeout").expect("it parses")
            ],
            live_check_result: None,
            timestamp: None,
            resource: None,
            instrumentation_scope: None,
        };
        let when = r#"name == "exception" && attributes["exception.type"] == "Timeout""#;
        assert!(evaluate(when, &event).expect("it evaluates"));
    }

    #[test]
    fn a_span_link_binds_its_attributes() {
        let link = SampleSpanLink {
            attributes: vec![
                SampleAttribute::try_from("myapp.link.kind=parent").expect("it parses")
            ],
            live_check_result: None,
            trace_id: None,
            span_id: None,
            resource: None,
            instrumentation_scope: None,
        };
        let when = r#"attributes["myapp.link.kind"] == "parent""#;
        assert!(evaluate(when, &link).expect("it evaluates"));
    }

    /// Live checks the fixture checkout span carrying one span event named
    /// `event_name`, under `matchers`, and returns the statistics.
    fn event_coverage(matchers: &str, event_name: &str) -> crate::CumulativeStatistics {
        use crate::matcher::fixture::{matcher_configs, v2_live_checker};
        use crate::CumulativeStatistics;

        let mut live_checker = v2_live_checker();
        live_checker
            .set_matchers(&matcher_configs(matchers))
            .expect("they check out");

        let mut span: SampleSpan = parse(include_str!(
            "../fixtures/cel/span-checkout/span-checkout-payment.json"
        ));
        span.span_events.push(SampleSpanEvent {
            name: event_name.to_owned(),
            attributes: vec![],
            live_check_result: None,
            timestamp: None,
            resource: None,
            instrumentation_scope: None,
        });

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        let parent = Sample::Span(span.clone());
        span.run_live_check(&mut live_checker, &mut stats, None, &parent)
            .expect("the check runs");
        let LiveCheckStatistics::Cumulative(stats) = stats else {
            panic!("cumulative statistics");
        };
        stats
    }

    /// A span event that a matcher pairs with a registry event counts toward
    /// that event's coverage, as a log does, and its own name is not reported
    /// as an unknown event.
    #[test]
    fn a_matched_span_event_counts_toward_the_event_coverage() {
        let stats = event_coverage(
            r#"
[[live-check.matchers]]
id = "myapp.order.step"
sample_type = "span_event"
when = 'name == "myapp.order.step"'
signal = "myapp.order.placed"
"#,
            "myapp.order.step",
        );
        assert_eq!(
            stats.seen_registry_events.get("myapp.order.placed"),
            Some(&1),
            "the matched event takes the coverage: {:?}",
            stats.seen_registry_events
        );
        assert!(
            !stats
                .seen_non_registry_events
                .contains_key("myapp.order.step"),
            "the span event resolved a registry event: {:?}",
            stats.seen_non_registry_events
        );
    }

    /// A span event's name does not identify an event type, as a log's
    /// `event_name` does, so it has no natural match. Without a matcher it
    /// counts toward no event, even one that shares its name.
    #[test]
    fn an_unmatched_span_event_counts_toward_no_event() {
        let stats = event_coverage(
            r#"
[[live-check.matchers]]
id = "myapp.never"
sample_type = "span_event"
when = 'name == "no-such-event"'
"#,
            "myapp.order.placed",
        );
        assert_eq!(
            stats.seen_registry_events.get("myapp.order.placed"),
            Some(&0),
            "an unmatched span event was not checked against the event: {:?}",
            stats.seen_registry_events
        );
        assert!(
            stats.seen_non_registry_events.is_empty(),
            "an unmatched span event is not an unknown event either: {:?}",
            stats.seen_non_registry_events
        );
    }
}
