// SPDX-License-Identifier: Apache-2.0

//! Span specification.

use crate::attribute::Attribute;
use crate::span_event::SpanEvent;
use crate::span_link::SpanLink;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};
use weaver_semconv::group::SpanKindSpec;

/// A span specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct Span {
    /// The name of the span.
    pub span_name: String,
    /// The kind of the span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<SpanKindSpec>,
    /// The attributes of the span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// The events of the span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<SpanEvent>,
    /// The links of the span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<SpanLink>,
    /// Brief description of the span.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

impl Span {
    /// Returns an attribute by its name.
    pub fn attribute(&self, id: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|a| a.id() == id)
    }
}
