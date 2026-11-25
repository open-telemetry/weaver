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

use super::dependency::V2Dependencies;
use super::imports::{ImportedItem, ImportedItems};
use super::index::Index;
use super::refinements::ResolvedRefinements;
use super::rules;
use super::signals::{
    EntityAssociations, ResolvedEntity, ResolvedEvent, ResolvedMetric, ResolvedSignals,
    ResolvedSpan, SignalKind,
};
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::entity_lookup::{resolve_entity, upgrade_entity_origin, EntityResolution};
use crate::graph::exclusion::is_excluded;

/// Maximum nesting depth allowed in `one_of` / `all_of` entity association trees.
const MAX_ENTITY_ASSOC_DEPTH: usize = 64;

/// Canonical type ID and exclusion status of an entity defined or imported locally.
struct LocalEntityInfo {
    type_id: SignalId,
    excluded: bool,
}

/// Uniform view over authored (`SemconvEntityAssociation`) and inherited (`ResolvedEntityAssociation`) nodes.
enum AssociationBranch<'a, N> {
    Leaf(&'a str),
    OneOf(&'a [N]),
    AllOf(&'a [N]),
}

/// Adapts raw and pre-resolved association tree nodes to share a single recursive resolver.
trait AssociationNode: Sized {
    /// Projects this association node into a leaf entity name or child slice.
    fn branch(&self) -> AssociationBranch<'_, Self>;
}

impl AssociationNode for SemconvEntityAssociation {
    fn branch(&self) -> AssociationBranch<'_, Self> {
        match self {
            Self::Ref(id) => AssociationBranch::Leaf(id.as_str()),
            Self::OneOf { one_of } => AssociationBranch::OneOf(one_of),
            Self::AllOf { all_of } => AssociationBranch::AllOf(all_of),
        }
    }
}

impl AssociationNode for ResolvedEntityAssociation {
    fn branch(&self) -> AssociationBranch<'_, Self> {
        match self {
            Self::Ref(entity_ref) => AssociationBranch::Leaf(&entity_ref.r#type),
            Self::OneOf { one_of } => AssociationBranch::OneOf(one_of),
            Self::AllOf { all_of } => AssociationBranch::AllOf(all_of),
        }
    }
}

/// Uniform access to entity association fields on resolved metric, span, and event signals.
trait AssociableSignal {
    const KIND: SignalKind;
    fn signal_id(&self) -> &SignalId;
    fn provenance_path(&self) -> &str;
    fn is_excluded(&self) -> bool;
    fn associations_and_origins(
        &mut self,
    ) -> (&mut EntityAssociations, &BTreeMap<String, SchemaUrl>);
}

impl AssociableSignal for ResolvedMetric {
    const KIND: SignalKind = SignalKind::Metric;
    fn signal_id(&self) -> &SignalId {
        &self.name
    }
    fn provenance_path(&self) -> &str {
        &self.provenance.path
    }
    fn is_excluded(&self) -> bool {
        is_excluded(&self.common.annotations)
    }
    fn associations_and_origins(
        &mut self,
    ) -> (&mut EntityAssociations, &BTreeMap<String, SchemaUrl>) {
        (&mut self.entity_associations, &self.association_origins)
    }
}

impl AssociableSignal for ResolvedSpan {
    const KIND: SignalKind = SignalKind::Span;
    fn signal_id(&self) -> &SignalId {
        &self.r#type
    }
    fn provenance_path(&self) -> &str {
        &self.provenance.path
    }
    fn is_excluded(&self) -> bool {
        is_excluded(&self.common.annotations)
    }
    fn associations_and_origins(
        &mut self,
    ) -> (&mut EntityAssociations, &BTreeMap<String, SchemaUrl>) {
        (&mut self.entity_associations, &self.association_origins)
    }
}

impl AssociableSignal for ResolvedEvent {
    const KIND: SignalKind = SignalKind::Event;
    fn signal_id(&self) -> &SignalId {
        &self.name
    }
    fn provenance_path(&self) -> &str {
        &self.provenance.path
    }
    fn is_excluded(&self) -> bool {
        is_excluded(&self.common.annotations)
    }
    fn associations_and_origins(
        &mut self,
    ) -> (&mut EntityAssociations, &BTreeMap<String, SchemaUrl>) {
        (&mut self.entity_associations, &self.association_origins)
    }
}

/// Shared lookup tables and error accumulator for resolving entity associations across a registry.
struct AssociationResolver<'a, C: SchemaCacheLookup> {
    local_entities: &'a HashMap<String, LocalEntityInfo>,
    imported_entities: &'a [ImportedItem<ResolvedEntity>],
    deps: &'a V2Dependencies,
    cache_lookup: &'a C,
    errors: Vec<Error>,
}

