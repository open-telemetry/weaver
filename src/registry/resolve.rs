// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::registry::SemConvRegistry;

use crate::format::{apply_format, Format};
use crate::registry::RegistryArgs;
use crate::util::{check_policy, init_policy_engine, load_semconv_specs, resolve_semconv_specs};
use crate::{DiagnosticArgs, ExitDirectives};
use miette::Diagnostic;

/// Parameters for the `registry resolve` sub-command
#[derive(Debug, Args)]
pub struct RegistryResolveArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

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

    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry. If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryResolveArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    if args.output.is_none() {
        logger.mute();
    }
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();
    let registry_path = args.registry.registry.clone();
    let registry_id = "default";
    let registry_repo = RegistryRepo::try_new("main", &registry_path)?;

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_repo, logger.clone())
        .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
        .into_result_failing_non_fatal()?;

    if !args.skip_policies {
        let policy_engine = init_policy_engine(&registry_repo, &args.policies, false)?;
        check_policy(&policy_engine, &semconv_specs)
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    logger.success(&format!(
                        "All `before_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    logger.success("No `before_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(&mut diag_msgs)?;
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;

    // Serialize the resolved schema and write it
    // to a file or print it to stdout.
    let registry = ResolvedRegistry::try_from_resolved_registry(&schema.registry, schema.catalog())
        .unwrap_or_else(|e| panic!("Failed to create the registry without catalog: {e:?}"));

    apply_format(&args.format, &registry)
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

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: args.output.is_none(),
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::format::Format;
    use crate::registry::resolve::RegistryResolveArgs;
    use crate::registry::{RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand};
    use crate::run_command;

    #[test]
    fn test_registry_resolve() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Resolve(RegistryResolveArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                    },
                    lineage: true,
                    output: None,
                    format: Format::Yaml,
                    policies: vec![],
                    skip_policies: true,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Now, let's run the command again with the policy checks enabled.
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Resolve(RegistryResolveArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                    },
                    lineage: true,
                    output: None,
                    format: Format::Json,
                    policies: vec![],
                    skip_policies: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }
}
