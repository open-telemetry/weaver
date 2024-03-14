// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use serde::Serialize;

use weaver_cache::Cache;
use weaver_forge::registry::TemplateRegistry;
use weaver_logger::Logger;
use weaver_resolver::SchemaResolver;

use crate::registry::RegistryArgs;

/// Supported output formats for the resolved schema
#[derive(Debug, Clone, ValueEnum)]
enum Format {
    /// YAML format
    Yaml,
    /// JSON format
    Json,
}

/// Parameters for the `registry resolve` sub-command
#[derive(Debug, Args)]
pub struct RegistryResolveArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Flag to indicate if the shared catalog should be included in the resolved schema
    #[arg(long, default_value = "false")]
    catalog: bool,

    /// Flag to indicate if lineage information should be included in the
    /// resolved schema (not yet implemented)
    #[arg(long, default_value = "false")]
    lineage: bool,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format for the resolved schema
    /// If not specified, the resolved schema is printed in YAML format
    /// Supported formats: yaml, json
    /// Default format: yaml
    /// Example: `--format json`
    #[arg(short, long, default_value = "yaml")]
    format: Format,
}

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryResolveArgs,
) {
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let registry_id = "default";

    // Load the semantic convention registry into a local cache.
    let mut registry = SchemaResolver::load_semconv_registry(
        registry_id,
        args.registry.registry.to_string(),
        args.registry.registry_git_sub_dir.clone(),
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

    // Serialize the resolved schema and write it
    // to a file or print it to stdout.
    match args.catalog {
        // The original resolved schema already includes the catalog.
        // So, we just need to serialize it.
        true => apply_format(&args.format, &schema)
            .map_err(|e| format!("Failed to serialize the registry: {e:?}")),
        // Build a template registry from the resolved schema and serialize it.
        // The template registry does not include any reference to a shared
        // catalog of attributes.
        false => {
            let registry = TemplateRegistry::try_from_resolved_registry(
                schema
                    .registry(registry_id)
                    .expect("Failed to get the registry from the resolved schema"),
                schema.catalog(),
            )
            .unwrap_or_else(|e| panic!("Failed to create the registry without catalog: {e:?}"));
            apply_format(&args.format, &registry)
                .map_err(|e| format!("Failed to serialize the registry: {e:?}"))
        }
    }
    .and_then(|s| match args.output {
        // Write the resolved registry to a file.
        Some(ref path) => std::fs::write(path, s)
            .map_err(|e| format!("Failed to write the resolved registry to file: {e:?}")),
        // Print the resolved registry to stdout.
        None => {
            println!("{}", s);
            Ok(())
        }
    })
    .unwrap_or_else(|e| {
        // Capture all the errors
        panic!("{}", e);
    });
}

fn apply_format<T: Serialize>(format: &Format, object: &T) -> Result<String, String> {
    match format {
        Format::Yaml => serde_yaml::to_string(object)
            .map_err(|e| format!("Failed to serialize in Yaml the resolved registry: {:?}", e)),
        Format::Json => serde_json::to_string_pretty(object)
            .map_err(|e| format!("Failed to serialize in Json the resolved registry: {:?}", e)),
    }
}
