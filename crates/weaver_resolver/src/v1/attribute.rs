// SPDX-License-Identifier: Apache-2.0

//! Attribute resolution.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use weaver_resolved_schema::v1::attribute::{Attribute, AttributeRef};
use weaver_resolved_schema::v1::catalog::{Catalog, RootAttribute};
use weaver_resolved_schema::v1::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2AttributeCatalogTrait;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::convert::{v2_attribute_type_to_v1, v2_examples_to_v1};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::{AttributeSpec, RequirementLevel};

use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::conflict_strategy::arbitrate_origin_conflict;
use crate::graph::exclusion::is_excluded;
use crate::v1::dependency::ResolvedDependency;
use crate::v1::upgrade::upgrade_attribute;
use crate::{Error, WeaverResolvedSchema};

/// Catalog of deduplicated resolved attributes and their references.
#[derive(Deserialize, Debug, Default, PartialEq)]
pub struct AttributeCatalog {
    /// Deduplicated resolved attributes mapped to their catalog references.
    attribute_refs: HashMap<Attribute, AttributeRef>,
    /// Root attributes indexed by name for provenance and reference lookups.
    #[serde(skip)]
    root_attributes: HashMap<String, AttributeWithSource>,
}

/// Origin of an attribute definition.
#[derive(Debug, PartialEq, Clone)]
pub enum AttributeSource {
    /// Defined locally in `group_id`.
    Local { group_id: String },
    /// Defined in a dependency schema.
    Dependency { schema_url: SchemaUrl },
}

/// Resolved attribute paired with its provenance source.
#[derive(Debug, PartialEq, Clone)]
pub struct AttributeWithSource {
    /// Resolved attribute.
    pub attribute: Attribute,
    /// Origin of the attribute.
    pub source: AttributeSource,
    /// Whether references may resolve against this entry; `false` when inherited via refinement from another registry.
    pub is_definition: bool,
}

impl AttributeCatalog {
    /// Returns the attribute for `ar` by linear scan (for tests only).
    #[cfg(test)]
    pub fn attribute(&self, ar: &AttributeRef) -> Option<Attribute> {
        self.attribute_refs
            .iter()
            .find_map(|(a, r)| if ar == r { Some(a.clone()) } else { None })
    }

    /// Returns the catalog reference for `attr`, inserting it if absent.
    fn attribute_ref(&mut self, attr: Attribute) -> AttributeRef {
        let next_id = self.attribute_refs.len() as u32;
        *self
            .attribute_refs
            .entry(attr)
            .or_insert_with(|| AttributeRef(next_id))
    }

    /// Upgrades `attr_with_source` to the graph's chosen dependency version when its source registry was superseded.
    pub(crate) fn upgrade_attribute_with_source<C: SchemaCacheLookup>(
        attr_with_source: AttributeWithSource,
        cache_lookup: &C,
    ) -> Result<AttributeWithSource, Error> {
        upgrade_attribute(attr_with_source, cache_lookup)
    }

    /// Interns `attr` in the catalog, upgrading transitive dependency definitions and recording root provenance.
    pub(crate) fn attribute_ref_with_provenance<C: SchemaCacheLookup>(
        &mut self,
        attr: Attribute,
        source: AttributeSource,
        cache_lookup: &C,
    ) -> Result<AttributeRef, Error> {
        let attr_name = attr.name.clone();
        let mut new_attr = AttributeWithSource {
            attribute: attr,
            source,
            is_definition: true,
        };
        new_attr = Self::upgrade_attribute_with_source(new_attr, cache_lookup)?;

        // Return a ref to this group's own copy of the attribute. Different
        // groups may reference the same attribute with different
        // `requirement_level` or `role`, so we must not return a variant
        // registered earlier by another group.
        let attr_ref = self.attribute_ref(new_attr.attribute.clone());

        // Separately, keep `root_attributes` (one entry per attribute name,
        // used for provenance lookups) up to date. If the name was already
        // registered, `resolve_conflict` decides which source wins.
        let root_attr = if let Some(existing) = self.root_attributes.get(&attr_name) {
            resolve_conflict(&attr_name, new_attr, existing.clone())?
        } else {
            new_attr
        };
        let _ = self.root_attributes.insert(attr_name, root_attr);

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

/// Selects the winning attribute entry on a name collision, preferring dependency provenance over
/// local copies so diamond-dependency version resolution retains the original `SchemaUrl`.
fn resolve_conflict(
    key: &str,
    m: AttributeWithSource,
    existing: AttributeWithSource,
) -> Result<AttributeWithSource, Error> {
    let m_excluded = m.attribute.annotations.as_ref().is_some_and(is_excluded);
    let existing_excluded = existing
        .attribute
        .annotations
        .as_ref()
        .is_some_and(is_excluded);
    if m_excluded != existing_excluded {
        return Ok(if m_excluded { existing } else { m });
    }
    match (m.is_definition, existing.is_definition) {
        (true, false) => return Ok(m),
        (false, true) => return Ok(existing),
        _ => {}
    }
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
            let winning_url = arbitrate_origin_conflict(key, schema_url, schema_url2)?;
            if winning_url == *schema_url2 {
                Ok(existing)
            } else {
                Ok(m)
            }
        }
    }
}

