// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use clap::Args;
use miette::Diagnostic;
use weaver_checker::PolicyStage;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::registry_path::RegistryPath;
use weaver_semconv::registry_repo::RegistryRepo;
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::{
    check_policy_stage, load_semconv_specs, prepare_main_registry, resolve_semconv_specs,
};
use crate::{DiagnosticArgs, ExitDirectives};

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct RegistryCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: Option<RegistryPath>,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Check a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryCheckArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    logger.log("Weaver Registry Check");
    logger.loading(&format!("Checking registry `{}`", args.registry.registry));

    // Initialize the main registry.
    let registry_path = &args.registry.registry;

    let (main_resolved_registry, mut policy_engine) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

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
            load_semconv_specs(repo, logger.clone(), args.registry.follow_symlinks)
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
                resolve_semconv_specs(&mut baseline_registry, logger.clone())
                    .capture_non_fatal_errors(&mut diag_msgs)?;
            let baseline_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
                &baseline_resolved_schema.registry,
                baseline_resolved_schema.catalog(),
            )
            .combine_diag_msgs_with(&diag_msgs)?;

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
                    logger.success(&format!(
                        "All `comparison_after_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    logger.success("No `comparison_after_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(&mut diag_msgs)?;
        }
    }

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::check::RegistryCheckArgs;
    use crate::registry::{
        semconv_registry, PolicyArgs, RegistryArgs, RegistryCommand, RegistryPath,
        RegistrySubCommand,
    };
    use crate::run_command;

    #[test]
    fn test_registry_check_exit_code() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
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
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                    },
                    baseline_registry: None,
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: false,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }

    #[test]
    fn test_semconv_registry() {
        let logger = TestLogger::new();

        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Check(RegistryCheckArgs {
                registry: RegistryArgs {
                    registry: RegistryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                    },
                    follow_symlinks: false,
                },
                baseline_registry: None,
                policy: PolicyArgs {
                    policies: vec![],
                    skip_policies: false,
                    display_policy_coverage: false,
                },
                diagnostic: Default::default(),
            }),
        };

        let cmd_result = semconv_registry(logger.clone(), &registry_cmd);
        // Violations should be observed.
        assert!(cmd_result.command_result.is_err());
        if let Err(diag_msgs) = cmd_result.command_result {
            assert!(!diag_msgs.is_empty());
            assert_eq!(
                diag_msgs.len(),
                12 /* allow_custom_values */
                + 3 /* missing stability on enum members */
                + 13 /* before resolution */
                    + 3 /* metric after resolution */
                    + 9 /* http after resolution */
            );
        }
    }
}
