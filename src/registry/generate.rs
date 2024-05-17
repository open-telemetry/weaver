// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::registry::TemplateRegistry;
use weaver_forge::{GeneratorConfig, OutputDirective, TemplateEngine};
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::{
    check_policies, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from,
    DiagnosticArgs, RegistryPath,
};

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct RegistryGenerateArgs {
    /// Target to generate the artifacts for.
    pub target: String,

    /// Path to the directory where the generated artifacts will be saved.
    /// Default is the `output` directory.
    #[arg(default_value = "output")]
    pub output: PathBuf,

    /// Path to the directory where the templates are located.
    /// Default is the `templates` directory.
    #[arg(short = 't', long, default_value = "templates")]
    pub templates: PathBuf,

    /// Local path or Git URL of the semantic convention registry.
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
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Generate artifacts from a semantic convention registry.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryGenerateArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!(
        "Generating artifacts for the registry `{}`",
        args.registry
    ));

    let registry_id = "default";
    let registry_path = semconv_registry_path_from(&args.registry, &args.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;

    check_policies(
        &registry_path,
        cache,
        &args.policies,
        &semconv_specs,
        logger.clone(),
    )?;
    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;
    let config = GeneratorConfig::new(args.templates.clone());

    let engine = TemplateEngine::try_new(&format!("registry/{}", args.target), config)?;

    let template_registry = TemplateRegistry::try_from_resolved_registry(
        schema
            .registry(registry_id)
            .expect("Failed to get the registry from the resolved schema"),
        schema.catalog(),
    )?;

    engine.generate(
        logger.clone(),
        &template_registry,
        args.output.as_path(),
        &OutputDirective::File,
    )?;

    logger.success("Artifacts generated successfully");
    Ok(())
}
