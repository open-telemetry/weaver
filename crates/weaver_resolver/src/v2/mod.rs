// SPDX-License-Identifier: Apache-2.0

//! Resolves pure V2 semantic convention definitions directly into V2 schemas.

use std::collections::{BTreeSet, HashMap};

use globset::Glob;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::semconv::SemConvSpecWithProvenance;
use weaver_semconv::v2::{GroupWildcard, Imports};

use crate::engine::WeaverResolver;
use crate::error::Error;
use crate::graph::chosen_versions::{collect_chosen_versions, SchemaLookupContext};
use crate::graph::loader::LoadedSemconvRegistry;

pub(crate) mod attributes;
pub(crate) mod dependency;
pub(crate) mod emit;
pub(crate) mod entities;
pub(crate) mod groups;
pub(crate) mod imports;
pub(crate) mod index;
pub(crate) mod refinements;
pub(crate) mod rules;
pub(crate) mod signals;

use self::attributes::Catalog;
use self::dependency::{DependencyAdapter, V2Dependencies};
use self::index::{ImportOrigin, IndexedImports};
#[cfg(test)]
pub(crate) use dependency::DisallowV1AdapterScope;

/// Returns `true` for dependency resolution warnings that propagate to consumers, excluding upstream authoring lints.
fn is_forwarded_dependency_nfe(err: &Error) -> bool {
    matches!(
        err,
        Error::V1DependencyUsesV1Path { .. }
            | Error::FailToResolveDefinition(_)
            | Error::DependencyVersionUpgradedWarning { .. }
            | Error::DeprecatedIncludeUnreferencedWarning {}
    )
}

