// SPDX-License-Identifier: Apache-2.0

//! Raw OTLP context (identity, timing, and provenance) captured onto a
//! sample when live-check is run with `--capture-telemetry`. Every field is
//! optional and every consumer of this type leaves it `None` unless that
//! flag is set, so default output is unaffected.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    sample_instrumentation_scope::SampleInstrumentationScope, sample_resource::SampleResource,
};

/// Raw OTLP context for one sample. Which fields are populated depends on
/// the signal: a span fills identity, causality, and timing; a span event
/// fills only its own timestamp; a span link fills only the identity of the
/// span it points to. See each `context` field's own doc comment for what
/// it carries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleContext {
    /// The trace this sample belongs to, as a lowercase hex string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// This sample's own span ID (or, on a span link, the linked span's ID),
    /// as a lowercase hex string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// The parent span's ID, as a lowercase hex string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    /// The span's W3C tracestate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_state: Option<String>,
    /// When this sample started, or when it was recorded for a sample with
    /// only one timestamp (a span event, a log record). RFC3339, matching
    /// `SampleExemplar::timestamp`'s existing convention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    /// When this sample ended. RFC3339. Absent for samples with only one
    /// timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    /// The resource this sample's Resource/ScopeSpans (or ScopeLogs/
    /// ScopeMetrics) container carried, denormalized onto the sample itself
    /// so a consumer never has to correlate by list order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<SampleResource>,
    /// The instrumentation scope that produced this sample, denormalized
    /// the same way as `resource`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrumentation_scope: Option<SampleInstrumentationScope>,
}
