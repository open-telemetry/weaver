// SPDX-License-Identifier: Apache-2.0

//! Attribute resolution.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use weaver_resolved_schema::attribute::AttributeRef;
use weaver_resolved_schema::attribute::{self};
use weaver_resolved_schema::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::attribute::AttributeSpec;

use crate::dependency::ResolvedDependency;

/// A catalog of deduplicated resolved attributes with their corresponding reference.
#[derive(Deserialize, Debug, Default, PartialEq)]
pub struct AttributeCatalog {
    /// A map of deduplicated resolved attributes with their corresponding reference.
    attribute_refs: HashMap<attribute::Attribute, AttributeRef>,
    #[serde(skip)]
    /// A map of root attributes indexed by their name.
    /// Root attributes are attributes that doesn't inherit from another attribute.
    root_attributes: HashMap<String, AttributeWithGroupId>,
}

#[derive(Debug, PartialEq)]
/// The Attribute with its group ID.
pub struct AttributeWithGroupId {
    /// The attribute.
    pub attribute: attribute::Attribute,
    /// The group ID.
    pub group_id: String,
}

impl AttributeCatalog {
    /// Returns the given attribute from the catalog.
    #[must_use]
    pub fn get_attribute(&self, name: &str) -> Option<&AttributeWithGroupId> {
        self.root_attributes.get(name)
    }

    /// Returns the reference of the given attribute or creates a new reference if the attribute
    /// does not exist in the catalog.
    pub fn attribute_ref(&mut self, attr: attribute::Attribute) -> AttributeRef {
        let next_id = self.attribute_refs.len() as u32;
        *self
            .attribute_refs
            .entry(attr)
            .or_insert_with(|| AttributeRef(next_id))
    }

    /// Returns a list of deduplicated attributes ordered by their references.
    #[must_use]
    pub fn drain_attributes(self) -> Vec<attribute::Attribute> {
        let mut attributes: Vec<(attribute::Attribute, AttributeRef)> =
            self.attribute_refs.into_iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        attributes.into_iter().map(|(attr, _)| attr).collect()
    }

    /// Removes all attributes that are not referenced by the given attribute refs.
    /// This leads to holes in the attribute ref vector so we need to remap the
    /// attribute refs.
    ///
    /// Returns a map of old attribute refs to new attribute refs.
    #[must_use]
    pub fn gc_unreferenced_attribute_refs_and_sort(
        &mut self,
        attr_refs: HashSet<AttributeRef>,
    ) -> HashMap<AttributeRef, AttributeRef> {
        let initial_length = self.attribute_refs.len();
        if attr_refs.is_empty() {
            panic!("Attempting to GC attribute references with no expected references! input: {attr_refs:?}");
        }
        self.attribute_refs
            .retain(|_, attr_ref| attr_refs.contains(attr_ref));
        let mut ordered: Vec<(attribute::Attribute, AttributeRef)> = self
            .attribute_refs
            .iter()
            .map(|(a, ar)| (a.clone(), ar.clone()))
            .collect();
        ordered.sort_by(|(ln, _), (rn, _)| ln.cmp(rn));
        let mut next_id = 0;
        // Construct map that converts old attirbute refs into new ones, where
        // the new IDs are incresing using attribute ordering.
        let gc_map: HashMap<AttributeRef, AttributeRef> = ordered
            .iter()
            .map(|(_, attr_ref)| {
                let new_attr_ref = AttributeRef(next_id);
                next_id += 1;
                (*attr_ref, new_attr_ref)
            })
            .collect();

        // Remap the current catalog
        self.attribute_refs.values_mut().for_each(|attr_ref| {
            *attr_ref = gc_map[attr_ref];
        });
        if gc_map.is_empty() {
            panic!("")
        }
        gc_map
    }

    /// Returns a list of indexed attribute names ordered by their references.
    #[must_use]
    pub fn attribute_name_index(&self) -> Vec<String> {
        let mut attributes: Vec<(&attribute::Attribute, &AttributeRef)> =
            self.attribute_refs.iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        attributes
            .iter()
            .map(|(attr, _)| attr.name.clone())
            .collect()
    }

