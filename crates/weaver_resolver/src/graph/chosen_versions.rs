// SPDX-License-Identifier: Apache-2.0

//! Graph-wide chosen version arbitration and schema cache lookup.

use lru::LruCache;
use std::collections::HashMap;
use std::sync::Arc;
use weaver_semconv::schema_url::SchemaUrl;

use crate::error::Error;
use crate::graph::conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion};
use crate::WeaverResolvedSchema;

/// Abstraction for querying chosen dependency versions and resolved schemas from cache during resolution.
pub(crate) trait SchemaCacheLookup {
    /// Returns the highest chosen SchemaUrl across the current graph for a given registry name.
    fn chosen_version(&self, registry_name: &str) -> Option<&SchemaUrl>;

    /// Looks up a resolved schema by its exact SchemaUrl from the cache.
    fn lookup_schema(&self, schema_url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>>;
}

/// No-op lookup context used when no cache or chosen version overrides are needed.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct NullSchemaCacheLookup;

impl SchemaCacheLookup for NullSchemaCacheLookup {
    fn chosen_version(&self, _registry_name: &str) -> Option<&SchemaUrl> {
        None
    }

    fn lookup_schema(&self, _schema_url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>> {
        None
    }
}

impl SchemaCacheLookup for () {
    fn chosen_version(&self, _registry_name: &str) -> Option<&SchemaUrl> {
        None
    }

    fn lookup_schema(&self, _schema_url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>> {
        None
    }
}

/// Concrete lookup context instantiated inside resolution.
pub(crate) struct SchemaLookupContext<'a> {
    /// Highest compatible schema URL selected per registry name.
    pub chosen_versions: HashMap<String, SchemaUrl>,
    /// Shared LRU cache of resolved schemas.
    pub cache: &'a LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>,
    /// Schemas pinned for the active resolution graph even if evicted from `cache`.
    pub pinned_schemas: HashMap<SchemaUrl, Arc<WeaverResolvedSchema>>,
}

impl<'a> SchemaCacheLookup for SchemaLookupContext<'a> {
    fn chosen_version(&self, registry_name: &str) -> Option<&SchemaUrl> {
        self.chosen_versions.get(registry_name)
    }

    fn lookup_schema(&self, schema_url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>> {
        if let Some(schema) = self.pinned_schemas.get(schema_url) {
            return Some(schema.clone());
        }
        self.cache.peek(schema_url).cloned()
    }
}

/// Records `url` in `chosen`, keeping the higher compatible version on conflict.
pub(crate) fn collect_chosen_versions_from_url(
    url: &SchemaUrl,
    chosen: &mut HashMap<String, SchemaUrl>,
) {
    let name = url.name().to_owned();
    if let Some(existing) = chosen.get(&name) {
        if let Ok(chosen_url) = UseLatestMajorVersion.resolve_conflict(existing, url) {
            let _ = chosen.insert(name, chosen_url);
        }
    } else {
        let _ = chosen.insert(name, url.clone());
    }
}

/// Records `url` and each entry of `sub_dependencies` in `chosen`.
pub(crate) fn collect_chosen_versions<'a>(
    url: Option<&SchemaUrl>,
    sub_dependencies: impl IntoIterator<Item = &'a SchemaUrl>,
    chosen: &mut HashMap<String, SchemaUrl>,
) {
    if let Some(url) = url {
        collect_chosen_versions_from_url(url, chosen);
    }
    for sub in sub_dependencies {
        collect_chosen_versions_from_url(sub, chosen);
    }
}

/// Determines whether `origin` has been superseded in this resolution graph by a newer
/// compatible chosen version.
///
/// Returns `Ok(Some(chosen_url))` if `chosen_url` supersedes `origin`, `Ok(None)` if no upgrade
/// applies, or `Err` on incompatible major version conflict.
pub(crate) fn chosen_version_if_superseded<C: SchemaCacheLookup>(
    origin: &SchemaUrl,
    cache_lookup: &C,
) -> Result<Option<SchemaUrl>, Error> {
    let Some(chosen_url) = cache_lookup.chosen_version(origin.name()) else {
        return Ok(None);
    };
    if chosen_url == origin {
        return Ok(None);
    }
    let winning_url = UseLatestMajorVersion.resolve_conflict(origin, chosen_url)?;
    if winning_url != *chosen_url {
        return Ok(None);
    }
    Ok(Some(chosen_url.clone()))
}

/// Returns the pre-resolved schema for `chosen_url`, or `ChosenSchemaNotInCache` if missing.
pub(crate) fn lookup_chosen_schema<C: SchemaCacheLookup>(
    chosen_url: &SchemaUrl,
    cache_lookup: &C,
) -> Result<Arc<WeaverResolvedSchema>, Error> {
    cache_lookup
        .lookup_schema(chosen_url)
        .ok_or_else(|| Error::ChosenSchemaNotInCache {
            schema_url: chosen_url.clone(),
        })
}
