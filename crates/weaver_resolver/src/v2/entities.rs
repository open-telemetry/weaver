// SPDX-License-Identifier: Apache-2.0

//! Entity association resolution for V2 signals and refinements.

use std::collections::{BTreeMap, HashMap};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::entity::{
    EntityAssociation as ResolvedEntityAssociation, EntityRef,
};
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::entity_association::EntityAssociation as SemconvEntityAssociation;
use weaver_semconv::v2::signal_id::SignalId;

use super::dependency::{DepImportedEntity, V2Dependencies};
use super::imports::ImportedItems;
use super::index::Index;
use super::refinements::ResolvedRefinements;
use super::rules;
use super::signals::{EntityAssociations, ResolvedSignals};
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::entity_lookup::{resolve_entity, upgrade_entity_origin, EntityResolution};
use crate::graph::exclusion::is_excluded;

/// Canonical type ID and exclusion status of an entity defined or imported locally.
struct LocalEntityInfo {
    type_id: SignalId,
    excluded: bool,
}

/// Resolves entity associations across all local, refined, and imported signals.
pub(crate) fn resolve_associations<C: SchemaCacheLookup>(
    schema_url: &SchemaUrl,
    signals: &mut ResolvedSignals,
    refinements: &mut ResolvedRefinements,
    imported: &mut ImportedItems,
    index: &Index,
    deps: &V2Dependencies,
    cache_lookup: &C,
) -> Result<(), Error> {
    // Seed with imported entities, then override with local entities and refinements.
    let mut local_entities: HashMap<String, LocalEntityInfo> = HashMap::new();
    for en in &imported.entities {
        let type_id = en.r#type.clone();
        _ = local_entities.insert(
            type_id.to_string(),
            LocalEntityInfo {
                type_id,
                excluded: en.excluded,
            },
        );
    }

    for (id, indexed_en) in index.entity_refinements() {
        let norm_id = rules::normalize_refinement_id("entity", id);
        let excluded = is_excluded(&indexed_en.refinement.annotations);
        _ = local_entities.insert(
            norm_id.to_string(),
            LocalEntityInfo {
                type_id: norm_id.clone(),
                excluded,
            },
        );
        _ = local_entities.insert(
            id.to_string(),
            LocalEntityInfo {
                type_id: norm_id,
                excluded,
            },
        );
    }

    for en in &signals.entities {
        let type_id = en.r#type.clone();
        let excluded = is_excluded(&en.common.annotations);
        _ = local_entities.insert(type_id.to_string(), LocalEntityInfo { type_id, excluded });
    }

    let mut errors = Vec::new();

    for m in &mut signals.metrics {
        if !m.entity_associations.is_empty() {
            let signal_id = format!("metric.{}", m.name);
            let referencing_is_excluded = is_excluded(&m.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &m.provenance.path);
            let resolved = resolve_tree(
                &m.entity_associations,
                &m.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            m.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for s in &mut signals.spans {
        if !s.entity_associations.is_empty() {
            let signal_id = format!("span.{}", s.r#type);
            let referencing_is_excluded = is_excluded(&s.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &s.provenance.path);
            let resolved = resolve_tree(
                &s.entity_associations,
                &s.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            s.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for e in &mut signals.events {
        if !e.entity_associations.is_empty() {
            let signal_id = format!("event.{}", e.name);
            let referencing_is_excluded = is_excluded(&e.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &e.provenance.path);
            let resolved = resolve_tree(
                &e.entity_associations,
                &e.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            e.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for (ref_id, m) in &mut refinements.metrics {
        if !m.entity_associations.is_empty() {
            let signal_id = format!("metric.{ref_id}");
            let referencing_is_excluded = is_excluded(&m.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &m.provenance.path);
            let resolved = resolve_tree(
                &m.entity_associations,
                &m.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            m.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for (ref_id, s) in &mut refinements.spans {
        if !s.entity_associations.is_empty() {
            let signal_id = format!("span.{ref_id}");
            let referencing_is_excluded = is_excluded(&s.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &s.provenance.path);
            let resolved = resolve_tree(
                &s.entity_associations,
                &s.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            s.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for (ref_id, e) in &mut refinements.events {
        if !e.entity_associations.is_empty() {
            let signal_id = format!("event.{ref_id}");
            let referencing_is_excluded = is_excluded(&e.common.annotations);
            let provenance = SemconvProvenance::new(schema_url.clone(), &e.provenance.path);
            let resolved = resolve_tree(
                &e.entity_associations,
                &e.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            e.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for m in &mut imported.metrics {
        if !m.entity_associations.is_empty() {
            let signal_id = format!("metric.{}", m.name);
            let referencing_is_excluded = m.excluded;
            let provenance = SemconvProvenance::new(m.origin.clone(), &m.provenance.path);
            let resolved = resolve_tree(
                &m.entity_associations,
                &m.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            m.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for s in &mut imported.spans {
        if !s.entity_associations.is_empty() {
            let signal_id = format!("span.{}", s.r#type);
            let referencing_is_excluded = s.excluded;
            let provenance = SemconvProvenance::new(s.origin.clone(), &s.provenance.path);
            let resolved = resolve_tree(
                &s.entity_associations,
                &s.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            s.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    for e in &mut imported.events {
        if !e.entity_associations.is_empty() {
            let signal_id = format!("event.{}", e.name);
            let referencing_is_excluded = e.excluded;
            let provenance = SemconvProvenance::new(e.origin.clone(), &e.provenance.path);
            let resolved = resolve_tree(
                &e.entity_associations,
                &e.association_origins,
                &local_entities,
                &imported.entities,
                deps,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                cache_lookup,
                &mut errors,
            );
            e.entity_associations = EntityAssociations::Resolved(resolved);
        }
    }

    handle_errors(errors)?;

    Ok(())
}

/// Maximum nesting depth allowed in `one_of` / `all_of` entity association trees.
const MAX_ENTITY_ASSOC_DEPTH: usize = 64;

/// Resolves a signal's raw or inherited entity associations into `ResolvedEntityAssociation` trees.
fn resolve_tree<C: SchemaCacheLookup>(
    assocs: &EntityAssociations,
    association_origins: &BTreeMap<String, SchemaUrl>,
    local_entities: &HashMap<String, LocalEntityInfo>,
    imported_entities: &[DepImportedEntity],
    deps: &V2Dependencies,
    referencing_signal_id: &str,
    referencing_is_excluded: bool,
    provenance: Option<&SemconvProvenance>,
    cache_lookup: &C,
    errors: &mut Vec<Error>,
) -> Vec<ResolvedEntityAssociation> {
    match assocs {
        EntityAssociations::Raw(raw_assocs) => raw_assocs
            .iter()
            .filter_map(|assoc| {
                resolve_semconv_node(
                    assoc,
                    association_origins,
                    local_entities,
                    imported_entities,
                    deps,
                    referencing_signal_id,
                    referencing_is_excluded,
                    provenance,
                    cache_lookup,
                    errors,
                    0,
                )
            })
            .collect(),
        EntityAssociations::Resolved(resolved_assocs) => resolved_assocs
            .iter()
            .filter_map(|assoc| {
                resolve_resolved_node(
                    assoc,
                    association_origins,
                    local_entities,
                    imported_entities,
                    deps,
                    referencing_signal_id,
                    referencing_is_excluded,
                    provenance,
                    cache_lookup,
                    errors,
                    0,
                )
            })
            .collect(),
    }
}

/// Recursively resolves an authored `SemconvEntityAssociation` node up to `MAX_ENTITY_ASSOC_DEPTH`.
fn resolve_semconv_node<C: SchemaCacheLookup>(
    node: &SemconvEntityAssociation,
    association_origins: &BTreeMap<String, SchemaUrl>,
    local_entities: &HashMap<String, LocalEntityInfo>,
    imported_entities: &[DepImportedEntity],
    deps: &V2Dependencies,
    referencing_signal_id: &str,
    referencing_is_excluded: bool,
    provenance: Option<&SemconvProvenance>,
    cache_lookup: &C,
    errors: &mut Vec<Error>,
    depth: usize,
) -> Option<ResolvedEntityAssociation> {
    if depth >= MAX_ENTITY_ASSOC_DEPTH {
        errors.push(Error::ConversionError {
            message: format!(
                "maximum entity association depth of {MAX_ENTITY_ASSOC_DEPTH} exceeded for {referencing_signal_id}"
            ),
        });
        return None;
    }

    match node {
        SemconvEntityAssociation::Ref(id) => {
            match resolve_leaf(
                id.as_str(),
                association_origins,
                local_entities,
                imported_entities,
                deps,
                referencing_signal_id,
                referencing_is_excluded,
                provenance,
                cache_lookup,
            ) {
                Ok(entity_ref) => Some(ResolvedEntityAssociation::Ref(entity_ref)),
                Err(e) => {
                    errors.push(e);
                    None
                }
            }
        }
        SemconvEntityAssociation::OneOf { one_of } => {
            let resolved_children: Vec<_> = one_of
                .iter()
                .filter_map(|child| {
                    resolve_semconv_node(
                        child,
                        association_origins,
                        local_entities,
                        imported_entities,
                        deps,
                        referencing_signal_id,
                        referencing_is_excluded,
                        provenance,
                        cache_lookup,
                        errors,
                        depth + 1,
                    )
                })
                .collect();
            Some(ResolvedEntityAssociation::OneOf {
                one_of: resolved_children,
            })
        }
        SemconvEntityAssociation::AllOf { all_of } => {
            let resolved_children: Vec<_> = all_of
                .iter()
                .filter_map(|child| {
                    resolve_semconv_node(
                        child,
                        association_origins,
                        local_entities,
                        imported_entities,
                        deps,
                        referencing_signal_id,
                        referencing_is_excluded,
                        provenance,
                        cache_lookup,
                        errors,
                        depth + 1,
                    )
                })
                .collect();
            Some(ResolvedEntityAssociation::AllOf {
                all_of: resolved_children,
            })
        }
    }
}

/// Re-resolves an inherited `ResolvedEntityAssociation` node against upgraded and local entities.
fn resolve_resolved_node<C: SchemaCacheLookup>(
    node: &ResolvedEntityAssociation,
    association_origins: &BTreeMap<String, SchemaUrl>,
    local_entities: &HashMap<String, LocalEntityInfo>,
    imported_entities: &[DepImportedEntity],
    deps: &V2Dependencies,
    referencing_signal_id: &str,
    referencing_is_excluded: bool,
    provenance: Option<&SemconvProvenance>,
    cache_lookup: &C,
    errors: &mut Vec<Error>,
    depth: usize,
) -> Option<ResolvedEntityAssociation> {
    if depth >= MAX_ENTITY_ASSOC_DEPTH {
        errors.push(Error::ConversionError {
            message: format!(
                "maximum entity association depth of {MAX_ENTITY_ASSOC_DEPTH} exceeded for {referencing_signal_id}"
            ),
        });
        return None;
    }

    match node {
        ResolvedEntityAssociation::Ref(entity_ref) => {
            match resolve_leaf(
                &entity_ref.r#type,
                association_origins,
                local_entities,
                imported_entities,
                deps,
                referencing_signal_id,
                referencing_is_excluded,
                provenance,
                cache_lookup,
            ) {
                Ok(resolved_ref) => Some(ResolvedEntityAssociation::Ref(resolved_ref)),
                Err(e) => {
                    errors.push(e);
                    None
                }
            }
        }
        ResolvedEntityAssociation::OneOf { one_of } => {
            let resolved_children: Vec<_> = one_of
                .iter()
                .filter_map(|child| {
                    resolve_resolved_node(
                        child,
                        association_origins,
                        local_entities,
                        imported_entities,
                        deps,
                        referencing_signal_id,
                        referencing_is_excluded,
                        provenance,
                        cache_lookup,
                        errors,
                        depth + 1,
                    )
                })
                .collect();
            Some(ResolvedEntityAssociation::OneOf {
                one_of: resolved_children,
            })
        }
        ResolvedEntityAssociation::AllOf { all_of } => {
            let resolved_children: Vec<_> = all_of
                .iter()
                .filter_map(|child| {
                    resolve_resolved_node(
                        child,
                        association_origins,
                        local_entities,
                        imported_entities,
                        deps,
                        referencing_signal_id,
                        referencing_is_excluded,
                        provenance,
                        cache_lookup,
                        errors,
                        depth + 1,
                    )
                })
                .collect();
            Some(ResolvedEntityAssociation::AllOf {
                all_of: resolved_children,
            })
        }
    }
}

/// Resolves a single entity name to a local or dependency-indexed `EntityRef`.
fn resolve_leaf<C: SchemaCacheLookup>(
    entity_name: &str,
    association_origins: &BTreeMap<String, SchemaUrl>,
    local_entities: &HashMap<String, LocalEntityInfo>,
    imported_entities: &[DepImportedEntity],
    deps: &V2Dependencies,
    referencing_signal_id: &str,
    referencing_is_excluded: bool,
    provenance: Option<&SemconvProvenance>,
    cache_lookup: &C,
) -> Result<EntityRef, Error> {
    if let Some(origin) = association_origins.get(entity_name) {
        let upgraded_origin = upgrade_entity_origin(entity_name, origin, cache_lookup)?;
        if imported_entities
            .iter()
            .any(|en| &*en.r#type == entity_name && en.origin == upgraded_origin)
        {
            return Ok(EntityRef::local(SignalId::from(entity_name)));
        }
        let source_idx = deps.dependency_ref(&upgraded_origin);
        return Ok(EntityRef {
            r#type: SignalId::from(entity_name),
            provenance: SchemaProvenance {
                source: source_idx,
                path: String::new(),
            },
        });
    }

    if let Some(local_info) = local_entities.get(entity_name) {
        if local_info.excluded && !referencing_is_excluded {
            return Err(Error::ExcludedFromDependencyResolution {
                id: entity_name.to_owned(),
                r#type: "entity".to_owned(),
                used_in: referencing_signal_id.to_owned(),
                provenance: provenance.map(|p| Box::new(p.clone())),
            });
        }
        return Ok(EntityRef::local(local_info.type_id.clone()));
    }

    match resolve_entity(deps.adapters(), entity_name) {
        EntityResolution::Found(origin) => {
            let upgraded_origin = upgrade_entity_origin(entity_name, &origin, cache_lookup)?;
            let source_idx = deps.dependency_ref(&upgraded_origin);
            Ok(EntityRef {
                r#type: SignalId::from(entity_name),
                provenance: SchemaProvenance {
                    source: source_idx,
                    path: String::new(),
                },
            })
        }
        EntityResolution::Private => Err(Error::UnresolvedEntityAssociation {
            group_id: referencing_signal_id.to_owned(),
            entity_type: entity_name.to_owned(),
            provenance: provenance.cloned().map(Box::new),
        }),
        EntityResolution::Ambiguous(registries) => Err(Error::AmbiguousEntityAssociation {
            group_id: referencing_signal_id.to_owned(),
            entity_type: entity_name.to_owned(),
            registries: registries.iter().map(|u| u.to_string()).collect(),
        }),
        EntityResolution::Unknown => Err(Error::UnresolvedEntityAssociation {
            group_id: referencing_signal_id.to_owned(),
            entity_type: entity_name.to_owned(),
            provenance: provenance.cloned().map(Box::new),
        }),
    }
}
