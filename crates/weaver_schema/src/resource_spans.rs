// SPDX-License-Identifier: Apache-2.0

//! A resource spans specification.

use crate::attribute::Attribute;
use crate::span::Span;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A resource spans specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ResourceSpans {
    /// Common attributes shared across spans.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Definitions of all spans this application or library generates.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<Span>,
    /// A set of tags for the resource spans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

impl ResourceSpans {
    /// Returns the number of spans.
    pub fn spans_count(&self) -> usize {
        self.spans.len()
    }

    /// Returns a slice of spans.
    pub fn spans(&self) -> Vec<&Span> {
        self.spans.iter().collect()
    }

    /// Returns a span by name or None if not found.
    pub fn span(&self, name: &str) -> Option<&Span> {
        self.spans
            .iter()
            .find(|span| span.span_name.as_str() == name)
    }
}
