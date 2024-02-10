// SPDX-License-Identifier: Apache-2.0

//! Resource logs specification.

use crate::attribute::Attribute;
use crate::event::Event;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A resource events specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ResourceEvents {
    /// Common attributes shared across events (implemented as log records).
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Definitions of structured events this application or library generates.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    /// A set of tags for the resource events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

impl ResourceEvents {
    /// Returns the number of events.
    pub fn events_count(&self) -> usize {
        self.events.len()
    }

    /// Returns an event by name or None if not found.
    pub fn event(&self, event_name: &str) -> Option<&Event> {
        self.events
            .iter()
            .find(|event| event.event_name.as_str() == event_name)
    }

    /// Returns a vector of events.
    pub fn events(&self) -> Vec<&Event> {
        self.events.iter().collect()
    }
}
