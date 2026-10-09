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

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_semconv::v2::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};
    use weaver_semconv::v2::CommonFields;

    #[test]
    fn test_attribute_catalog_lookups() {
        let attr = Attribute {
            key: "http.status_code".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            examples: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        };
        let catalog = vec![attr.clone()];

        // Test Vec<Attribute>
        assert_eq!(catalog.attribute(&AttributeRef(0)), Some(&attr));
        assert_eq!(
            catalog.attribute_key(&AttributeRef(0)),
            Some("http.status_code")
        );
        assert_eq!(catalog.attribute(&AttributeRef(1)), None);
        assert_eq!(catalog.attribute_key(&AttributeRef(1)), None);

        // Test &[Attribute] slice
        let slice: &[Attribute] = &catalog;
        assert_eq!(slice.attribute(&AttributeRef(0)), Some(&attr));
        assert_eq!(
            slice.attribute_key(&AttributeRef(0)),
            Some("http.status_code")
        );
        assert_eq!(slice.attribute(&AttributeRef(5)), None);
    }
}
