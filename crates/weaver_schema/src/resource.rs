// SPDX-License-Identifier: Apache-2.0

//! A common resource specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A common resource specification.
/// All the attributes mentioned in this specification will be inherited by all
/// the other specialized resource specifications.
/// Only used when a Client SDK is generated.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    /// The common attributes of the resource.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// A set of tags for the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
}

impl Resource {
    /// Returns an iterator over the attributes.
    pub fn attributes(&self) -> impl Iterator<Item = &Attribute> {
        self.attributes.iter()
    }

    /// Returns the tags of the resource or None if not set.
    #[must_use]
    pub fn tags(&self) -> Option<&Tags> {
        self.tags.as_ref()
    }
}
