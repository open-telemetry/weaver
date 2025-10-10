//! Catalog of attributes and other.

use crate::v2::attribute::{Attribute, AttributeRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A catalog of indexed attributes shared across semconv groups, or signals.
/// Attribute references are used to refer to attributes in the catalog.
///
/// Note : In the future, this catalog could be extended with other entities.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
#[must_use]
pub struct Catalog {
    /// Catalog of attributes used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<Attribute>,
}

// TODO - statistics.

impl Catalog {
    /// Creates a catalog from a list of attributes.
    pub fn from_attributes(attributes: Vec<Attribute>) -> Self {
        Self { attributes }
    }

    /// Lists all the attributes in the registry.
    pub fn attributes(&self) -> &Vec<Attribute> {
        &self.attributes
    }

    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    #[must_use]
    pub fn attribute_key(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.key.as_ref())
    }

    /// Returns the attribute from an attribute ref if it exists.
    #[must_use]
    pub fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.attributes.get(attribute_ref.0 as usize)
    }

    #[must_use]
    pub(crate) fn convert_ref(
        &self,
        attribute: &crate::attribute::Attribute,
    ) -> Option<AttributeRef> {
        self.attributes
            .iter()
            .position(
                |a| {
                    a.key == attribute.name
            // TODO check everything
            && a.r#type == attribute.r#type
            && a.examples == attribute.examples
            && a.common.brief == attribute.brief
            && a.common.note == attribute.note
            && a.common.deprecated == attribute.deprecated
                }, // && a.common.stability == attribute.stability
                   // && a.common.annotations == attribute.annotations
            )
            .map(|idx| AttributeRef(idx as u32))
    }
}
