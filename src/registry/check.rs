// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use crate::registry::{semconv_registry_path_from, RegistryPath};
use clap::Args;
use std::path::PathBuf;
use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_policy_engine::Engine;
use weaver_resolver::attribute::AttributeCatalog;
use weaver_resolver::registry::resolve_semconv_registry;
use weaver_resolver::SchemaResolver;

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct RegistryCheckArgs {
    /// Local path or Git URL of the semantic convention registry to check.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: RegistryPath,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,

    /// Optional list of policy files to check against the files of the semantic
    /// convention registry before the resolution process.
    #[arg(short = 'b', long)]
    pub before_resolution_policies: Vec<PathBuf>,
}

/// Check a semantic convention registry.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(log: impl Logger + Sync + Clone, cache: &Cache, args: &RegistryCheckArgs) {
    log.loading(&format!("Checking registry `{}`", args.registry));

    let registry_id = "default";
    let policy_engine = if args.before_resolution_policies.is_empty() {
        None
    } else {
        let mut engine = Engine::new();
        for policy in &args.before_resolution_policies {
            engine.add_policy(policy).unwrap_or_else(|e| {
                panic!("Invalid policy file `{:?}`, error: {e}", policy);
            });
        }
        Some(engine)
    };

    // Load the semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = SchemaResolver::load_semconv_registry(
        registry_id,
        semconv_registry_path_from(&args.registry, &args.registry_git_sub_dir),
        cache,
        log.clone(),
        policy_engine.as_ref(),
    )
    .unwrap_or_else(|e| {
        e.log(log.clone());
        #[allow(clippy::exit)] // We are exiting in a CLI tool
        std::process::exit(1);
    });

    // Resolve the semantic convention registry.
    let mut attr_catalog = AttributeCatalog::default();
    let registry_path = args.registry.to_string();
    let _ = resolve_semconv_registry(&mut attr_catalog, &registry_path, &semconv_specs)
        .unwrap_or_else(|e| {
            e.log(log.clone());
            #[allow(clippy::exit)] // We are exiting in a CLI tool
            std::process::exit(1);
        });

    log.success(&format!(
        "Registry `{}` checked successfully",
        args.registry
    ));
}
