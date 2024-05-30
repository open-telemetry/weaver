// SPDX-License-Identifier: Apache-2.0

//! Resolve a telemetry schema.

use crate::format::Format;
use crate::registry::RegistryArgs;
use crate::util::{
    check_policies, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from,
};
use crate::{format, DiagnosticArgs};
use clap::Args;
use std::path::PathBuf;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_semconv::registry::SemConvRegistry;

/// Parameters for the `schema resolve` sub-command
#[derive(Debug, Args)]
pub struct SchemaResolveArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

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

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Resolve a telemetry schema and write the resolved schema to a
/// file or print it to stdout.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &SchemaResolveArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!("Resolving schema `{}`", args.registry.registry));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;

    if !args.skip_policies {
        check_policies(
            &registry_path,
            cache,
            &args.policies,
            &semconv_specs,
            logger.clone(),
        )?;
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;

    // Serialize the resolved schema and write it
    // to a file or print it to stdout.
    format::apply_format(&args.format, &schema)
        .map_err(|e| format!("Failed to serialize the registry: {e:?}"))
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

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::format::Format;
    use crate::registry::{RegistryArgs, RegistryPath};
    use crate::run_command;
    use crate::schema::resolve::SchemaResolveArgs;
    use crate::schema::{SchemaCommand, SchemaSubCommand};

    #[test]
    fn test_schema_resolve() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Schema(SchemaCommand {
                command: SchemaSubCommand::Resolve(SchemaResolveArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
                    output: None,
                    format: Format::Yaml,
                    policies: vec![],
                    skip_policies: true,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_code = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_code, 0);

        // Now, let's run the command again with the policy checks enabled.
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Schema(SchemaCommand {
                command: SchemaSubCommand::Resolve(SchemaResolveArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
                    output: None,
                    format: Format::Json,
                    policies: vec![],
                    skip_policies: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_code = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_code, 1);
    }
}
