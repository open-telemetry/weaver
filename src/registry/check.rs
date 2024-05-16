// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use crate::registry::{
    check_policies, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from,
    DiagnosticArgs, RegistryPath,
};
use clap::Args;
use std::path::PathBuf;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_semconv::registry::SemConvRegistry;

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
    /// convention registry.
    #[arg(short = 'p', long)]
    pub policies: Vec<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Check a semantic convention registry.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryCheckArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!("Checking registry `{}`", args.registry));

    let registry_id = "default";
    let registry_path = semconv_registry_path_from(&args.registry, &args.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;

    check_policies(
        &registry_path,
        cache,
        &args.policies,
        &semconv_specs,
        logger.clone(),
    )?;

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    _ = resolve_semconv_specs(&mut registry, logger.clone())?;

    Ok(())
}