/// Resolves a pure V2 semantic convention registry and its dependencies into a [`V2Schema`].
pub(crate) fn resolve(
    resolver: &mut WeaverResolver,
    repo: RegistryRepo,
    specs: Vec<SemConvSpecWithProvenance>,
    dependencies: Vec<LoadedSemconvRegistry>,
) -> WResult<V2Schema, Error> {
    let (mut local_index, index_nfes) = match index::build(&specs) {
        Ok(idx) => idx,
        Err(e) => return WResult::FatalErr(e),
    };

    let mut non_fatal_errors = index_nfes;

    if resolver.include_unreferenced() {
        non_fatal_errors.push(Error::DeprecatedIncludeUnreferencedWarning {});
        let wildcard = vec![GroupWildcard(Glob::new("*").expect("valid glob"))];
        local_index.append_import(IndexedImports {
            imports: Imports {
                metrics: Some(wildcard.clone()),
                spans: Some(wildcard.clone()),
                events: Some(wildcard.clone()),
                entities: Some(wildcard.clone()),
                attribute_groups: Some(wildcard),
            },
            origin: ImportOrigin::IncludeUnreferenced,
        });
    }

    let mut resolved_dependencies = Vec::new();

    for d in dependencies {
        match resolver.resolve_dependency(d) {
            WResult::Ok(schema) => resolved_dependencies.push(schema),
            WResult::OkWithNFEs(schema, nfes) => {
                resolved_dependencies.push(schema);
                non_fatal_errors.extend(nfes.into_iter().filter(is_forwarded_dependency_nfe));
            }
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        }
    }

    let mut chosen_versions = HashMap::new();
    for d in &resolved_dependencies {
        collect_chosen_versions(
            SchemaUrl::try_from(d.schema_url_str()).ok().as_ref(),
            d.dependencies(),
            &mut chosen_versions,
        );
    }

    let dep_urls: BTreeSet<SchemaUrl> = chosen_versions.values().cloned().collect();

    let mut pinned_schemas = resolver.active_graph_schemas_snapshot();
    for chosen_url in chosen_versions.values() {
        if !pinned_schemas.contains_key(chosen_url) {
            if let Some(cached) = resolver.active_or_cached_schema(chosen_url) {
                _ = pinned_schemas.insert(chosen_url.clone(), cached);
            }
        }
    }

    // Pre-resolve superseded dependency versions not yet in `pinned_schemas`.
    for (reg_name, chosen_url) in &chosen_versions {
        let supersedes_loaded = resolved_dependencies.iter().any(|d| {
            let self_url = SchemaUrl::try_from(d.schema_url_str()).ok();
            self_url
                .as_ref()
                .is_some_and(|u| u.name() == reg_name && u != chosen_url)
                || d.dependencies()
                    .iter()
                    .any(|sub| sub.name() == reg_name && sub != chosen_url)
        });
        if supersedes_loaded && !pinned_schemas.contains_key(chosen_url) {
            match resolver.resolve_dependency_schema(chosen_url) {
                WResult::Ok(resolved) => {
                    _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                }
                WResult::OkWithNFEs(resolved, nfes) => {
                    _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                    non_fatal_errors.extend(nfes.into_iter().filter(is_forwarded_dependency_nfe));
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }
    }

    let mut adapters = Vec::new();
    for d in &resolved_dependencies {
        adapters.push(DependencyAdapter::from_resolved_schema(d));
    }
    let v2_deps = V2Dependencies::new(adapters, dep_urls.clone());

    let schema_url = match repo.to_v2_manifest() {
        Ok(m) => m.schema_url().clone(),
        Err(_) => match SchemaUrl::try_from_name_version(repo.name(), repo.version()) {
            Ok(url) => url,
            Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
        },
    };

    let direct: Vec<SchemaUrl> = resolved_dependencies
        .iter()
        .filter_map(|d| SchemaUrl::try_from(d.schema_url_str()).ok())
        .collect();
    resolver.record_direct_dependencies_internal(&schema_url, direct);

    let lookup_ctx = SchemaLookupContext {
        chosen_versions,
        cache: resolver.cache_ref(),
        pinned_schemas,
    };

    let mut catalog = Catalog::new(&v2_deps, &lookup_ctx);
    if let Err(e) = catalog.define_local(&local_index) {
        return WResult::FatalErr(e);
    }

    let expanded_groups = match groups::expand(&local_index, &mut catalog, &mut non_fatal_errors) {
        Ok(g) => g,
        Err(e) => return WResult::FatalErr(e),
    };

    let mut resolved_signals =
        match signals::resolve_bases(&local_index, &expanded_groups, &mut catalog) {
            Ok(s) => s,
            Err(e) => return WResult::FatalErr(e),
        };

    let mut resolved_refinements = match refinements::resolve(
        &local_index,
        &v2_deps,
        &resolved_signals,
        &expanded_groups,
        &mut catalog,
        &lookup_ctx,
        &mut non_fatal_errors,
    ) {
        Ok(r) => r,
        Err(e) => return WResult::FatalErr(e),
    };

    let (mut imported_items, import_warnings) = match imports::resolve(
        &schema_url,
        &resolved_signals,
        &expanded_groups,
        local_index.imports(),
        &v2_deps,
        &mut catalog,
        &lookup_ctx,
    ) {
        Ok(res) => res,
        Err(e) => return WResult::FatalErr(e),
    };
    non_fatal_errors.extend(import_warnings);

    if let Err(e) = entities::resolve_associations(
        &schema_url,
        &mut resolved_signals,
        &mut resolved_refinements,
        &mut imported_items,
        &local_index,
        &v2_deps,
        &lookup_ctx,
    ) {
        return WResult::FatalErr(e);
    }

    let final_catalog = catalog.finalize();

    let resolved_schema = emit::schema(
        schema_url,
        final_catalog,
        expanded_groups,
        resolved_signals,
        resolved_refinements,
        imported_items,
        v2_deps.urls(),
        &local_index,
    );

    if non_fatal_errors.is_empty() {
        WResult::Ok(resolved_schema)
    } else {
        WResult::OkWithNFEs(resolved_schema, non_fatal_errors)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_rs_files(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
    }

    #[test]
    fn test_v2_boundary_no_v1_imports() {
        let v2_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("v2");
        let mut rs_files = Vec::new();
        collect_rs_files(&v2_dir, &mut rs_files);

        let crate_v1 = format!("{}:{}", "crate", ":v1");
        let external_v1 = [
            format!("{}:{}", "weaver_semconv", ":v1"),
            format!("{}:{}", "weaver_resolved_schema", ":v1"),
        ];

        for path in rs_files {
            let content = fs::read_to_string(&path).expect("read file content");

            assert!(
                !content.contains(&crate_v1),
                "File {path:?} violates V2 architectural boundary: contains `{crate_v1}`",
            );

            // External V1 schema crates are allowed only in the V1 dependency adapter.
            if path.file_name().is_some_and(|name| name == "v1_dep.rs") {
                continue;
            }

            for pattern in &external_v1 {
                assert!(
                    !content.contains(pattern),
                    "File {path:?} violates V2 architectural boundary: contains `{pattern}`",
                );
            }
        }
    }
}
