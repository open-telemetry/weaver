// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};

use crate::{sample_attribute::SampleAttribute, LiveCheckResult};

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

/// Represents a span link
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanLink {
    /// The attributes of the link
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}