    /// Tries to resolve the given attribute spec (ref or id) from the catalog.
    /// Returns `None` if the attribute spec is a ref and it does not exist yet
    /// in the catalog.
    pub(crate) fn resolve(
        &mut self,
        group_id: &str,
        group_prefix: &str,
        attr: &AttributeSpec,
        lineage: Option<&mut GroupLineage>,
        dependencies: &Vec<ResolvedDependency>,
    ) -> Option<AttributeRef> {
        match attr {
            AttributeSpec::Ref {
                r#ref,
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
                prefix,
                annotations,
                role,
            } => {
                let name;
                let mut root_attr: Option<&AttributeWithGroupId> = self.root_attributes.get(r#ref);
                // If we fail to find an attribute, check dependencies first.
                if root_attr.is_none() {
                    if let Some(at) = dependencies.lookup_attribute(r#ref) {
                        _ = self.root_attributes.insert(r#ref.to_owned(), at);
                        root_attr = self.root_attributes.get(r#ref);
                    }
                }
                if let Some(root_attr) = root_attr {
                    let mut attr_lineage = AttributeLineage::new(&root_attr.group_id);

                    if *prefix {
                        // depending on the prefix we either create embedded attribute or normal reference
                        name = format!("{group_prefix}.{ref}");
                    } else {
                        name = r#ref.clone();
                    }

                    // Create a fully resolved attribute from an attribute spec
                    // (ref) and override the root attribute with the new
                    // values if they are present.
                    let resolved_attr = attribute::Attribute {
                        name: name.clone(),
                        r#type: root_attr.attribute.r#type.clone(),
                        brief: attr_lineage.brief(brief, &root_attr.attribute.brief),
                        examples: attr_lineage.examples(examples, &root_attr.attribute.examples),
                        tag: attr_lineage.tag(tag, &root_attr.attribute.tag),
                        requirement_level: attr_lineage.requirement_level(
                            requirement_level,
                            &root_attr.attribute.requirement_level,
                        ),
                        sampling_relevant: attr_lineage.sampling_relevant(
                            sampling_relevant,
                            &root_attr.attribute.sampling_relevant,
                        ),
                        note: attr_lineage.note(note, &root_attr.attribute.note),
                        stability: attr_lineage
                            .stability(stability, &root_attr.attribute.stability),
                        deprecated: attr_lineage
                            .deprecated(deprecated, &root_attr.attribute.deprecated),
                        tags: root_attr.attribute.tags.clone(),
                        value: root_attr.attribute.value.clone(),
                        prefix: *prefix,
                        role: attr_lineage.optional_role(role, &root_attr.attribute.role),
                        annotations: attr_lineage
                            .annotations(annotations, &root_attr.attribute.annotations),
                    };

                    let attr_ref = self.attribute_ref(resolved_attr.clone());

                    // Update the lineage based on the inherited fields.
                    // Note: the lineage is only updated if a group lineage is provided.
                    if let Some(lineage) = lineage {
                        lineage.add_attribute_lineage(name.clone(), attr_lineage);
                    }

                    if *prefix {
                        // if it's a prefix with reference
                        // we need to add it to the dictionary of resolved attributes
                        _ = self.root_attributes.insert(
                            name,
                            AttributeWithGroupId {
                                attribute: resolved_attr,
                                group_id: group_id.to_owned(),
                            },
                        );
                    }

                    Some(attr_ref)
                } else {
                    None
                }
            }
            AttributeSpec::Id {
                id,
                r#type,
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
                annotations,
                role,
            } => {
                // Create a fully resolved attribute from an attribute spec (id),
                // and check if it already exists in the catalog.
                // If it does, return the reference to the existing attribute.
                // If it does not, add it to the catalog and return a new reference.
                let attr = attribute::Attribute {
                    name: id.clone(),
                    r#type: r#type.clone(),
                    brief: brief.clone().unwrap_or_default(),
                    examples: examples.clone(),
                    tag: tag.clone(),
                    requirement_level: requirement_level.clone(),
                    sampling_relevant: *sampling_relevant,
                    note: note.clone(),
                    stability: stability.clone(),
                    deprecated: deprecated.clone(),
                    tags: None,
                    value: None,
                    prefix: false,
                    annotations: annotations.clone(),
                    role: role.clone(),
                };

                _ = self.root_attributes.insert(
                    id.to_owned(),
                    AttributeWithGroupId {
                        attribute: attr.clone(),
                        group_id: group_id.to_owned(),
                    },
                );
                Some(self.attribute_ref(attr))
            }
        }
    }
}

/// Helper trait for abstracting over V1 and V2 schema.
trait AttributeLookup {
    fn lookup_attribute(&self, key: &str) -> Option<AttributeWithGroupId>;
}

impl AttributeLookup for Vec<ResolvedDependency> {
    fn lookup_attribute(&self, key: &str) -> Option<AttributeWithGroupId> {
        self.iter().find_map(|d| d.lookup_attribute(key))
    }
}

impl AttributeLookup for ResolvedDependency {
    fn lookup_attribute(&self, key: &str) -> Option<AttributeWithGroupId> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_attribute(key),
            ResolvedDependency::V2(schema) => schema.lookup_attribute(key),
        }
    }
}

impl AttributeLookup for V1Schema {
    fn lookup_attribute(&self, key: &str) -> Option<AttributeWithGroupId> {
        // TODO - fast lookup, not a table scan...
        // Because of how the algorithm works, we need to looks across
        // *all possible* groups for an attribute.
        // Note: This *only* works with lineage and breaks otherwise.
        let result = self.registry.groups.iter().find_map(|g| {
            g.attributes
                .iter()
                .find_map(|ar| {
                    self.catalog
                        .attribute(ar)
                        .filter(|a| a.name == key)
                        .and_then(|a| {
                            let lineage = g
                                .lineage
                                .as_ref()
                                .and_then(|l| l.attribute(&a.name))
                                .filter(|al| al.source_group == g.id);
                            // We defined the attribute.
                            if lineage.is_none() {
                                Some(a.clone())
                            } else {
                                // We did not define the attribute.
                                None
                            }
                        })
                })
                .map(|a| AttributeWithGroupId {
                    attribute: a,
                    group_id: g.id.to_owned(),
                })
        });
        result
    }
}

impl AttributeLookup for V2Schema {
    fn lookup_attribute(&self, key: &str) -> Option<AttributeWithGroupId> {
        todo!("Lookup {key} on v2 schema.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_semconv::attribute::BasicRequirementLevelSpec::{Recommended, Required};
    use weaver_semconv::attribute::{AttributeType, PrimitiveOrArrayTypeSpec, RequirementLevel};

    #[test]
    fn test_attribute_catalog() {
        let mut catalog = AttributeCatalog::default();

        // Generate and add 10 attributes to the catalog.
        for i in 0..10 {
            let attr = gen_attr(i);
            _ = catalog.attribute_ref(attr.clone());
        }

        // Attributes 2, 4, and 7 are referenced all others are not
        // so we will garbage collect them.
        let mut attr_refs: HashSet<AttributeRef> = HashSet::new();
        _ = attr_refs.insert(AttributeRef(2));
        _ = attr_refs.insert(AttributeRef(4));
        _ = attr_refs.insert(AttributeRef(7));
        let attr_ref_map = catalog.gc_unreferenced_attribute_refs_and_sort(attr_refs);

        // We should have 3 attributes left in the catalog.
        // The resulting map should map the old attribute refs [2, 4, 7] to the new ones
        // [0, 1, 2] but in an arbitrary order since we are using a HashMap.
        assert_eq!(catalog.attribute_refs.len(), 3);
        assert_eq!(attr_ref_map.len(), 3);

        // Check that all the old attribute refs are present in the new map
        assert!(attr_ref_map.contains_key(&AttributeRef(2)));
        assert!(attr_ref_map.contains_key(&AttributeRef(4)));
        assert!(attr_ref_map.contains_key(&AttributeRef(7)));

        // Check that the new attribute refs are present in the value map
        let values: Vec<AttributeRef> = attr_ref_map.values().cloned().collect();
        assert!(values.contains(&AttributeRef(0)));
        assert!(values.contains(&AttributeRef(1)));
        assert!(values.contains(&AttributeRef(2)));
    }

    #[test]
    fn test_catalog() {
        let mut attribute_refs = HashMap::new();
        _ = attribute_refs.insert(
            gen_attr_by_name(
                "error.type".to_owned(),
                RequirementLevel::Basic(Recommended),
            ),
            AttributeRef(0),
        );
        _ = attribute_refs.insert(
            gen_attr_by_name("unused".to_owned(), RequirementLevel::Basic(Recommended)),
            AttributeRef(1),
        );
        _ = attribute_refs.insert(
            gen_attr_by_name(
                "auction.id".to_owned(),
                RequirementLevel::Basic(Recommended),
            ),
            AttributeRef(2),
        );
        _ = attribute_refs.insert(
            gen_attr_by_name(
                "auction.name".to_owned(),
                RequirementLevel::Basic(Recommended),
            ),
            AttributeRef(3),
        );
        _ = attribute_refs.insert(
            gen_attr_by_name("auction.id".to_owned(), RequirementLevel::Basic(Required)),
            AttributeRef(4),
        );
        _ = attribute_refs.insert(
            gen_attr_by_name("error.type".to_owned(), RequirementLevel::Basic(Required)),
            AttributeRef(5),
        );

        let mut catalog = AttributeCatalog {
            attribute_refs,
            root_attributes: Default::default(),
        };

        let mut attr_refs: HashSet<AttributeRef> = HashSet::new();
        _ = attr_refs.insert(AttributeRef(3));
        _ = attr_refs.insert(AttributeRef(4));
        _ = attr_refs.insert(AttributeRef(5));
        _ = catalog.gc_unreferenced_attribute_refs_and_sort(attr_refs);
        let attr_names = catalog
            .attribute_name_index()
            .into_iter()
            .collect::<HashSet<_>>();

        assert_eq!(attr_names.len(), 3);
        assert_eq!(
            attr_names,
            HashSet::from([
                "auction.id".to_owned(),
                "auction.name".to_owned(),
                "error.type".to_owned(),
            ])
        );
    }

    fn gen_attr(id: usize) -> attribute::Attribute {
        gen_attr_by_name(format!("attr-{id}"), RequirementLevel::Basic(Recommended))
    }

    fn gen_attr_by_name(name: String, requirement_level: RequirementLevel) -> attribute::Attribute {
        attribute::Attribute {
            name,
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean),
            brief: "brief".to_owned(),
            examples: None,
            tag: None,
            requirement_level,
            sampling_relevant: Some(false),
            note: "NA".to_owned(),
            stability: None,
            deprecated: None,
            tags: None,
            value: None,
            prefix: false,
            annotations: None,
            role: None,
        }
    }
}
