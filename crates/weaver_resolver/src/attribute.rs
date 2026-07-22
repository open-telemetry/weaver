// SPDX-License-Identifier: Apache-2.0

//! Attribute resolution.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use weaver_resolved_schema::attribute::AttributeRef;
use weaver_resolved_schema::attribute::{self};
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::attribute::AttributeSpec;
use weaver_semconv::schema_url::SchemaUrl;

use crate::conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion};
use crate::dependency::ResolvedDependency;
use crate::dependency_resolution::is_excluded;
use crate::Error;
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2AttributeCatalogTrait;

/// A catalog of deduplicated resolved attributes with their corresponding reference.
#[derive(Deserialize, Debug, Default, PartialEq)]
pub struct AttributeCatalog {
    /// A map of deduplicated resolved attributes with their corresponding reference.
    attribute_refs: HashMap<attribute::Attribute, AttributeRef>,
    #[serde(skip)]
    /// A map of root attributes indexed by their name.
    /// Root attributes are attributes that doesn't inherit from another attribute.
    root_attributes: HashMap<String, AttributeWithSource>,
}

#[derive(Debug, PartialEq, Clone)]
/// The source of an attribute (local or dependency).
pub enum AttributeSource {
    /// The attribute is defined locally in a group.
    Local { group_id: String },
    /// The attribute is defined in a dependency schema.
    Dependency { schema_url: SchemaUrl },
}

#[derive(Debug, PartialEq, Clone)]
/// The Attribute with its source.
pub struct AttributeWithSource {
    /// The attribute.
    pub attribute: attribute::Attribute,
    /// The source.
    pub source: AttributeSource,
}

impl AttributeCatalog {
    /// Returns an attribute from a reference.
    /// NOTE: this is inefficient and should only be used in tests.
    #[cfg(test)]
    pub fn attribute(&self, ar: &AttributeRef) -> Option<attribute::Attribute> {
        self.attribute_refs
            .iter()
            .find_map(|(a, r)| if ar == r { Some(a.clone()) } else { None })
    }

    /// Returns the reference of the given attribute or creates a new reference if the attribute
    /// does not exist in the catalog.
    fn attribute_ref(&mut self, attr: attribute::Attribute) -> AttributeRef {
        let next_id = self.attribute_refs.len() as u32;
        *self
            .attribute_refs
            .entry(attr)
            .or_insert_with(|| AttributeRef(next_id))
    }

