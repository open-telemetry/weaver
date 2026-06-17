// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use crate::registry::{load_config, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use log::info;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_config::{WeaverCommand, WeaverConfig};
use weaver_macros::weaver_command;
use weaver_semconv::registry_repo::RegistryRepo;

/// Validate a semantic convention registry against policies and schema rules.
#[weaver_command(section = "check")]
#[derive(Debug, Args, WeaverCommand)]
pub struct RegistryCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    #[shared(registry)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: Option<VirtualDirectoryPath>,

    /// Policy parameters
    #[command(flatten)]
    #[shared(policy)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,
}

/// Check a semantic convention registry.
pub(crate) fn command(
    args: &RegistryCheckArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    let mut diag_msgs = DiagnosticMessages::empty();
    info!("Weaver Registry Check");
    info!("Checking registry `{}`", cmd_config.registry.registry);
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);

    // Initialize the main registry.
    let main_resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    // Initialize the baseline registry if provided.
    let baseline = if let Some(br) = args.baseline_registry.as_ref() {
        // ignore warnings.
        let mut ignored = DiagnosticMessages::empty();
        let registry_repo = RegistryRepo::try_new_with_auth(None, br, &mut vec![], auth)?;
        let loaded = weaver.load_definitions(registry_repo, &mut ignored)?;
        // TODO - do we need to keep any loading diagnostic messages?
        Some(weaver.resolve(loaded, &mut diag_msgs)?)
    } else {
        None
    };

    main_resolved.check_after_resolution_policy(&mut diag_msgs)?;
    // Now the comparison.
    if let Some(b) = baseline {
        main_resolved.check_comparison_after_resolution(&b, &mut diag_msgs)?;
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
    use crate::registry::check::RegistryCheckArgs;
    use crate::registry::{
        semconv_registry, PolicyArgs, RegistryArgs, RegistryCommand, RegistrySubCommand,
    };
    use crate::run_command;
    use weaver_common::http_auth::HttpAuthResolver;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryCheckArgs>();
    }

    #[test]
    fn test_registry_check_exit_code() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            config: None,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: Some(VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        }),
                        ..Default::default()
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        skip_policies: Some(true),
                        ..Default::default()
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
            allow_git_credentials: false,
            config: None,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: Some(VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        }),
                        ..Default::default()
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        ..Default::default()
                    },
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }

    #[test]
    fn test_semconv_registry() {
        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: Some(VirtualDirectoryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                    }),
                    ..Default::default()
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    ..Default::default()
                },
                diagnostic: Default::default(),
            }),
        };

        let cmd_result = semconv_registry(&registry_cmd, None, &HttpAuthResolver::empty());
        // Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert_eq!(
                diag_msgs.len(),
                2 /* legacy template examples format */
                + 3 /* missing stability on enum members */
                + 13 /* before resolution */
                + 3 /* metric after resolution */
                + 9 /* http after resolution */
                + 1 /* deprecated string note */
            );
        }
    }

    #[test]
    fn test_v2_policies() {
        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: Some(VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_check/".to_owned(),
                    }),
                    v2: Some(true),
                    ..Default::default()
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    display_policy_coverage: Some(true),
                    ..Default::default()
                },
                diagnostic: Default::default(),
            }),
        };
        let cmd_result = semconv_registry(&registry_cmd, None, &HttpAuthResolver::empty());
        // V2 Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert!(diag_msgs
                .clone()
                .into_inner()
                .iter()
                .any(|msg| format!("{msg:?}").contains("invalid_metric_attr")));
            assert_eq!(
                diag_msgs.len(),
                1 /* Unstable file version */
                + 1 /* post-resolution metric error */
            );
        }
    }

    #[test]
    fn test_v2_baseline_policies() {
        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: Some(VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_check_baseline/next/".to_owned(),
                    }),
                    v2: Some(true),
                    ..Default::default()
                },
                baseline_registry: Some(VirtualDirectoryPath::LocalFolder {
                    path: "tests/v2_check_baseline/base".to_owned(),
                }),
                policy: PolicyArgs {
                    ..Default::default()
                },
                diagnostic: Default::default(),
            }),
        };
        let cmd_result = semconv_registry(&registry_cmd, None, &HttpAuthResolver::empty());
        // V2 Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert!(diag_msgs
                .clone()
                .into_inner()
                .iter()
                .any(|msg| format!("{msg:?}")
                    .contains("cannot change required/recommended attributes")));
            assert_eq!(
                diag_msgs.len(),
                1 /* Unstable file version */
                + 1 /* baseline error checking */
            );
        }
    }

    #[test]
    fn test_v2_before_resolution_policies() {
        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: Some(VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_check_before_resolution/".to_owned(),
                    }),
                    v2: Some(true),
                    ..Default::default()
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    ..Default::default()
                },
                diagnostic: Default::default(),
            }),
        };
        let cmd_result = semconv_registry(&registry_cmd, None, &HttpAuthResolver::empty());
        // V2 should warn about before_resolution.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert!(diag_msgs
                .clone()
                .into_inner()
                .iter()
                .any(|msg| format!("{msg:?}").contains("is unsupported with V2")));
            assert_eq!(
                diag_msgs.len(),
                1 /* Unstable file version */
                + 1 /* before_resolution warning */
            );
        }
    }
}
