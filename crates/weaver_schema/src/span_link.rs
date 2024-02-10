// SPDX-License-Identifier: Apache-2.0

//! Event specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A span link specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanLink {
    /// The name of the span link.
    pub link_name: String,
    /// The attributes of the span link.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Brief description of the span link.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the span link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}
