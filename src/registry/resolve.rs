// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use log::info;
use weaver_cli::registry::resolve::RegistryResolveArgs;
use weaver_common::diagnostic::DiagnosticMessages;

use crate::format::apply_format;
use crate::util::prepare_main_registry;
use crate::ExitDirectives;

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
pub(crate) fn command(args: &RegistryResolveArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Resolving registry `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();
    let (registry, _) = prepare_main_registry(&args.registry, &args.policy, &mut diag_msgs)?;

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
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::registry::resolve::RegistryResolveArgs;
    use crate::registry::{RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use weaver_cli::cli::{Cli, Commands};
    use weaver_cli::format::Format;
    use weaver_cli::registry::{PolicyArgs, RegistryArgs};
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
                    },
                    lineage: true,
                    output: None,
                    format: Format::Yaml,
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
                    },
                    lineage: true,
                    output: None,
                    format: Format::Json,
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
