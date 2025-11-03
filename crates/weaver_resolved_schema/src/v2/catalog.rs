//! Catalog of attributes and other.

use std::collections::BTreeMap;

use crate::v2::attribute::{Attribute, AttributeRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A catalog of indexed attributes shared across semconv groups, or signals.
/// Attribute references are used to refer to attributes in the catalog.
///
/// Note: This is meant to be a temporary datastructure used for creating
/// the registry.
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

/// Collapses this catalog into the attribute list, preserving order.
impl From<Catalog> for Vec<Attribute> {
    fn from(val: Catalog) -> Self {
        val.attributes
    }
}

impl Catalog {
    /// Creates a catalog from a list of attributes.
    pub(crate) fn from_attributes(attributes: Vec<Attribute>) -> Self {
        let mut lookup: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, attr) in attributes.iter().enumerate() {
            lookup.entry(attr.key.clone()).or_default().push(idx);
        }
        Self { attributes, lookup }
    }

    /// Converts an attribute from V1 into an AttributeRef
    /// on the current list of attributes in the order of this catalog.
    #[must_use]
    pub(crate) fn convert_ref(
        &self,
        attribute: &crate::attribute::Attribute,
    ) -> Option<AttributeRef> {
        // Note - we do a fast lookup to contentious attributes,
        // then linear scan of attributes with same key but different
        // other aspects.
        self.lookup
            .get(&attribute.name)?
            .iter()
            .find_map(|idx| {
                self.attributes
                    .get(*idx)
                    .filter(|a| {
                        a.key == attribute.name
                            && a.r#type == attribute.r#type
                            && a.examples == attribute.examples
                            && a.common.brief == attribute.brief
                            && a.common.note == attribute.note
                            && a.common.deprecated == attribute.deprecated
                            && attribute
                                .stability
                                .as_ref()
                                .map(|s| a.common.stability == *s)
                                .unwrap_or(false)
                            && attribute
                                .annotations
                                .as_ref()
                                .map(|ans| a.common.annotations == *ans)
                                .unwrap_or(false)
                    })
                    .map(|_| AttributeRef(*idx as u32))
            })
    }
}
