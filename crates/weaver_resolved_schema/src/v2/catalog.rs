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
        self.lookup.get(&attribute.name)?.iter().find_map(|idx| {
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
                            .unwrap_or(a.common.annotations.is_empty())
                })
                .map(|_| AttributeRef(*idx as u32))
        })
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::{attribute::AttributeType, stability::Stability};

    use super::Catalog;
    use crate::v2::attribute::Attribute;
    use crate::v2::CommonFields;

    #[test]
    fn test_lookup_works() {
        let key = "test.key".to_owned();
        let atype = AttributeType::PrimitiveOrArray(
            weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
        );
        let brief = "brief".to_owned();
        let note = "note".to_owned();
        let stability = Stability::Stable;
        let annotations = BTreeMap::new();
        let catalog = Catalog::from_attributes(vec![Attribute {
            key: key.clone(),
            r#type: atype.clone(),
            examples: None,
            common: CommonFields {
                brief: brief.clone(),
                note: note.clone(),
                stability: stability.clone(),
                deprecated: None,
                annotations: annotations.clone(),
            },
        }]);

        let result = catalog.convert_ref(&crate::attribute::Attribute {
            name: key.clone(),
            r#type: atype.clone(),
            brief: brief.clone(),
            examples: None,
            tag: None,
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: Some(true),
            note: note.clone(),
            stability: Some(stability.clone()),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: Some(annotations.clone()),
            value: None,
            role: None,
        });
        assert!(result.is_some());

        // Make sure "none" annotations is the same as empty annotations.
        let result2 = catalog.convert_ref(&crate::attribute::Attribute {
            name: key.clone(),
            r#type: atype.clone(),
            brief: brief.clone(),
            examples: None,
            tag: None,
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: Some(true),
            note: note.clone(),
            stability: Some(stability.clone()),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: None,
        });
        assert!(result2.is_some());
    }
}
