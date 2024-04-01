// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use clap::Args;
use std::path::PathBuf;

use weaver_cache::Cache;
use weaver_forge::debug::print_dedup_errors;
use weaver_forge::registry::TemplateRegistry;
use weaver_forge::{GeneratorConfig, TemplateEngine};
use weaver_logger::Logger;
use weaver_resolver::SchemaResolver;

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
    pub templates: String,

    /// Local path or Git URL of the semantic convention registry.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: String,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,
}

/// Generate artifacts from a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryGenerateArgs,
) {
    logger.loading(&format!(
        "Generating artifacts for the registry `{}`",
        args.registry
    ));

    let registry_id = "default";

    // Load the semantic convention registry into a local cache.
    let mut registry = SchemaResolver::load_semconv_registry(
        registry_id,
        args.registry.to_string(),
        args.registry_git_sub_dir.clone(),
        cache,
        logger.clone(),
    )
    .unwrap_or_else(|e| {
        panic!("Failed to load and parse the semantic convention registry, error: {e}");
    });

    // Resolve the semantic convention registry.
    let schema =
        SchemaResolver::resolve_semantic_convention_registry(&mut registry, logger.clone())
            .expect("Failed to resolve registry");

    let engine = TemplateEngine::try_new(
        &format!("registry/{}", args.target),
        GeneratorConfig::default(),
    )
    .expect("Failed to create template engine");

    let template_registry = TemplateRegistry::try_from_resolved_registry(
        schema
            .registry(registry_id)
            .expect("Failed to get the registry from the resolved schema"),
        schema.catalog(),
    )
    .unwrap_or_else(|e| {
        panic!(
            "Failed to create the context for the template evaluation: {:?}",
            e
        )
    });

    match engine.generate(logger.clone(), &template_registry, args.output.as_path()) {
        Ok(_) => logger.success("Artifacts generated successfully"),
        Err(e) => {
            print_dedup_errors(logger.clone(), e);
            #[allow(clippy::exit)]  // Expected behavior
            std::process::exit(1);
        }
    };
}
