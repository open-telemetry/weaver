// SPDX-License-Identifier: Apache-2.0

//! Legacy V1 resolution algorithm.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use weaver_common::result::WResult;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::semconv::SemConvSpecWithProvenance;
use weaver_semconv::v1::group::ImportsWithProvenance;

use crate::error::Error;
use crate::graph::chosen_versions::SchemaLookupContext;
use crate::graph::loader::LoadedSemconvRegistry;
use crate::v1::attribute::AttributeCatalog;
use crate::v1::dependency::ResolvedDependency;
use crate::v1::registry::resolve_registry_with_dependencies;
use crate::{WeaverResolvedSchema, WeaverResolver};

pub(crate) mod attribute;
pub(crate) mod dependency;
pub(crate) mod exclusion;
pub(crate) mod imports;
pub(crate) mod registry;
pub(crate) mod upgrade;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static V1_ENGINE_DISALLOWED: Cell<bool> = const { Cell::new(false) };
}

/// A test-only RAII scope that asserts `v1::resolve_registry_internal` is never entered.
#[cfg(test)]
pub(crate) struct DisallowV1EngineScope;

#[cfg(test)]
impl DisallowV1EngineScope {
    /// Activates the check on the current test thread.
    #[must_use]
    pub(crate) fn new() -> Self {
        V1_ENGINE_DISALLOWED.with(|c| c.set(true));
        Self
    }
}

#[cfg(test)]
impl Default for DisallowV1EngineScope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Drop for DisallowV1EngineScope {
    fn drop(&mut self) {
        V1_ENGINE_DISALLOWED.with(|c| c.set(false));
    }
}

/// Resolves an internal definition registry and its dependencies using the V1 algorithm.
pub(crate) fn resolve_registry_internal(
    resolver: &mut WeaverResolver,
    repo: RegistryRepo,
    specs: Vec<SemConvSpecWithProvenance>,
    dependencies: Vec<LoadedSemconvRegistry>,
) -> WResult<ResolvedTelemetrySchema, Error> {
    #[cfg(test)]
    V1_ENGINE_DISALLOWED.with(|c| {
        if c.get() {
            panic!(
                "v1::resolve_registry_internal was invoked while DisallowV1EngineScope was active!"
            );
        }
    });
    let v1_specs: Vec<_> = specs
        .into_iter()
        .map(SemConvSpecWithProvenance::into_v1)
        .collect();

    let mut imports = Vec::new();
    for (i, provenance) in v1_specs.iter().filter_map(|v1| {
        v1.spec
            .imports()
            .map(|i| (i.clone(), v1.provenance.clone()))
    }) {
        imports.push(ImportsWithProvenance {
            imports: i,
            provenance,
        });
    }

    let mut opt_resolved_dependencies: Vec<WResult<Arc<WeaverResolvedSchema>, Error>> = vec![];
    for d in dependencies {
        opt_resolved_dependencies.push(resolver.resolve_dependency(d));
    }

    let mut resolved_dependencies = vec![];
    let mut non_fatal_errors = vec![];
    for r in opt_resolved_dependencies {
        match r {
            WResult::Ok(d) => resolved_dependencies.push(ResolvedDependency::from_arc(&d)),
            WResult::OkWithNFEs(d, nfes) => {
                resolved_dependencies.push(ResolvedDependency::from_arc(&d));
                non_fatal_errors.extend(nfes);
            }
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        }
    }

    let manifest = repo.v1_manifest();
    let schema_url = if let Some(m) = manifest.as_ref() {
        m.schema_url().clone()
    } else {
        match SchemaUrl::try_from_name_version(repo.name(), repo.version()) {
            Ok(url) => url,
            Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
        }
    };
    let mut attr_catalog = AttributeCatalog::default();

    let direct: Vec<SchemaUrl> = resolved_dependencies
        .iter()
        .filter_map(|d| match d {
            ResolvedDependency::V1(schema) => SchemaUrl::try_from(schema.schema_url.as_str()).ok(),
            ResolvedDependency::V2(schema) => Some(schema.schema_url.clone()),
        })
        .collect();
    resolver.record_direct_dependencies_internal(&schema_url, direct);

    let mut chosen_versions = HashMap::new();
    for d in &resolved_dependencies {
        d.collect_chosen_versions(&mut chosen_versions);
    }

    let dependencies: BTreeSet<SchemaUrl> = chosen_versions.values().cloned().collect();

    let mut pinned_schemas = resolver.active_graph_schemas_snapshot();
    for chosen_url in chosen_versions.values() {
        if !pinned_schemas.contains_key(chosen_url) {
            if let Some(cached) = resolver.active_or_cached_schema(chosen_url) {
                _ = pinned_schemas.insert(chosen_url.clone(), cached);
            }
        }
    }

    // Pre-resolve uncached chosen versions that supersede a version used in this graph.
    for (reg_name, chosen_url) in &chosen_versions {
        let supersedes_loaded = resolved_dependencies.iter().any(|d| match d {
            ResolvedDependency::V1(s) => {
                let self_url = SchemaUrl::try_from(s.schema_url.as_str()).ok();
                self_url
                    .as_ref()
                    .is_some_and(|u| u.name() == reg_name && u != chosen_url)
                    || s.dependencies
                        .iter()
                        .any(|sub| sub.name() == reg_name && sub != chosen_url)
            }
            ResolvedDependency::V2(s) => {
                (s.schema_url.name() == reg_name && &s.schema_url != chosen_url)
                    || s.dependencies
                        .iter()
                        .any(|sub| sub.name() == reg_name && sub != chosen_url)
            }
        });
        if supersedes_loaded && !pinned_schemas.contains_key(chosen_url) {
            match resolver.resolve_dependency_schema(chosen_url) {
                WResult::Ok(resolved) => {
                    _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                }
                WResult::OkWithNFEs(resolved, nfes) => {
                    _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                    non_fatal_errors.extend(nfes);
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }
    }

    let lookup_ctx = SchemaLookupContext {
        chosen_versions,
        cache: resolver.cache_ref(),
        pinned_schemas,
    };

    let include_unreferenced = resolver.include_unreferenced();
    resolve_registry_with_dependencies(
        &mut attr_catalog,
        repo,
        v1_specs,
        imports,
        resolved_dependencies,
        include_unreferenced,
        &lookup_ctx,
    )
    .map(move |resolved_registry| ResolvedTelemetrySchema {
        file_format: "1.0.0".to_owned(),
        schema_url: schema_url.as_str().to_owned(),
        registry_id: schema_url.name().to_owned(),
        registry: resolved_registry,
        catalog: attr_catalog.into(),
        resource: None,
        instrumentation_library: None,
        dependencies,
        versions: None,
        registry_manifest: manifest,
    })
}
