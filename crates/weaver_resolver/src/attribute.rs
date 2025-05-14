// SPDX-License-Identifier: Apache-2.0

//! Attribute resolution.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use weaver_resolved_schema::attribute;
use weaver_resolved_schema::attribute::AttributeRef;
use weaver_resolved_schema::lineage::{AttributeLineage, GroupLineage};
use weaver_semconv::attribute::AttributeSpec;

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
    pub fn gc_unreferenced_attribute_refs(
        &mut self,
        attr_refs: HashSet<AttributeRef>,
    ) -> HashMap<AttributeRef, AttributeRef> {
        self.attribute_refs
            .retain(|_, attr_ref| attr_refs.contains(attr_ref));
        let mut next_id = 0;
        let gc_map = self
            .attribute_refs
            .values()
            .map(|attr_ref| {
                let new_attr_ref = AttributeRef(next_id);
                next_id += 1;
                (*attr_ref, new_attr_ref)
            })
            .collect();

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
    pub fn resolve(
        &mut self,
        group_id: &str,
        group_prefix: &str,
        attr: &AttributeSpec,
        lineage: Option<&mut GroupLineage>,
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
                let root_attr = self.root_attributes.get(r#ref);
                if let Some(root_attr) = root_attr {
                    let mut attr_lineage = AttributeLineage::new(&root_attr.group_id);

                    if *prefix {
                        // depending on the prefix we either create embedded attribute or normal reference
                        name = format!("{}.{}", group_prefix, r#ref);
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
