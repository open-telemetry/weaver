// SPDX-License-Identifier: Apache-2.0

//! Defines the catalog of attributes, metrics, and other telemetry items
//! that are shared across multiple signals in the Resolved Telemetry Schema.

use crate::attribute::{Attribute, AttributeRef};
use crate::metric::Metric;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// A catalog of attributes, metrics, and other telemetry signals that are shared
/// in the Resolved Telemetry Schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// Catalog of attributes used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Catalog of metrics used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
}

impl Catalog {
    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    pub fn attribute_name(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.name.as_ref())
    }

    /// Returns the attribute from an attribute ref if it exists.
    pub fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.attributes.get(attribute_ref.0 as usize)
    }
}
