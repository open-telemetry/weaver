// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use crate::registry::{check_policies, load_semconv_specs, resolve_semconv_specs, RegistryPath};
use clap::Args;
use std::path::PathBuf;
use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_semconv::SemConvRegistry;

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
pub(crate) fn command(logger: impl Logger + Sync + Clone, cache: &Cache, args: &RegistryCheckArgs) {
    logger.loading(&format!("Checking registry `{}`", args.registry));

    let registry_id = "default";

    // Load the semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = load_semconv_specs(
        &args.registry,
        &args.registry_git_sub_dir,
        cache,
        logger.clone(),
    );

    check_policies(
        &args.before_resolution_policies,
        &semconv_specs,
        logger.clone(),
    );

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    _ = resolve_semconv_specs(&mut registry, logger);
}
