// SPDX-License-Identifier: Apache-2.0

//! Command to resolve a schema file, then output and display the results on the console.

use clap::{Args, Subcommand};
use std::path::PathBuf;
use std::process::exit;
use weaver_cache::Cache;

use weaver_logger::Logger;
use weaver_resolver::SchemaResolver;
use weaver_schema::SemConvImport;
use weaver_semconv::ResolverConfig;

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
#[no_coverage]
pub fn command_resolve(log: impl Logger + Sync + Clone, command: &ResolveCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        exit(1);
    });
    match command.command {
        ResolveSubCommand::Registry(ref command) => {
            let registry_id = "default";
            let mut registry = SchemaResolver::semconv_registry_from_imports(
                registry_id,
                &[SemConvImport::GitUrl {
                    git_url: command.registry.clone(),
                    path: command.path.clone(),
                }],
                ResolverConfig::with_keep_specs(),
                &cache,
                log.clone(),
            )
            .unwrap_or_else(|e| {
                log.error(&e.to_string());
                exit(1);
            });

            let resolved_schema =
                SchemaResolver::resolve_semantic_convention_registry(&mut registry, log.clone())
                    .unwrap_or_else(|e| {
                        log.error(&e.to_string());
                        exit(1);
                    });
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
            let schema = SchemaResolver::resolve_schema_file(schema, &cache, log.clone());

            match schema {
                Ok(schema) => match serde_yaml::to_string(&schema) {
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
                },
                Err(e) => {
                    log.error(&format!("{}", e));
                    exit(1)
                }
            }
        }
    }
}
