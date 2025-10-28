//! Catalog of attributes and other.

use std::collections::BTreeMap;

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
pub(crate) struct Catalog {
    /// Catalog of attributes used in the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<Attribute>,
    /// Lookup map to more efficiently find attributes.
    lookup: BTreeMap<String, Vec<usize>>,
}

impl Catalog {
    /// Creates a catalog from a list of attributes.
    pub fn from_attributes(attributes: Vec<Attribute>) -> Self {
        let mut lookup: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, attr) in attributes.iter().enumerate() {
            lookup.entry(attr.key.clone()).or_default().push(idx);
        }
        Self { attributes, lookup }
    }

    /// Lists all the attributes in the registry.
    pub fn attributes(&self) -> &Vec<Attribute> {
        &self.attributes
    }

    #[must_use]
    pub(crate) fn convert_ref(
        &self,
        attribute: &crate::attribute::Attribute,
    ) -> Option<AttributeRef> {
        return self
            .lookup
            .get(&attribute.name)?
            .iter()
            .filter_map(|idx| {
                self.attributes
                    .get(*idx)
                    .filter(|a| {
                        a.key == attribute.name
                    // TODO check everything
                    && a.r#type == attribute.r#type
                    && a.examples == attribute.examples
                    && a.common.brief == attribute.brief
                    && a.common.note == attribute.note
                    && a.common.deprecated == attribute.deprecated
                        // && a.common.stability == attribute.stability
                        // && a.common.annotations == attribute.annotations
                    })
                    .map(|_| AttributeRef(*idx as u32))
            })
            .next();
    }
}
