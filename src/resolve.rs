// SPDX-License-Identifier: Apache-2.0

//! Command to resolve a schema file, then output and display the results on the console.

use std::path::PathBuf;
use std::process::exit;

use clap::{Args, Subcommand};

use weaver_cache::Cache;
use weaver_common::error::ExitIfError;
use weaver_common::Logger;
use weaver_resolver::SchemaResolver;
use weaver_semconv::ResolverConfig;

use crate::error::ExitIfError;

/// Specify the `resolve` command
#[derive(Args)]
pub struct ResolveCommand {
    /// Define the sub-commands for the `resolve` command
    #[clap(subcommand)]
    pub command: ResolveSubCommand,
}

/// Sub-commands for the `resolve` command
#[derive(Subcommand)]
pub enum ResolveSubCommand {
    /// Resolve a semantic convention registry
    Registry(ResolveRegistry),
    /// Resolve a telemetry schema
    Schema(ResolveSchema),
}

/// Parameters for the `resolve registry` sub-command
#[derive(Args)]
pub struct ResolveRegistry {
    /// Registry to resolve
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Parameters for the `resolve schema` sub-command
#[derive(Args)]
pub struct ResolveSchema {
    /// Schema file to resolve
    pub schema: PathBuf,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Resolve a schema file and print the result
#[cfg(not(tarpaulin_include))]
pub fn command_resolve(log: impl Logger + Sync + Clone, command: &ResolveCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        exit(1);
    });
    match command.command {
        ResolveSubCommand::Registry(ref command) => {
            let registry_id = "default";
            let registry_path = weaver_semconv::path::RegistryPath::GitUrl {
                git_url: command.registry.clone(),
                path: command.path.clone(),
            };
            let semconv_specs = SchemaResolver::load_semconv_specs(&registry_path, &cache)
                .exit_if_error(log.clone());
            let mut registry = SchemaResolver::semconv_registry_from_imports(
                registry_id,
                semconv_specs,
                ResolverConfig::with_keep_specs(),
                log.clone(),
            )
            .exit_if_error(log.clone());

            let resolved_schema =
                SchemaResolver::resolve_semantic_convention_registry(&mut registry)
                    .exit_if_error(log.clone());
            match serde_yaml::to_string(&resolved_schema) {
                Ok(yaml) => {
                    if let Some(output) = &command.output {
                        log.loading(&format!(
                            "Saving resolved registry to {}",
                            output
                                .to_str()
                                .unwrap_or("<unrepresentable-filename-not-utf8>")
                        ));
                        if let Err(e) = std::fs::write(output, &yaml) {
                            log.error(&format!(
                                "Failed to write to {}: {}",
                                output.to_str().expect("Invalid filename"),
                                e
                            ));
                            exit(1)
                        }
                        log.success(&format!(
                            "Saved resolved registry to '{}'",
                            output
                                .to_str()
                                .unwrap_or("<unrepresentable-filename-not-utf8>")
                        ));
                    } else {
                        log.log(&yaml);
                    }
                }
                Err(e) => {
                    log.error(&format!("{}", e));
                    exit(1)
                }
            }
        }
        ResolveSubCommand::Schema(ref command) => {
            let schema = command.schema.clone();
            let schema = SchemaResolver::resolve_schema_file(schema, &cache, log.clone())
                .exit_if_error(log.clone());

            match serde_yaml::to_string(&schema) {
                Ok(yaml) => {
                    if let Some(output) = &command.output {
                        log.loading(&format!(
                            "Saving resolved schema to {}",
                            output
                                .to_str()
                                .unwrap_or("<unrepresentable-filename-not-utf8>")
                        ));
                        if let Err(e) = std::fs::write(output, &yaml) {
                            log.error(&format!(
                                "Failed to write to {}: {}",
                                output.to_str().expect("Invalid filename"),
                                e
                            ));
                            exit(1)
                        }
                        log.success(&format!(
                            "Saved resolved schema to '{}'",
                            output
                                .to_str()
                                .unwrap_or("<unrepresentable-filename-not-utf8>")
                        ));
                    } else {
                        log.log(&yaml);
                    }
                }
                Err(e) => {
                    log.error(&format!("{}", e));
                    exit(1)
                }
            }
        }
    }
}
