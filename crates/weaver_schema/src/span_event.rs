// SPDX-License-Identifier: Apache-2.0

//! Event specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A span event specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanEvent {
    /// The name of the span event.
    pub event_name: String,
    /// The attributes of the span event.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Brief description of the span event.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the span event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}