impl AttributeCatalog {
    /// Consumes the catalog and returns deduplicated attributes ordered by `AttributeRef`.
    #[must_use]
    pub fn drain_attributes(self) -> Vec<Attribute> {
        let mut attributes: Vec<(Attribute, AttributeRef)> =
            self.attribute_refs.into_iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        attributes.into_iter().map(|(attr, _)| attr).collect()
    }

    /// Removes attributes outside `attr_refs`, sorts the remainder, and returns the old-to-new `AttributeRef` mapping.
    #[must_use]
    pub fn gc_unreferenced_attribute_refs_and_sort(
        &mut self,
        attr_refs: HashSet<AttributeRef>,
    ) -> HashMap<AttributeRef, AttributeRef> {
        self.attribute_refs
            .retain(|_, attr_ref| attr_refs.contains(attr_ref));
        let mut ordered: Vec<(Attribute, AttributeRef)> = self
            .attribute_refs
            .iter()
            .map(|(a, ar)| (a.clone(), *ar))
            .collect();
        ordered.sort_by(|(ln, _), (rn, _)| ln.cmp(rn));
        let mut next_id = 0;
        let gc_map: HashMap<AttributeRef, AttributeRef> = ordered
            .iter()
            .map(|(_, attr_ref)| {
                let new_attr_ref = AttributeRef(next_id);
                next_id += 1;
                (*attr_ref, new_attr_ref)
            })
            .collect();
        self.attribute_refs.values_mut().for_each(|attr_ref| {
            *attr_ref = gc_map[attr_ref];
        });
        gc_map
    }

    /// Returns attribute names ordered by their `AttributeRef` index.
    #[must_use]
    pub fn attribute_name_index(&self) -> Vec<String> {
        let mut attributes: Vec<(&Attribute, &AttributeRef)> = self.attribute_refs.iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        attributes
            .iter()
            .map(|(attr, _)| attr.name.clone())
            .collect()
    }

    /// Resolves `attr` against the catalog and `dependencies`, returning `Ok(None)` when a `ref` target is not yet registered.
    pub(crate) fn resolve<C: SchemaCacheLookup>(
        &mut self,
        group_id: &str,
        group_prefix: &str,
        group_excluded: bool,
        attr: &AttributeSpec,
        origin: Option<&SchemaUrl>,
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
                let mut root_attr: Option<&AttributeWithSource> = self
                    .root_attributes
                    .get(r#ref)
                    .filter(|root| root.is_definition);
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
                            provenance: None,
                        });
                    }
                    let mut attr_lineage = match &root_attr.source {
                        AttributeSource::Local { group_id } => AttributeLineage::new(group_id),
                        AttributeSource::Dependency { schema_url } => {
                            // Encode dependency provenance with a `v2_dependency.` prefix so V1 lineage strings remain compatible.
                            AttributeLineage::new(&format!("v2_dependency.{}", schema_url.name()))
                        }
                    };

                    if *prefix {
                        name = format!("{group_prefix}.{ref}");
                    } else {
                        name = r#ref.clone();
                    }

                    let resolved_attr = Attribute {
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
                                is_definition: true,
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
                        provenance: None,
                    });
                }
                let attr = Attribute {
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

                let source = match origin {
                    Some(schema_url) => AttributeSource::Dependency {
                        schema_url: schema_url.clone(),
                    },
                    None => AttributeSource::Local {
                        group_id: group_id.to_owned(),
                    },
                };
                let root_attr = AttributeWithSource {
                    attribute: attr.clone(),
                    source,
                    // An `Id` spec carrying an origin is an attribute inherited
                    // from a dependency through a refinement, not a definition
                    // this registry owns.
                    is_definition: origin.is_none(),
                };
                let root_attr = match self.root_attributes.get(id) {
                    Some(existing) => resolve_conflict(id, root_attr, existing.clone())?,
                    None => root_attr,
                };
                _ = self.root_attributes.insert(id.to_owned(), root_attr);
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
                (
                    k,
                    RootAttribute {
                        attribute: v.attribute,
                        source_group: source_str,
                        is_definition: v.is_definition,
                    },
                )
            })
            .collect();
        let mut attributes: Vec<(Attribute, AttributeRef)> =
            attr_catalog.attribute_refs.into_iter().collect();
        attributes.sort_by_key(|(_, attr_ref)| attr_ref.0);
        let attributes = attributes.into_iter().map(|(attr, _)| attr).collect();
        Catalog::new(attributes, root_attributes)
    }
}

