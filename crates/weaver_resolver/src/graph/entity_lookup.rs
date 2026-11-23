// SPDX-License-Identifier: Apache-2.0

//! Shared entity lookup types for entity associations.

use std::collections::BTreeMap;
use weaver_resolved_schema::v2::entity::EntityAssociation as V2EntityAssociation;
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::schema_url::SchemaUrl;

use crate::error::Error;
use crate::graph::chosen_versions::{upgrade_if_superseded, SchemaCacheLookup};
use crate::graph::conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion};
use crate::WeaverResolvedSchema;

/// Origin and exclusion status of a resolved entity definition.
#[derive(Debug, Clone)]
pub(crate) struct EntityLocation {
    /// Schema URL of the registry that originally defined the entity.
    pub origin: SchemaUrl,
    /// Whether the defining registry excludes the entity from dependency resolution.
    pub excluded: bool,
}

/// Lookup of entity definitions and refinements for `entity_associations` resolution.
pub(crate) trait EntityLookup {
    /// Looks up an entity by type or refinement ID, returning `None` if absent.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation>;
}

impl EntityLookup for WeaverResolvedSchema {
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        match self {
            WeaverResolvedSchema::V1(s) => s.lookup_entity(name),
            WeaverResolvedSchema::V2(s) => s.lookup_entity(name),
        }
    }
}

/// Outcome of resolving an entity association name across dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntityResolution {
    /// Unambiguously defined by a single dependency origin.
    Found(SchemaUrl),
    /// Defined only as an excluded entity in dependencies.
    Private,
    /// Defined by multiple unrelated dependency registries.
    Ambiguous(Vec<SchemaUrl>),
    /// Not defined in any dependency.
    Unknown,
}

/// Resolves an entity association name across all dependencies, deduplicating by origin registry
/// and arbitrating compatible versions within the same registry family.
pub(crate) fn resolve_entity<T: EntityLookup>(deps: &[T], name: &str) -> EntityResolution {
    let mut origins: Vec<SchemaUrl> = vec![];
    let mut private = false;
    for location in deps.iter().filter_map(|d| d.lookup_entity(name)) {
        if location.excluded {
            private = true;
        } else if let Some(existing) = origins
            .iter_mut()
            .find(|o| o.name() == location.origin.name())
        {
            if existing != &location.origin {
                if let Ok(winning) =
                    UseLatestMajorVersion.resolve_conflict(existing, &location.origin)
                {
                    *existing = winning;
                }
            }
        } else {
            origins.push(location.origin);
        }
    }
    match origins.len() {
        0 if private => EntityResolution::Private,
        0 => EntityResolution::Unknown,
        1 => EntityResolution::Found(origins.swap_remove(0)),
        _ => EntityResolution::Ambiguous(origins),
    }
}

/// Resolves the origin `SchemaUrl` of a V2 item from `deps` (materialized from `schema.dependencies`)
/// when `source` is set, or `schema.schema_url` otherwise.
#[must_use]
pub(crate) fn v2_source_url(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    source: Option<DependencyRef>,
) -> SchemaUrl {
    source
        .and_then(|dep_ref| deps.get(dep_ref.0 as usize).cloned())
        .unwrap_or_else(|| schema.schema_url.clone())
}

/// Resolves the defining `SchemaUrl` for each non-excluded entity referenced in `associations` within `schema`.
#[must_use]
pub(crate) fn v2_association_origins(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    associations: &[V2EntityAssociation],
) -> BTreeMap<String, SchemaUrl> {
    associations
        .iter()
        .flat_map(|assoc| assoc.refs())
        .filter_map(|entity_ref| {
            let origin = match entity_ref.provenance.source {
                Some(dep_ref) => deps.get(dep_ref.0 as usize).cloned(),
                None => schema
                    .lookup_entity(&entity_ref.r#type)
                    .filter(|location| !location.excluded)
                    .map(|location| location.origin),
            };
            origin.map(|origin| (entity_ref.r#type.to_string(), origin))
        })
        .collect()
}

/// Upgrades an entity association origin to the chosen version if superseded, failing if the entity was removed or excluded.
pub(crate) fn upgrade_entity_origin<C: SchemaCacheLookup>(
    entity_name: &str,
    origin: &SchemaUrl,
    cache_lookup: &C,
) -> Result<SchemaUrl, Error> {
    let Some((chosen_url, ())) = upgrade_if_superseded(
        origin,
        cache_lookup,
        |_, chosen_schema| {
            Ok(chosen_schema
                .lookup_entity(entity_name)
                .is_some_and(|loc| !loc.excluded)
                .then_some(()))
        },
        |orig, chosen| Error::EntityNotFoundInUpgradedSchema {
            entity_name: entity_name.to_owned(),
            original_url: orig.to_string(),
            upgraded_url: chosen.to_string(),
        },
    )?
    else {
        return Ok(origin.clone());
    };
    Ok(chosen_url)
}
