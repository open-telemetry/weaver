// SPDX-License-Identifier: Apache-2.0

//! Catalog of attributes and related traits.

use crate::v2::attribute::{Attribute, AttributeRef};

/// Provides methods which can resolve an `AttributeRef` into an `Attribute`.
pub trait AttributeCatalog {
    /// Returns the attribute from an attribute ref if it exists.
    #[must_use]
    fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute>;
    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    #[must_use]
    fn attribute_key(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attribute(attribute_ref).map(|a| a.key.as_str())
    }
}

impl AttributeCatalog for [Attribute] {
    fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.get(attribute_ref.0 as usize)
    }
}

impl AttributeCatalog for Vec<Attribute> {
    fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.get(attribute_ref.0 as usize)
    }
}