/// Looks up root attribute definitions by name across V1 and V2 schemas.
pub(crate) trait AttributeLookup {
    /// Looks up the root attribute definition for `key`, returning `Ok(None)` if not defined.
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error>;
}

impl AttributeLookup for WeaverResolvedSchema {
    fn lookup_attribute(&self, key: &str) -> Result<Option<AttributeWithSource>, Error> {
        match self {
            WeaverResolvedSchema::V1(s) => s.lookup_attribute(key),
            WeaverResolvedSchema::V2(s) => s.lookup_attribute(key),
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
        if let Some((attr, group_id)) = self.catalog.root_attribute_definition(key) {
            // V1 catalogs prefix dependency-owned attributes with `v2_dependency.` in `source_group`.
            let source = if let Some(schema_name) = group_id.strip_prefix("v2_dependency.") {
                // Recover the full `SchemaUrl` from `dependencies` first, since an
                // attribute-only reference imports no group.
                self.dependencies
                    .iter()
                    .find(|url| url.name() == schema_name)
                    .cloned()
                    .or_else(|| {
                        self.registry.groups.iter().find_map(|g| {
                            g.provenance()
                                .filter(|prov| prov.schema_url.name() == schema_name)
                                .map(|prov| prov.schema_url)
                        })
                    })
                    .map(|schema_url| AttributeSource::Dependency { schema_url })
                    .unwrap_or_else(|| AttributeSource::Local {
                        group_id: group_id.to_owned(),
                    })
            } else {
                self.registry
                    .groups
                    .iter()
                    .find(|g| g.id == group_id)
                    .and_then(|g| g.provenance())
                    .map(|prov| AttributeSource::Dependency {
                        schema_url: prov.schema_url,
                    })
                    .unwrap_or_else(|| AttributeSource::Local {
                        group_id: group_id.to_owned(),
                    })
            };
            return Ok(Some(AttributeWithSource {
                attribute: attr.clone(),
                source,
                is_definition: true,
            }));
        }

        // Skip the group scan if `root_attributes` records `key` as a non-definition
        // inherited via refinement from another registry.
        if self.catalog.root_attribute(key).is_some() {
            return Ok(None);
        }

        // Fallback for deserialized schemas without `root_attributes`.
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
                            is_definition: true,
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
        let attr = self
            .registry
            .attributes
            .iter()
            .filter_map(|attr_ref| self.attribute_catalog.attribute(attr_ref))
            .find(|attr| attr.key == key)
            .or_else(|| {
                self.registry
                    .attribute_groups
                    .iter()
                    .flat_map(|g| &g.attributes)
                    .filter_map(|a| self.attribute_catalog.attribute(&a.base))
                    .find(|attr| attr.key == key)
            });

        let Some(attr) = attr else {
            return Ok(None);
        };

        Ok(Some(AttributeWithSource {
            attribute: Attribute {
                name: attr.key.clone(),
                r#type: v2_attribute_type_to_v1(attr.r#type.clone()),
                brief: attr.common.brief.clone(),
                examples: attr.examples.clone().map(v2_examples_to_v1),
                tag: None,
                // Catalog attributes carry no requirement level; default until overridden by a reference.
                requirement_level: RequirementLevel::default(),
                sampling_relevant: None,
                note: attr.common.note.clone().unwrap_or_default(),
                stability: Some(attr.common.stability.clone().into()),
                deprecated: attr.common.deprecated.clone(),
                prefix: false,
                tags: None,
                annotations: Some(attr.common.annotations.clone()),
                value: None,
                role: None,
            },
            is_definition: true,
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
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_semconv::v1::attribute::BasicRequirementLevelSpec::{Recommended, Required};
    use weaver_semconv::v1::attribute::{
        AttributeType, PrimitiveOrArrayTypeSpec, RequirementLevel,
    };

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

    fn gen_attr(id: usize) -> Attribute {
        gen_attr_by_name(format!("attr-{id}"), RequirementLevel::Basic(Recommended))
    }

    fn gen_attr_by_name(name: String, requirement_level: RequirementLevel) -> Attribute {
        Attribute {
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
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
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
        use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
        use weaver_resolved_schema::v2::attribute::Attribute as AttributeV2;
        use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;

        let attr_name = "error.type";

        // Create V1 Schema (acting as Local source)
        let mut root_attributes = HashMap::new();
        let attr_v1 = Attribute {
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
        _ = root_attributes.insert(
            attr_name.to_owned(),
            RootAttribute {
                attribute: attr_v1.clone(),
                source_group: "group1".to_owned(),
                is_definition: true,
            },
        );

        let schema_v1 = V1Schema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "http://test/schema/1.0.0".to_owned(),
            registry_id: "test-registry".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "v1-example".to_owned(),
                entity_association_origins: Default::default(),
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
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
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
                YamlValue(serde_saphyr::from_str("exclude: true").expect("valid yaml")),
            );
        }
        let attr = AttributeV2 {
            key: key.to_owned(),
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
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
