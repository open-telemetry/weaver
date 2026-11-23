// SPDX-License-Identifier: Apache-2.0

//! Native V2 attribute catalog builder.

use std::collections::{BTreeMap, BTreeSet};
use weaver_resolved_schema::v2::attribute::{Attribute, AttributeRef as V2AttributeRef};
use weaver_resolved_schema::v2::provenance::Provenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::AttributeRef;

use super::dependency::V2Dependencies;
use super::index::Index;
use super::rules;
use crate::error::Error;
use crate::graph::chosen_versions::{upgrade_if_superseded, SchemaCacheLookup};
use crate::graph::exclusion::is_excluded;
use crate::graph::merge::merge_annotations;

/// A local definition attribute with its exclusion status.
#[derive(Debug, Clone)]
pub(crate) struct LocalDef {
    pub attribute: Attribute,
    pub excluded: bool,
}

/// Catalog builder for V2 attributes.
pub(crate) struct Catalog<'a, C: SchemaCacheLookup> {
    deps: &'a V2Dependencies,
    cache_lookup: &'a C,
    local_defs: BTreeMap<String, LocalDef>,
    v1_imported_defs: Vec<Attribute>,
    attributes: BTreeSet<Attribute>,
}

impl<'a, C: SchemaCacheLookup> Catalog<'a, C> {
    /// Creates a new catalog builder.
    #[must_use]
    pub fn new(deps: &'a V2Dependencies, cache_lookup: &'a C) -> Self {
        Self {
            deps,
            cache_lookup,
            local_defs: BTreeMap::new(),
            v1_imported_defs: Vec::new(),
            attributes: BTreeSet::new(),
        }
    }

    /// Registers all local attribute definitions from the symbol table.
    pub fn define_local(&mut self, index: &Index) -> Result<(), Error> {
        for (key, indexed_def) in index.attributes() {
            let def = &indexed_def.item;
            let excluded = is_excluded(&def.common.annotations);
            let attr = Attribute {
                key: def.key.clone(),
                r#type: def.r#type.clone(),
                examples: def.examples.clone(),
                common: def.common.clone(),
                provenance: Provenance {
                    source: None,
                    path: indexed_def.provenance.path.clone(),
                },
            };
            _ = self.local_defs.insert(
                key.clone(),
                LocalDef {
                    attribute: attr.clone(),
                    excluded,
                },
            );
            _ = self.attributes.insert(attr);
        }
        Ok(())
    }

    /// Resolves an attribute reference, checking exclusion and applying overrides.
    pub fn resolve_attribute_ref(
        &mut self,
        aref: &AttributeRef,
        consumer_id: &str,
        consumer_excluded: bool,
        provenance: Option<&SemconvProvenance>,
    ) -> Result<Attribute, Error> {
        let key = &aref.r#ref;

        if let Some(local_def) = self.local_defs.get(key) {
            if local_def.excluded && !consumer_excluded {
                return Err(Error::ExcludedFromDependencyResolution {
                    id: key.clone(),
                    r#type: "Attribute".to_owned(),
                    used_in: consumer_id.to_owned(),
                    provenance: provenance.map(|p| Box::new(p.clone())),
                });
            }
            let resolved = rules::apply_attribute_ref_overrides(&local_def.attribute, aref);
            _ = self.attributes.insert(resolved.clone());
            return Ok(resolved);
        }

        if let Some(dep_attr) = self.deps.lookup_attribute(key, self.cache_lookup)? {
            let source_ref = self.deps.dependency_ref(&dep_attr.origin).ok_or_else(|| {
                Error::SchemaNotInCache {
                    schema_url: dep_attr.origin.clone(),
                }
            })?;
            let mut def_attr = dep_attr.attribute;
            def_attr.provenance = Provenance {
                source: Some(source_ref),
                path: String::new(),
            };
            let resolved = rules::apply_attribute_ref_overrides(&def_attr, aref);
            _ = self.attributes.insert(resolved.clone());
            return Ok(resolved);
        }

        Err(Error::UnresolvedAttributeRef {
            group_id: consumer_id.to_owned(),
            attribute_ref: key.clone(),
            provenance: provenance.cloned().map(Box::new),
        })
    }

