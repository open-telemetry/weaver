// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use clap::Args;
use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_resolver::attribute::AttributeCatalog;
use weaver_resolver::registry::resolve_semconv_registry;
use weaver_resolver::SchemaResolver;

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct CheckRegistry {
    /// Local path or Git URL of the semantic convention registry to check.
    pub registry: String,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,
}

/// Check a semantic convention registry.
pub(crate) fn check_registry_command(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    registry_args: &CheckRegistry,
) {
    log.loading(&format!("Checking registry `{}`", registry_args.registry));

    // Load the semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = SchemaResolver::load_semconv_registry(
        registry_args.registry.to_string(),
        registry_args.path.clone(),
        cache,
        log.clone(),
    )
    .unwrap_or_else(|e| {
        panic!("Failed to load and parse the semantic convention registry, error: {e}");
    });

    // Resolve the semantic convention registry.
    let mut attr_catalog = AttributeCatalog::default();
    let _ = resolve_semconv_registry(&mut attr_catalog, &registry_args.registry, &semconv_specs)
        .unwrap_or_else(|e| {
            panic!("Failed to resolve the semantic convention registry.\n{e}");
        });

    log.success(&format!(
        "Registry `{}` checked successfully",
        registry_args.registry
    ));
}