    /// Returns an attribute ref from the catalog and also registers the attribute
    /// in `root_attributes` so its provenance is recorded.
    ///
    /// # Transitive Dependency Version Upgrading
    ///
    /// When `self` (the parent catalog being resolved, e.g. `main`) imports groups across a
    /// multi-registry diamond dependency graph (e.g., `main` -> [`a/0.1.0`, `b/0.1.0`]),
    /// each dependency (`a` or `b`) may have been resolved against a different version of a shared
    /// transitive dependency (`c/1.1.0` inside `a` vs. `c/1.2.0` inside `b`).
    ///
    /// If an attribute (`c.attr1`) is imported solely via `a/0.1.0`, `resolve_conflict` will NOT be
    /// triggered because no second path (`b`) brings in `c.attr1`. Without intervention, `main`
    /// would retain the stale `v1.1.0` definition of `c.attr1` while simultaneously using `v1.2.0` for
    /// `c.attr2` (imported via `b`).
    ///
    /// To ensure complete SemVer consistency across all attributes in the graph, we query `cache_lookup`:
    /// 1. If `attr_with_source` has `AttributeSource::Dependency { schema_url: dep_url }` (`c/1.1.0`),
    ///    we check if `cache_lookup.chosen_version("c")` has selected a newer compatible version (`c/1.2.0`).
    /// 2. If `chosen_url > dep_url` (with matching major version), we query the `cache_lookup` for the
    ///    pre-resolved `WeaverResolvedSchema` of `chosen_url` (`c/1.2.0`).
    /// 3. We look up `attr.name` inside `chosen_url`. If found, we replace `attr_with_source` on-the-fly with
    ///    the upgraded definition (`c.attr1` from `v1.2.0`) and update its provenance to `chosen_url`.
    /// 4. If not found in `chosen_url`, fail with an `AttributeNotFoundInUpgradedSchema` error.
    pub(crate) fn upgrade_attribute_with_source<C: crate::SchemaCacheLookup>(
        mut attr_with_source: AttributeWithSource,
        cache_lookup: &C,
    ) -> Result<AttributeWithSource, Error> {
        if let AttributeSource::Dependency { schema_url } = &attr_with_source.source {
            let reg_name = schema_url.name();
            if let Some(chosen_url) = cache_lookup.chosen_version(reg_name) {
                if chosen_url != schema_url {
                    let winning_url =
                        UseLatestMajorVersion.resolve_conflict(schema_url, chosen_url)?;
                    if winning_url == *chosen_url {
                        if let Some(upgraded_schema) = cache_lookup.lookup_schema(chosen_url) {
                            if let Some(upgraded_attr) = upgraded_schema
                                .lookup_attribute(&attr_with_source.attribute.name)?
                            {
                                attr_with_source = upgraded_attr;
                                attr_with_source.source = AttributeSource::Dependency {
                                    schema_url: chosen_url.clone(),
                                };
                            } else {
                                return Err(Error::AttributeNotFoundInUpgradedSchema {
                                    attribute_name: attr_with_source.attribute.name.clone(),
                                    original_url: schema_url.to_string(),
                                    upgraded_url: chosen_url.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(attr_with_source)
    }

    /// Returns an attribute ref from the catalog and also registers the attribute
    /// in `root_attributes` so its provenance is recorded.
    ///
    /// Note: This may "upgrade" an attribute reference to use a different version
    /// when dealing with transitive dependencies and version conflicts.
    pub(crate) fn attribute_ref_with_provenance<C: crate::SchemaCacheLookup>(
        &mut self,
        attr: attribute::Attribute,
        source: AttributeSource,
        cache_lookup: &C,
    ) -> Result<AttributeRef, Error> {
        let attr_name = attr.name.clone();
        let mut new_attr = AttributeWithSource {
            attribute: attr,
            source,
        };
        // Make sure we pick the attribute from the *correct* version of a transitive dependency.
        new_attr = Self::upgrade_attribute_with_source(new_attr, cache_lookup)?;

        // If we have a version conflict - resolve it.
        let winning_attr = if let Some(existing) = self.root_attributes.get(&attr_name) {
            resolve_conflict(&attr_name, new_attr, existing.clone())?
        } else {
            new_attr
        };

        let attr_ref = self.attribute_ref(winning_attr.attribute.clone());
        let _ = self.root_attributes.insert(attr_name, winning_attr);

        Ok(attr_ref)
    }

    /// Returns the attribute with its source provenance if available.
    #[cfg(test)]
    pub fn attribute_with_provenance(
        &self,
        attr_ref: &AttributeRef,
    ) -> Option<&AttributeWithSource> {
        let attr = self.attribute(attr_ref)?;
        self.root_attributes.get(&attr.name)
    }
}

/// Resolves conflicts between two attributes based on provenance.
///
/// Note: When an attribute imported from a dependency is referenced in a local group,
/// we prefer the `Dependency` source over `Local` so that its original `SchemaUrl`
/// provenance is preserved for downstream conflict resolution in diamond dependencies.
/// This is because, when seeing the same definition in local and dependency it's a sign the "local"
/// is an imported definition.
fn resolve_conflict(
    key: &str,
    m: AttributeWithSource,
    existing: AttributeWithSource,
) -> Result<AttributeWithSource, Error> {
    match (&m.source, &existing.source) {
        (AttributeSource::Local { .. }, AttributeSource::Local { .. }) => Ok(existing),
        // Prefer Dependency over Local to preserve original SchemaUrl provenance:
        (AttributeSource::Local { .. }, AttributeSource::Dependency { .. }) => Ok(existing),
        (AttributeSource::Dependency { .. }, AttributeSource::Local { .. }) => Ok(m),
        (
            AttributeSource::Dependency { schema_url },
            AttributeSource::Dependency {
                schema_url: schema_url2,
            },
        ) => {
            let winning_url = UseLatestMajorVersion
                .resolve_conflict(schema_url, schema_url2)
                .map_err(|_| Error::AmbiguousReference {
                    r#ref: key.to_owned(),
                    schema_url1: schema_url.to_string(),
                    schema_url2: schema_url2.to_string(),
                })?;
            if winning_url == *schema_url2 {
                Ok(existing)
            } else {
                Ok(m)
            }
        }
    }
}

impl AttributeCatalog {
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
        self.attribute_refs
            .retain(|_, attr_ref| attr_refs.contains(attr_ref));
        let mut ordered: Vec<(attribute::Attribute, AttributeRef)> = self
            .attribute_refs
            .iter()
            .map(|(a, ar)| (a.clone(), *ar))
            .collect();
        ordered.sort_by(|(ln, _), (rn, _)| ln.cmp(rn));
        // assert_eq!(ordered.len(), self.attribute_refs.len());
        let mut next_id = 0;
        // Construct map that converts old attribute refs into new ones, where
        // the new IDs are increasing using attribute ordering.
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
    pub(crate) fn resolve<C: crate::SchemaCacheLookup>(
        &mut self,
        group_id: &str,
        group_prefix: &str,
        group_excluded: bool,
        attr: &AttributeSpec,
        lineage: Option<&mut GroupLineage>,
        dependencies: &Vec<ResolvedDependency>,
        cache_lookup: &C,
    ) -> Result<Option<AttributeRef>, Error> {
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
                let mut root_attr: Option<&AttributeWithSource> = self.root_attributes.get(r#ref);
                // If we fail to find an attribute, check dependencies first.
                if root_attr.is_none() {
                    if let Some(at) = dependencies.lookup_attribute(r#ref)? {
                        let at = Self::upgrade_attribute_with_source(at, cache_lookup)?;
                        _ = self.root_attributes.insert(r#ref.to_owned(), at);
                        root_attr = self.root_attributes.get(r#ref);
                    }
                }
                if let Some(root_attr) = root_attr {
                    // Cross-registry refs to an excluded item always fail; within
                    // the same registry, the consuming group gets a pass only if
                    // it's also excluded.
                    let target_excluded = root_attr
                        .attribute
                        .annotations
                        .as_ref()
                        .is_some_and(is_excluded);
                    let fails = match &root_attr.source {
                        AttributeSource::Dependency { .. } => target_excluded,
                        AttributeSource::Local { .. } => target_excluded && !group_excluded,
                    };
                    if fails {
                        return Err(Error::ExcludedFromDependencyResolution {
                            id: r#ref.clone(),
                            r#type: "Attribute".to_owned(),
                            used_in: group_id.to_owned(),
                        });
                    }
                    let mut attr_lineage = match &root_attr.source {
                        AttributeSource::Local { group_id } => AttributeLineage::new(group_id),
                        AttributeSource::Dependency { schema_url } => {
                            // Note: We didn't want to break V1 schema - so we encode v2 schema_url tracking via
                            // this special string for now. This can round-trip for now, but looks odd when using
                            // V2 with V1 output.  We expect this to be temporary.
                            AttributeLineage::new(&format!("v2_dependency.{}", schema_url.name()))
                        }
                    };

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
                            AttributeWithSource {
                                attribute: resolved_attr,
                                source: AttributeSource::Local {
                                    group_id: group_id.to_owned(),
                                },
                            },
                        );
                    }

                    Ok(Some(attr_ref))
                } else {
                    Ok(None)
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
                if !group_excluded && annotations.as_ref().is_some_and(is_excluded) {
                    return Err(Error::ExcludedFromDependencyResolution {
                        id: id.clone(),
                        r#type: "Attribute".to_owned(),
                        used_in: group_id.to_owned(),
                    });
                }
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
                    AttributeWithSource {
                        attribute: attr.clone(),
                        source: AttributeSource::Local {
                            group_id: group_id.to_owned(),
                        },
                    },
                );
                Ok(Some(self.attribute_ref(attr)))
            }
        }
    }
}

impl From<AttributeCatalog> for Catalog {
    fn from(attr_catalog: AttributeCatalog) -> Self {
        let root_attributes = attr_catalog
            .root_attributes
            .into_iter()
            .map(|(k, v)| {
                let source_str = match v.source {
                    AttributeSource::Local { group_id } => group_id,
                    AttributeSource::Dependency { schema_url } => {
                        format!("v2_dependency.{}", schema_url.name())
                    }
                };
                (k, (v.attribute, source_str))
            })
            .collect();
        let mut attributes: Vec<(attribute::Attribute, AttributeRef)> =
            attr_catalog.attribute_refs.into_iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        let attributes = attributes.into_iter().map(|(attr, _)| attr).collect();
        Catalog::new(attributes, root_attributes)
    }
}

/// Helper trait for abstracting over V1 and V2 schema.
pub(crate) trait AttributeLookup {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error>;
}

impl AttributeLookup for crate::WeaverResolvedSchema {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        match self {
            crate::WeaverResolvedSchema::V1(s) => s.lookup_attribute(key),
            crate::WeaverResolvedSchema::V2(s) => s.lookup_attribute(key),
        }
    }
}

impl AttributeLookup for Vec<ResolvedDependency> {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        let mut matches = vec![];
        for d in self.iter() {
            if let Some(at) = d.lookup_attribute(key)? {
                matches.push(at);
            }
        }
        matches
            .into_iter()
            .try_fold(None, |acc: Option<AttributeWithSource>, m| match acc {
                None => Ok(Some(m)),
                Some(existing) => {
                    // Handle Exclusions first.  We can exclude groups from dependency resolution,
                    // e.g. in Semconv we deprecate a group and move it to a new schema_url - and
                    // this will mean we don't get conflicts or resolve the deprecated group.
                    let m_excluded = m.attribute.annotations.as_ref().is_some_and(is_excluded);
                    let existing_excluded = existing
                        .attribute
                        .annotations
                        .as_ref()
                        .is_some_and(is_excluded);
                    if m_excluded || existing_excluded {
                        return Ok(Some(if m_excluded { existing } else { m }));
                    }
                    // Handle real version conflicts next.
                    Ok(Some(resolve_conflict(key, m, existing)?))
                }
            })
    }
}

impl AttributeLookup for ResolvedDependency {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        match self {
            ResolvedDependency::V1(schema) => {
                if let Some(mut at) = schema.lookup_attribute(key)? {
                    if let AttributeSource::Local { .. } = at.source {
                        at.source = AttributeSource::Dependency {
                            schema_url: SchemaUrl::try_from(schema.schema_url.as_str()).map_err(
                                |e| Error::InvalidUrl {
                                    url: schema.schema_url.clone(),
                                    error: e,
                                },
                            )?,
                        };
                    }
                    Ok(Some(at))
                } else {
                    Ok(None)
                }
            }
            ResolvedDependency::V2(schema) => schema.lookup_attribute(key),
        }
    }
}

impl AttributeLookup for V1Schema {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        if let Some((attr, group_id)) = self.catalog.root_attribute(key) {
            // We encode pure schema_url dependencies with magic strings in V1.
            let group = if let Some(schema_name) = group_id.strip_prefix("v2_dependency.") {
                self.registry.groups.iter().find(|g| {
                    if let Some(prov) = g.provenance() {
                        prov.schema_url.name() == schema_name
                    } else {
                        false
                    }
                })
            } else {
                self.registry.groups.iter().find(|g| g.id == group_id)
            };
            let source = if let Some(g) = group {
                if let Some(prov) = g.provenance() {
                    AttributeSource::Dependency {
                        schema_url: prov.schema_url.clone(),
                    }
                } else {
                    AttributeSource::Local {
                        group_id: group_id.to_owned(),
                    }
                }
            } else {
                AttributeSource::Local {
                    group_id: group_id.to_owned(),
                }
            };
            return Ok(Some(AttributeWithSource {
                attribute: attr.clone(),
                source,
            }));
        }

        // Fallback: search in all groups for the attribute
        for group in self.registry.groups.iter() {
            for attr_ref in group.attributes.iter() {
                if let Some(a) = self.catalog.attribute(attr_ref) {
                    if a.name == key {
                        let source = if let Some(prov) = group.provenance() {
                            AttributeSource::Dependency {
                                schema_url: prov.schema_url.clone(),
                            }
                        } else {
                            AttributeSource::Local {
                                group_id: group.id.clone(),
                            }
                        };
                        return Ok(Some(AttributeWithSource {
                            attribute: a.clone(),
                            source,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }
}

impl AttributeLookup for V2Schema {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        Ok(self.registry.attributes.iter().find_map(|attr_ref| {
            let attr = self.attribute_catalog.attribute(attr_ref)?;
            if attr.key == key {
                Some(AttributeWithSource {
                    attribute: attribute::Attribute {
                        name: attr.key.clone(),
                        r#type: attr.r#type.clone(),
                        brief: attr.common.brief.clone(),
                        examples: attr.examples.clone(),
                        tag: None,
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: None,
                        note: attr.common.note.clone(),
                        stability: Some(attr.common.stability.clone()),
                        deprecated: attr.common.deprecated.clone(),
                        prefix: false,
                        tags: None,
                        annotations: Some(attr.common.annotations.clone()),
                        value: None,
                        role: None,
                    },
                    source: AttributeSource::Dependency {
                        schema_url: if let Some(dep_ref) = &attr.provenance.source {
                            self.dependencies
                                .iter()
                                .nth(dep_ref.0 as usize)
                                .cloned()
                                // TODO - Should this be an error?
                                // This probably means the resolved schema is broken.
                                .unwrap_or_else(|| self.schema_url.clone())
                        } else {
                            self.schema_url.clone()
                        },
                    },
                })
            } else {
                None
            }
        }))
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

    #[test]
    fn test_lookup_attribute_ambiguous_reference() {
        use weaver_resolved_schema::v2::attribute::Attribute as AttributeV2;
        use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;

        let attr1 = AttributeV2 {
            key: "error.type".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Default::default(),
        };

        let schema1 = V2Schema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: "http://test/schema/1.0.0".try_into().unwrap(),
            attribute_catalog: vec![attr1.clone()],
            registry: weaver_resolved_schema::v2::registry::Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(0)],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let schema2 = V2Schema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: "http://test/schema/2.0.0".try_into().unwrap(),
            attribute_catalog: vec![attr1.clone()],
            registry: weaver_resolved_schema::v2::registry::Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(0)],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let dep1 = ResolvedDependency::V2(Box::new(schema1));
        let dep2 = ResolvedDependency::V2(Box::new(schema2));

        let deps = vec![dep1, dep2];

        let result = deps.lookup_attribute("error.type");
        assert!(result.is_err());
        if let Err(Error::AmbiguousReference { .. }) = result {
            // ok
        } else {
            panic!("Expected AmbiguousReference error, got {:?}", result);
        }
    }

    #[test]
    fn test_lookup_attribute_local_vs_dependency_conflict() {
        use std::collections::HashMap;
        use weaver_resolved_schema::v2::attribute::Attribute as AttributeV2;
        use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
        use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;

        let attr_name = "error.type";

        // Create V1 Schema (acting as Local source)
        let mut root_attributes = HashMap::new();
        let attr_v1 = attribute::Attribute {
            name: attr_name.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "brief v1".to_owned(),
            examples: None,
            tag: None,
            requirement_level: RequirementLevel::Basic(Recommended),
            sampling_relevant: None,
            note: "".to_owned(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: None,
        };
        _ = root_attributes.insert(attr_name.to_owned(), (attr_v1.clone(), "group1".to_owned()));

        let schema_v1 = V1Schema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "http://test/schema/1.0.0".to_owned(),
            registry_id: "test-registry".to_owned(),
            registry: weaver_resolved_schema::registry::Registry {
                registry_url: "v1-example".to_owned(),
                groups: vec![],
            },
            catalog: Catalog::new(vec![attr_v1], root_attributes),
            resource: None,
            instrumentation_library: None,
            dependencies: Default::default(),
            versions: None,
            registry_manifest: None,
        };

        // Create V2 Schema (acting as Dependency source)
        let attr_v2 = AttributeV2 {
            key: attr_name.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Default::default(),
        };

        let schema_v2 = V2Schema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: "http://test/schema/2.0.0".try_into().unwrap(),
            attribute_catalog: vec![attr_v2],
            registry: weaver_resolved_schema::v2::registry::Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(0)],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let dep1 = ResolvedDependency::V1(Box::new(schema_v1));
        let dep2 = ResolvedDependency::V2(Box::new(schema_v2));

        let deps = vec![dep1, dep2];

        let result = deps.lookup_attribute(attr_name);
        assert!(result.is_err());
        if let Err(Error::AmbiguousReference {
            r#ref,
            schema_url1,
            schema_url2,
        }) = result
        {
            assert_eq!(r#ref, attr_name);
            let urls = [schema_url1, schema_url2];
            assert!(urls.contains(&"http://test/schema/1.0.0".to_owned()));
            assert!(urls.contains(&"http://test/schema/2.0.0".to_owned()));
        } else {
            panic!("Expected AmbiguousReference error, got {:?}", result);
        }
    }

    fn v2_schema_with_attr(
        url: &str,
        key: &str,
        excluded: bool,
    ) -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        use weaver_resolved_schema::v2::attribute::Attribute as AttributeV2;
        use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
        use weaver_semconv::v2::CommonFields;
        use weaver_semconv::YamlValue;

        let mut common = CommonFields::default();
        if excluded {
            _ = common.annotations.insert(
                "dependency_resolution".to_owned(),
                YamlValue(serde_yaml::from_str("exclude: true").expect("valid yaml")),
            );
        }
        let attr = AttributeV2 {
            key: key.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common,
            provenance: Default::default(),
        };
        V2Schema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: url.try_into().expect("valid url"),
            attribute_catalog: vec![attr],
            registry: weaver_resolved_schema::v2::registry::Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(0)],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        }
    }

    #[test]
    fn test_lookup_attribute_excluded_in_dep_is_hidden() {
        // Excluded definitions are invisible to dependents, so a visible match
        // in another dep wins over an excluded one without raising ambiguity.
        let excluded = v2_schema_with_attr("http://test/schema/excluded", "error.type", true);
        let visible = v2_schema_with_attr("http://test/schema/visible", "error.type", false);
        let deps = vec![
            ResolvedDependency::V2(Box::new(excluded)),
            ResolvedDependency::V2(Box::new(visible)),
        ];

        let found = deps
            .lookup_attribute("error.type")
            .expect("lookup")
            .expect("match");
        match found.source {
            AttributeSource::Dependency { schema_url } => {
                assert_eq!(schema_url.to_string(), "http://test/schema/visible");
            }
            other @ AttributeSource::Local { .. } => panic!("unexpected source {other:?}"),
        }
    }
}