    /// Upgrades an inherited dependency attribute if superseded and binds its provenance source.
    pub fn register_inherited_dep_attribute(
        &mut self,
        mut attr: Attribute,
        origin: &SchemaUrl,
    ) -> Result<Attribute, Error> {
        let effective_origin =
            upgrade_attribute_content(&mut attr, origin, self.deps, self.cache_lookup)?;

        let source_ref =
            self.deps
                .dependency_ref(&effective_origin)
                .ok_or_else(|| Error::SchemaNotInCache {
                    schema_url: effective_origin.clone(),
                })?;
        attr.provenance = Provenance {
            source: Some(source_ref),
            path: String::new(),
        };
        Ok(attr)
    }

    /// Registers an attribute directly into the catalog.
    pub fn register_attribute(&mut self, attr: Attribute) {
        _ = self.attributes.insert(attr);
    }

    /// Registers an attribute definition imported from a V1 dependency.
    pub fn register_v1_imported_def_attribute(&mut self, attr: Attribute) {
        self.v1_imported_defs.push(attr);
    }

    /// Finalizes the catalog into deduplicated, sorted attributes and indexes.
    #[must_use]
    pub fn finalize(self) -> FinalCatalog {
        let attributes: Vec<Attribute> = self.attributes.into_iter().collect();
        FinalCatalog {
            attributes,
            local_defs: self.local_defs,
            v1_imported_defs: self.v1_imported_defs,
        }
    }
}

/// Finalized V2 attribute catalog with exact indexing.
#[derive(Debug)]
pub(crate) struct FinalCatalog {
    attributes: Vec<Attribute>,
    local_defs: BTreeMap<String, LocalDef>,
    v1_imported_defs: Vec<Attribute>,
}

impl FinalCatalog {
    /// Consumes the catalog and returns the owned sorted attribute vector.
    #[must_use]
    pub fn into_attributes(self) -> Vec<Attribute> {
        self.attributes
    }

    /// Looks up the `AttributeRef` index of a resolved `Attribute` in the catalog.
    #[must_use]
    pub fn attribute_ref(&self, attr: &Attribute) -> V2AttributeRef {
        let idx = self
            .attributes
            .binary_search(attr)
            .expect("Resolved attribute must exist in finalized catalog");
        V2AttributeRef(idx as u32)
    }

    /// Returns the sorted indices of all local attribute definitions for `registry.attributes`.
    #[must_use]
    pub fn local_def_attribute_refs(&self) -> Vec<V2AttributeRef> {
        let mut refs = Vec::with_capacity(self.local_defs.len() + self.v1_imported_defs.len());
        for local_def in self.local_defs.values() {
            refs.push(self.attribute_ref(&local_def.attribute));
        }
        for attr in &self.v1_imported_defs {
            refs.push(self.attribute_ref(attr));
        }
        refs.sort_by_key(|r| r.0);
        refs.dedup();
        refs
    }
}

