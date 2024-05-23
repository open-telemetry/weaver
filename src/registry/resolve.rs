// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use serde::Serialize;

use crate::DiagnosticArgs;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::registry::TemplateRegistry;
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::{
    check_policies, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from,
    RegistryArgs,
};

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

    /// Optional list of policy files to check against the files of the semantic
    /// convention registry.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryResolveArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

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
    .and_then(|s| {
        if let Some(ref path) = args.output {
            // Write the resolved registry to a file.
            std::fs::write(path, s)
                .map_err(|e| format!("Failed to write the resolved registry to file: {e:?}"))
        } else {
            // Print the resolved registry to stdout.
            println!("{}", s);
            Ok(())
        }
    })
    .unwrap_or_else(|e| {
        // Capture all the errors
        panic!("{}", e);
    });

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn apply_format<T: Serialize>(format: &Format, object: &T) -> Result<String, String> {
    match format {
        Format::Yaml => serde_yaml::to_string(object)
            .map_err(|e| format!("Failed to serialize in Yaml the resolved registry: {:?}", e)),
        Format::Json => serde_json::to_string_pretty(object)
            .map_err(|e| format!("Failed to serialize in Json the resolved registry: {:?}", e)),
    }
}
