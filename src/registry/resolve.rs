// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use log::info;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::{OutputProcessor, OutputTarget};
use weaver_semconv::registry_repo::RegistryRepo;

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::{PolicyError, ResolvedV2, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};

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
    /// Supported formats: yaml, json, jsonl, mute
    /// Default format: yaml
    /// Example: `--format json`
    #[arg(short, long, default_value = "yaml")]
    format: String,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
pub(crate) fn command(args: &RegistryResolveArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Resolving registry `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&args.registry, &args.policy);
    let registry_path = &args.registry.registry;

    let mut nfes = vec![];
    let main_registry_repo = RegistryRepo::try_new(None, registry_path, &mut nfes)?;

    diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

    let loaded = weaver.load_definitions(main_registry_repo, &mut diag_msgs)?;
    // TODO - only do this in weaver check?
    if args.registry.v2 {
        // Issue a warning so we fail --future.
        if loaded.has_before_resolution_policy() {
            diag_msgs.extend(PolicyError::BeforeResolutionUnsupported.into());
        }
    } else {
        loaded.check_before_resolution_policy(&mut diag_msgs)?;
    }
    let resolved = weaver.resolve(loaded, &mut diag_msgs)?;

    let target = OutputTarget::from_optional_file(args.output.as_ref());
    let mut output = OutputProcessor::new(&args.format, "resolved_registry", None, None, target)
        .map_err(DiagnosticMessages::from)?;

    if args.registry.v2 {
        let resolved_v2: ResolvedV2 = resolved.try_into()?;
        resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;
        output
            .generate(&resolved_v2.template_schema())
            .map_err(DiagnosticMessages::from)?;
    } else {
        resolved.check_after_resolution_policy(&mut diag_msgs)?;
        output
            .generate(&resolved.template_schema())
            .map_err(DiagnosticMessages::from)?;
    }

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::registry::resolve::RegistryResolveArgs;
    use crate::registry::{PolicyArgs, RegistryArgs, RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_registry_resolve() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Resolve(RegistryResolveArgs {
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    lineage: true,
                    output: None,
                    format: "yaml".to_owned(),
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
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
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    lineage: true,
                    output: None,
                    format: "json".to_owned(),
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: false,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }
}
