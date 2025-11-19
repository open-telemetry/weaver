// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::{
    check_policy_stage, load_semconv_specs, prepare_main_registry_v2, resolve_semconv_specs,
};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use log::info;
use miette::Diagnostic;
use weaver_checker::PolicyStage;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::log_success;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::registry_repo::RegistryRepo;

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct RegistryCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: Option<VirtualDirectoryPath>,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Check a semantic convention registry.
pub(crate) fn command(args: &RegistryCheckArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    info!("Weaver Registry Check");
    info!("Checking registry `{}`", args.registry.registry);

    // Initialize the main registry.
    let registry_path = &args.registry.registry;

    let (main_resolved_registry, v2_registry, mut policy_engine) =
        prepare_main_registry_v2(&args.registry, &args.policy, &mut diag_msgs)?;

    // Initialize the baseline registry if provided.
    let baseline_registry_repo = if let Some(baseline_registry) = &args.baseline_registry {
        Some(RegistryRepo::try_new("baseline", baseline_registry)?)
    } else {
        None
    };

    let baseline_semconv_specs = baseline_registry_repo
        .as_ref()
        .map(|repo| {
            // Baseline registry resolution should allow non-future features
            // and warnings against it should be suppressed when evaluating
            // against it as a "baseline".
            load_semconv_specs(repo, args.registry.follow_symlinks)
                .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
                .capture_non_fatal_errors(&mut diag_msgs)
        })
        .transpose()?;

    if let Some(policy_engine) = policy_engine.as_mut() {
        if let (Some(baseline_registry_repo), Some(baseline_semconv_specs)) =
            (baseline_registry_repo, baseline_semconv_specs)
        {
            let mut baseline_registry = SemConvRegistry::from_semconv_specs(
                &baseline_registry_repo,
                baseline_semconv_specs,
            )?;
            let baseline_resolved_schema =
                resolve_semconv_specs(&mut baseline_registry, args.registry.include_unreferenced)
                    .capture_non_fatal_errors(&mut diag_msgs)?;
            let baseline_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
                &baseline_resolved_schema.registry,
                baseline_resolved_schema.catalog(),
            )
            .combine_diag_msgs_with(&diag_msgs)?;

            // TODO - This is quite an ugly way to handle v2 vs. v1, see if we can refactor.
            if args.policy.policy_use_v2 {
                // TODO - Fix error passing here so original error is a diagnostic or we can convert to something reasonable.
                let v2_baseline_schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
                    baseline_resolved_schema.try_into().map_err(
                        |e: weaver_resolved_schema::error::Error| {
                            weaver_forge::error::Error::TemplateEngineError {
                                error: e.to_string(),
                            }
                        },
                    )?;
                let v2_baseline_resolved_registry =
                    weaver_forge::v2::registry::ForgeResolvedRegistry::try_from_resolved_schema(
                        v2_baseline_schema,
                    )?;
                check_policy_stage(
                    policy_engine,
                    PolicyStage::ComparisonAfterResolution,
                    &registry_path.to_string(),
                    &v2_registry,
                    &[v2_baseline_resolved_registry],
                )
                .inspect(|_, violations| {
                    if let Some(violations) = violations {
                        log_success(format!(
                            "All `comparison_after_resolution` policies checked ({} violations found)",
                            violations.len()
                        ));
                    } else {
                        log_success("No `comparison_after_resolution` policy violation");
                    }
                })
                .capture_non_fatal_errors(&mut diag_msgs)?;
            } else {
                // Check the policies against the resolved registry (`PolicyState::ComparisonAfterResolution`).
                check_policy_stage(
                    policy_engine,
                    PolicyStage::ComparisonAfterResolution,
                    &registry_path.to_string(),
                    &main_resolved_registry,
                    &[baseline_resolved_registry],
                )
                .inspect(|_, violations| {
                    if let Some(violations) = violations {
                        log_success(format!(
                            "All `comparison_after_resolution` policies checked ({} violations found)",
                            violations.len()
                        ));
                    } else {
                        log_success("No `comparison_after_resolution` policy violation");
                    }
                })
                .capture_non_fatal_errors(&mut diag_msgs)?;
            }
        }
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
    use assert_cmd::assert;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_registry_check_exit_code() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                        policy_use_v2: false,
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
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: false,
                        display_policy_coverage: false,
                        policy_use_v2: false,
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
                    registry: VirtualDirectoryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                    },
                    follow_symlinks: false,
                    include_unreferenced: false,
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    policies: vec![],
                    skip_policies: false,
                    display_policy_coverage: false,
                    policy_use_v2: false,
                },
                diagnostic: Default::default(),
            }),
        };

        let cmd_result = semconv_registry(&registry_cmd);
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
                    registry: VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_check/".to_owned(),
                    },
                    follow_symlinks: false,
                    include_unreferenced: false,
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    policies: vec![],
                    skip_policies: false,
                    display_policy_coverage: true,
                    policy_use_v2: true,
                },
                diagnostic: Default::default(),
            }),
        };
        let cmd_result = semconv_registry(&registry_cmd);
        // V2 Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert!(diag_msgs
                .clone()
                .into_inner()
                .iter()
                .find(|msg| format!(
                    "{msg:?}").contains("invalid_metric_attr")
                )
                .is_some());
            assert_eq!(
                diag_msgs.len(),
                1 /* Unstable file version */
                + 1 /* post-resoluton metric error */
            );
        }
    }

    #[test]
    fn test_v2_baseline_policies() {
        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_check_baseline/next/".to_owned(),
                    },
                    follow_symlinks: false,
                    include_unreferenced: false,
                },
                baseline_registry: Some(VirtualDirectoryPath::LocalFolder {
                    path: "tests/v2_check_baseline/base".to_owned(),
                }),
                policy: PolicyArgs {
                    policies: vec![],
                    skip_policies: false,
                    display_policy_coverage: false,
                    policy_use_v2: true,
                },
                diagnostic: Default::default(),
            }),
        };
        let cmd_result = semconv_registry(&registry_cmd);
        // V2 Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert!(diag_msgs
                .clone()
                .into_inner()
                .iter()
                .find(|msg| format!(
                    "{msg:?}").contains("cannot change required/recommended attributes")
                )
                .is_some());
            assert_eq!(
                diag_msgs.len(),
                1 /* Unstable file version */
                + 1 /* baseline error checking */
            );
        }
    }
}
