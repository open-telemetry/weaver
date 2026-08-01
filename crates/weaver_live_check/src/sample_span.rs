// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_semconv::group::SpanKindSpec;

use crate::{
    advice::{emit_findings, FindingBuilder},
    live_checker::LiveChecker,
    sample_attribute::SampleAttribute,
    sample_resource::SampleResource,
    Advisable, Error, FindingId, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample,
    SampleRef, VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, SPAN_STATUS_ADVICE_CONTEXT_KEY,
    SPAN_TYPE_ADVICE_CONTEXT_KEY,
};

/// The attribute that carries the semconv span type on a sampled span.
pub const OTEL_SPAN_TYPE: &str = "otel.span.type";

/// The attribute that describes the cause of an error.
pub const ERROR_TYPE: &str = "error.type";

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

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusCode::Unset => write!(f, "unset"),
            StatusCode::Ok => write!(f, "ok"),
            StatusCode::Error => write!(f, "error"),
        }
    }
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

impl SampleSpan {
    /// The value of the `otel.span.type` attribute, if the span carries one.
    #[must_use]
    pub fn span_type(&self) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attr| attr.name == OTEL_SPAN_TYPE)
            .and_then(|attr| attr.value.as_ref())
            .and_then(|value| value.as_str())
    }

    /// Checks the span status against the `error.type` attribute.
    ///
    /// `Ok` is reserved for application code, so instrumentation setting it is a violation,
    /// and `error.type` is only meaningful on a span whose status is `Error`.
    fn check_status(&self, parent_signal: &Sample) -> Vec<PolicyFinding> {
        let status_code = self.status.as_ref().map_or(&StatusCode::Unset, |s| &s.code);
        let mut findings = Vec::new();

        if status_code == &StatusCode::Ok {
            findings.push(
                FindingBuilder::new(FindingId::SpanStatusOk)
                    .context(json!({ SPAN_STATUS_ADVICE_CONTEXT_KEY: status_code }))
                    .message(
                        "Span status is 'ok'. Instrumentations must leave the status 'unset'; \
                         'ok' is reserved for application code.",
                    )
                    .level(FindingLevel::Violation)
                    .signal(parent_signal)
                    .build(),
            );
        }

        let has_error_type = self.attributes.iter().any(|attr| attr.name == ERROR_TYPE);
        if has_error_type && status_code != &StatusCode::Error {
            findings.push(
                FindingBuilder::new(FindingId::ErrorTypeWithoutErrorStatus)
                    .context(json!({
                        ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: ERROR_TYPE,
                        SPAN_STATUS_ADVICE_CONTEXT_KEY: status_code,
                    }))
                    .message(format!(
                        "Attribute '{ERROR_TYPE}' is set, but the span status is '{status_code}' \
                         instead of 'error'."
                    ))
                    .level(FindingLevel::Violation)
                    .signal(parent_signal)
                    .build(),
            );
        }

        findings
    }
}

impl LiveCheckRunner for SampleSpan {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        // Spans are resolved against the registry by their `otel.span.type`, not by name.
        // Spans without the attribute are not checked against a definition.
        let span_type = self.span_type().map(str::to_owned);
        let semconv_span = span_type
            .as_deref()
            .and_then(|span_type| live_checker.find_span(span_type));

        // Status checks do not need a definition — they apply to every span.
        let mut findings = self.check_status(parent_signal);
        if let (Some(span_type), None) = (&span_type, &semconv_span) {
            findings.push(
                FindingBuilder::new(FindingId::MissingSpan)
                    .context(json!({ SPAN_TYPE_ADVICE_CONTEXT_KEY: span_type }))
                    .message(format!(
                        "Span type '{span_type}' does not exist in the registry."
                    ))
                    .level(FindingLevel::Violation)
                    .signal(parent_signal)
                    .build(),
            );
        }
        emit_findings(
            &findings,
            &SampleRef::Span(self),
            live_checker.otlp_emitter.as_deref(),
            parent_signal,
        );

        let parent_group = semconv_span;
        let mut result =
            self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?;
        let sample_ref = SampleRef::Span(self);
        result.add_advice_list(
            findings,
            live_checker.finding_modifier.as_ref(),
            &sample_ref,
        );
        self.live_check_result = Some(result);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        self.span_events.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            parent_signal,
        )?;
        self.span_links
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
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
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
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
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        Ok(())
    }
}
