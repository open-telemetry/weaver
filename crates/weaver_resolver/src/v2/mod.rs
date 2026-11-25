// SPDX-License-Identifier: Apache-2.0

//! Resolves pure V2 semantic convention definitions directly into V2 schemas.

use globset::Glob;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::semconv::SemConvSpecWithProvenance;
use weaver_semconv::v2::{GroupWildcard, Imports};

use crate::engine::WeaverResolver;
use crate::error::Error;
use crate::graph::chosen_versions::SchemaLookupContext;
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
    let mut local_index = match index::build(&specs) {
        Ok(idx) => idx,
        Err(e) => return WResult::FatalErr(e),
    };

    let mut non_fatal_errors = Vec::new();

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

    let schema_url = match repo.to_v2_manifest() {
        Ok(m) => m.schema_url().clone(),
        Err(_) => match SchemaUrl::try_from_name_version(repo.name(), repo.version()) {
            Ok(url) => url,
            Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
        },
    };

    let dep_graph = match resolver.resolve_dependency_graph(
        &schema_url,
        dependencies,
        is_forwarded_dependency_nfe,
    ) {
        WResult::Ok(graph) => graph,
        WResult::OkWithNFEs(graph, nfes) => {
            non_fatal_errors.extend(nfes);
            graph
        }
        WResult::FatalErr(e) => return WResult::FatalErr(e),
    };

    let adapters = dep_graph
        .dependencies
        .iter()
        .map(DependencyAdapter::from_resolved_schema)
        .collect();
    let v2_deps = V2Dependencies::new(adapters, dep_graph.dependency_urls);

    let lookup_ctx = SchemaLookupContext {
        chosen_versions: dep_graph.chosen_versions,
        cache: resolver.cache_ref(),
        pinned_schemas: dep_graph.pinned_schemas,
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
        &local_index,
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

    if let Err(e) = signals::validate_span_link_targets(
        &local_index,
        &resolved_signals,
        imported_items.spans.iter().map(|s| s.item.r#type.clone()),
    ) {
        return WResult::FatalErr(e);
    }

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