impl<'a, C: SchemaCacheLookup> AssociationResolver<'a, C> {
    /// Resolves entity associations across a slice of local base signals.
    fn resolve_local_signals<S: AssociableSignal>(
        &mut self,
        signals: &mut [S],
        schema_url: &SchemaUrl,
    ) {
        for sig in signals {
            let id = sig.signal_id().clone();
            let path = sig.provenance_path().to_owned();
            let excluded = sig.is_excluded();
            let (assocs, origins) = sig.associations_and_origins();
            self.resolve_signal(
                assocs,
                origins,
                S::KIND.as_str(),
                &id,
                schema_url,
                &path,
                excluded,
            );
        }
    }

    /// Resolves entity associations across a slice of local signal refinements.
    fn resolve_refinements<S: AssociableSignal>(
        &mut self,
        refinements: &mut [(SignalId, S)],
        schema_url: &SchemaUrl,
    ) {
        for (ref_id, sig) in refinements {
            let path = sig.provenance_path().to_owned();
            let excluded = sig.is_excluded();
            let (assocs, origins) = sig.associations_and_origins();
            self.resolve_signal(
                assocs,
                origins,
                S::KIND.as_str(),
                ref_id,
                schema_url,
                &path,
                excluded,
            );
        }
    }

    /// Resolves entity associations across a slice of imported signals.
    fn resolve_imported_signals<S: AssociableSignal>(&mut self, imported: &mut [ImportedItem<S>]) {
        for entry in imported {
            let id = entry.item.signal_id().clone();
            let path = entry.item.provenance_path().to_owned();
            let (assocs, origins) = entry.item.associations_and_origins();
            self.resolve_signal(
                assocs,
                origins,
                S::KIND.as_str(),
                &id,
                &entry.origin,
                &path,
                entry.excluded,
            );
        }
    }
    /// Resolves `assocs` in place when non-empty.
    fn resolve_signal(
        &mut self,
        assocs: &mut EntityAssociations,
        association_origins: &BTreeMap<String, SchemaUrl>,
        kind_prefix: &str,
        id: &SignalId,
        origin_url: &SchemaUrl,
        provenance_path: &str,
        referencing_is_excluded: bool,
    ) {
        if assocs.is_empty() {
            return;
        }
        let signal_id = format!("{kind_prefix}.{id}");
        let provenance = SemconvProvenance::new(origin_url.clone(), provenance_path);
        let resolved = match assocs {
            EntityAssociations::Raw(raw_assocs) => self.resolve_slice(
                raw_assocs,
                association_origins,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                0,
            ),
            EntityAssociations::Resolved(resolved_assocs) => self.resolve_slice(
                resolved_assocs,
                association_origins,
                &signal_id,
                referencing_is_excluded,
                Some(&provenance),
                0,
            ),
        };
        *assocs = EntityAssociations::Resolved(resolved);
    }

    /// Resolves a slice of sibling association nodes at `depth`.
    fn resolve_slice<N: AssociationNode>(
        &mut self,
        nodes: &[N],
        association_origins: &BTreeMap<String, SchemaUrl>,
        referencing_signal_id: &str,
        referencing_is_excluded: bool,
        provenance: Option<&SemconvProvenance>,
        depth: usize,
    ) -> Vec<ResolvedEntityAssociation> {
        nodes
            .iter()
            .filter_map(|node| {
                self.resolve_node(
                    node,
                    association_origins,
                    referencing_signal_id,
                    referencing_is_excluded,
                    provenance,
                    depth,
                )
            })
            .collect()
    }