/// Upgrades an attribute's content and schema URL if the originating schema was superseded, preserving local overrides.
pub(crate) fn upgrade_attribute_content<C: SchemaCacheLookup>(
    attr: &mut Attribute,
    origin: &SchemaUrl,
    deps: &V2Dependencies,
    cache_lookup: &C,
) -> Result<SchemaUrl, Error> {
    let Some((chosen_url, dep_attr)) = upgrade_if_superseded(
        origin,
        cache_lookup,
        |chosen_url, chosen_schema| {
            Ok(deps
                .adapter_for(chosen_url, chosen_schema)
                .lookup_attribute(&attr.key)?
                .filter(|a| !a.excluded))
        },
        |orig, chosen| Error::AttributeNotFoundInUpgradedSchema {
            attribute_name: attr.key.clone(),
            original_url: orig.to_string(),
            upgraded_url: chosen.to_string(),
        },
    )?
    else {
        return Ok(origin.clone());
    };

    attr.r#type = dep_attr.attribute.r#type;
    attr.common.stability = dep_attr.attribute.common.stability;
    attr.common.deprecated = dep_attr.attribute.common.deprecated;

    let orig_base_attr = cache_lookup.lookup_schema(origin).and_then(|orig_schema| {
        deps.adapter_for(origin, &orig_schema)
            .lookup_attribute(&attr.key)
            .ok()
            .flatten()
    });
    if let Some(orig_base) = orig_base_attr {
        if attr.common.brief == orig_base.attribute.common.brief {
            attr.common.brief = dep_attr.attribute.common.brief;
        }
        if attr.common.note == orig_base.attribute.common.note {
            attr.common.note = dep_attr.attribute.common.note;
        }
        if attr.examples == orig_base.attribute.examples {
            attr.examples = dep_attr.attribute.examples;
        }
        if attr.common.annotations == orig_base.attribute.common.annotations {
            attr.common.annotations = dep_attr.attribute.common.annotations;
        } else {
            attr.common.annotations = merge_annotations(
                dep_attr.attribute.common.annotations,
                &attr.common.annotations,
            );
        }
    } else {
        if attr.common.brief.is_empty() {
            attr.common.brief = dep_attr.attribute.common.brief;
        }
        if attr.common.note.is_none() {
            attr.common.note = dep_attr.attribute.common.note;
        }
        if attr.examples.is_none() {
            attr.examples = dep_attr.attribute.examples;
        }
        attr.common.annotations = merge_annotations(
            dep_attr.attribute.common.annotations,
            &attr.common.annotations,
        );
    }

    Ok(chosen_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::dependency::{DependencyAdapter, V2Dependencies, V2Dependency};
    use crate::WeaverResolvedSchema;
    use std::sync::Arc;
    use weaver_resolved_schema::v2::provenance::Provenance;
    use weaver_resolved_schema::v2::refinements::Refinements;
    use weaver_resolved_schema::v2::registry::Registry;
    use weaver_semconv::v2::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};

    struct TestLookupContext {
        chosen_version: SchemaUrl,
        schema: Arc<WeaverResolvedSchema>,
    }

    impl SchemaCacheLookup for TestLookupContext {
        fn chosen_version(&self, _registry_name: &str) -> Option<&SchemaUrl> {
            Some(&self.chosen_version)
        }

        fn lookup_schema(&self, url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>> {
            if url == &self.chosen_version {
                Some(self.schema.clone())
            } else {
                None
            }
        }
    }

    #[test]
    fn test_register_inherited_dep_attribute_missing_in_upgraded_schema() {
        let attr = Attribute {
            key: "missing.in.upgraded".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };

        let s1_url = SchemaUrl::try_from("https://example.com/schemas/1.0.0").unwrap();
        let s2_url = SchemaUrl::try_from("https://example.com/schemas/1.1.0").unwrap();

        let s1 = weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: s1_url.clone(),
            attribute_catalog: vec![attr.clone()],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let s2 = weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: s2_url.clone(),
            attribute_catalog: vec![],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let urls = [s1_url.clone(), s2_url.clone()].into_iter().collect();
        let deps = V2Dependencies::new(
            vec![
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s1))),
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s2.clone()))),
            ],
            urls,
        );

        let ctx = TestLookupContext {
            chosen_version: s2_url,
            schema: Arc::new(WeaverResolvedSchema::V2(s2)),
        };

        let mut cat = Catalog::new(&deps, &ctx);
        let err = cat
            .register_inherited_dep_attribute(attr, &s1_url)
            .unwrap_err();
        assert!(
            matches!(err, Error::AttributeNotFoundInUpgradedSchema { ref attribute_name, .. } if attribute_name == "missing.in.upgraded"),
            "Expected AttributeNotFoundInUpgradedSchema, got: {err:?}"
        );
    }
}
