// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};

use crate::sample_attribute::SampleAttribute;

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
    #[serde(skip_serializing)]
    pub attributes: Vec<SampleAttribute>,
    /// SpanEvents
    #[serde(skip_serializing)]
    pub span_events: Vec<SampleSpanEvent>,
    /// SpanLinks
    #[serde(skip_serializing)]
    pub span_links: Vec<SampleSpanLink>,
}

/// Represents a span event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanEvent {
    /// The name of the event
    pub name: String,
    /// The attributes of the event
    #[serde(skip_serializing)]
    pub attributes: Vec<SampleAttribute>,
}

/// Represents a span link
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpanLink {
    /// The attributes of the link
    #[serde(skip_serializing)]
    pub attributes: Vec<SampleAttribute>,
}