    /// Recursively resolves an association tree node up to `MAX_ENTITY_ASSOC_DEPTH`.
    fn resolve_node<N: AssociationNode>(
        &mut self,
        node: &N,
        association_origins: &BTreeMap<String, SchemaUrl>,
        referencing_signal_id: &str,
        referencing_is_excluded: bool,
        provenance: Option<&SemconvProvenance>,
        depth: usize,
    ) -> Option<ResolvedEntityAssociation> {
        if depth >= MAX_ENTITY_ASSOC_DEPTH {
            self.errors.push(Error::ConversionError {
                message: format!(
                    "maximum entity association depth of {MAX_ENTITY_ASSOC_DEPTH} exceeded for {referencing_signal_id}"
                ),
            });
            return None;
        }

        match node.branch() {
            AssociationBranch::Leaf(entity_name) => match self.resolve_leaf(
                entity_name,
                association_origins,
                referencing_signal_id,
                referencing_is_excluded,
                provenance,
            ) {
                Ok(entity_ref) => Some(ResolvedEntityAssociation::Ref(entity_ref)),
                Err(e) => {
                    self.errors.push(e);
                    None
                }
            },
            AssociationBranch::OneOf(children) => {
                let one_of = self.resolve_slice(
                    children,
                    association_origins,
                    referencing_signal_id,
                    referencing_is_excluded,
                    provenance,
                    depth + 1,
                );
                Some(ResolvedEntityAssociation::OneOf { one_of })
            }
            AssociationBranch::AllOf(children) => {
                let all_of = self.resolve_slice(
                    children,
                    association_origins,
                    referencing_signal_id,
                    referencing_is_excluded,
                    provenance,
                    depth + 1,
                );
                Some(ResolvedEntityAssociation::AllOf { all_of })
            }
        }
    }

    /// Resolves a single entity name to a local or dependency-indexed `EntityRef`.
    fn resolve_leaf(
        &self,
        entity_name: &str,
        association_origins: &BTreeMap<String, SchemaUrl>,
        referencing_signal_id: &str,
        referencing_is_excluded: bool,
        provenance: Option<&SemconvProvenance>,
    ) -> Result<EntityRef, Error> {
        if let Some(origin) = association_origins.get(entity_name) {
            let upgraded_origin = upgrade_entity_origin(entity_name, origin, self.cache_lookup)?;
            if self
                .imported_entities
                .iter()
                .any(|en| &*en.item.r#type == entity_name && en.origin == upgraded_origin)
            {
                return Ok(EntityRef::local(SignalId::from(entity_name)));
            }
            let source_idx = self.deps.dependency_ref(&upgraded_origin);
            return Ok(EntityRef {
                r#type: SignalId::from(entity_name),
                provenance: SchemaProvenance {
                    source: source_idx,
                    path: String::new(),
                },
            });
        }

        if let Some(local_info) = self.local_entities.get(entity_name) {
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

        match resolve_entity(self.deps.adapters(), entity_name) {
            EntityResolution::Found(origin) => {
                let upgraded_origin =
                    upgrade_entity_origin(entity_name, &origin, self.cache_lookup)?;
                let source_idx = self.deps.dependency_ref(&upgraded_origin);
                Ok(EntityRef {
                    r#type: SignalId::from(entity_name),
                    provenance: SchemaProvenance {
                        source: source_idx,
                        path: String::new(),
                    },
                })
            }
            EntityResolution::Private | EntityResolution::Unknown => {
                Err(Error::UnresolvedEntityAssociation {
                    group_id: referencing_signal_id.to_owned(),
                    entity_type: entity_name.to_owned(),
                    provenance: provenance.cloned().map(Box::new),
                })
            }
            EntityResolution::Ambiguous(registries) => Err(Error::AmbiguousEntityAssociation {
                group_id: referencing_signal_id.to_owned(),
                entity_type: entity_name.to_owned(),
                registries: registries.iter().map(ToString::to_string).collect(),
            }),
        }
    }
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
        let type_id = en.item.r#type.clone();
        _ = local_entities.insert(
            type_id.to_string(),
            LocalEntityInfo {
                type_id,
                excluded: en.excluded,
            },
        );
    }

    for (id, indexed_en) in index.entity_refinements() {
        let norm_id = rules::normalize_refinement_id(SignalKind::Entity, id);
        let excluded = is_excluded(&indexed_en.item.annotations);
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

    let mut resolver = AssociationResolver {
        local_entities: &local_entities,
        imported_entities: &imported.entities,
        deps,
        cache_lookup,
        errors: Vec::new(),
    };

    resolver.resolve_local_signals(&mut signals.metrics, schema_url);
    resolver.resolve_local_signals(&mut signals.spans, schema_url);
    resolver.resolve_local_signals(&mut signals.events, schema_url);

    resolver.resolve_refinements(&mut refinements.metrics, schema_url);
    resolver.resolve_refinements(&mut refinements.spans, schema_url);
    resolver.resolve_refinements(&mut refinements.events, schema_url);

    resolver.resolve_imported_signals(&mut imported.metrics);
    resolver.resolve_imported_signals(&mut imported.spans);
    resolver.resolve_imported_signals(&mut imported.events);

    handle_errors(resolver.errors)?;
    Ok(())
}
