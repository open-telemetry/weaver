// SPDX-License-Identifier: Apache-2.0

//! Log record specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// An event specification.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The name of the event.
    pub event_name: String,
    /// The domain of the event.
    pub domain: String,
    /// The attributes of the log record.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Brief description of the event.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

impl Event {
    /// Returns an attribute by its name.
    pub fn attribute(&self, id: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|a| a.id() == id)
    }
}
