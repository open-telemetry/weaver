// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use std::path::PathBuf;

use clap::Args;
use weaver_cache::registry_path::RegistryPath;
use weaver_cache::RegistryRepo;
use weaver_checker::PolicyStage;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::error::handle_errors;
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::RegistryArgs;
use crate::util::{
    check_policies, check_policy_stage, init_policy_engine, load_semconv_specs,
    resolve_semconv_specs,
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

    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry.  If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Display the policy coverage report (useful for debugging).
    #[arg(long, default_value = "false")]
    pub display_policy_coverage: bool,

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
    let mut registry_path = args.registry.registry.clone();
    // Support for --registry-git-sub-dir
    // ToDo: This parameter is now deprecated and should be removed in the future
    if let RegistryPath::GitRepo { sub_folder, .. } = &mut registry_path {
        if sub_folder.is_none() {
            sub_folder.clone_from(&args.registry.registry_git_sub_dir);
        }
    }
    let main_registry_repo = RegistryRepo::try_new("main", &registry_path)?;

    // Initialize the baseline registry if provided.
    let baseline_registry_repo = if let Some(baseline_registry) = &args.baseline_registry {
        Some(RegistryRepo::try_new("baseline", baseline_registry)?)
    } else {
        None
    };

    // Load the semantic convention registry into a local registry repo.
    // No parsing errors should be observed.
    let main_semconv_specs = load_semconv_specs(&main_registry_repo, logger.clone())
        .capture_warnings(&mut diag_msgs)
        .into_result()?;
    let baseline_semconv_specs = baseline_registry_repo
        .as_ref()
        .map(|repo| {
            // Baseline registry resolution should allow non-future features
            // and warnings against it should be suppressed when evaluating
            // against it as a "baseline".
            load_semconv_specs(repo, logger.clone())
                .ignore_warnings()
                .into_result()
        })
        .transpose()?;

    let mut policy_engine = if !args.skip_policies {
        Some(init_policy_engine(
            &main_registry_repo,
            &args.policies,
            args.display_policy_coverage,
        )?)
    } else {
        None
    };

    if let Some(policy_engine) = policy_engine.as_ref() {
        // Check the policies against the semantic convention specifications before resolution.
        // All violations should be captured into an ongoing list of diagnostic messages which
        // will be combined with the final result of future stages.
        // `check_policies` either returns `()` or diagnostic messages, and `capture_diag_msgs_into` updates the
        // provided parameters with any diagnostic messages produced by `check_policies`.
        // In this specific case, `capture_diag_msgs_into` returns either `Some(())` or `None`
        // if diagnostic messages have been captured. Therefore, it is acceptable to ignore the result in this
        // particular case.
        _ = check_policies(policy_engine, &main_semconv_specs, logger.clone())
            .capture_diag_msgs_into(&mut diag_msgs);
    }

    let mut main_registry =
        SemConvRegistry::from_semconv_specs(main_registry_repo.id(), main_semconv_specs);
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    let main_resolved_schema = resolve_semconv_specs(&mut main_registry, logger.clone())
        .combine_diag_msgs_with(&diag_msgs)?;

    if let Some(policy_engine) = policy_engine.as_mut() {
        // Convert the resolved schemas into a resolved registry.
        // If there are any policy violations, they should be captured into the ongoing list of
        // diagnostic messages and returned immediately because there is no point in continuing
        // as the registry resolution is a prerequisite for the next stages.
        let main_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
            main_resolved_schema
                .registry(main_registry_repo.id())
                .expect("Failed to get the registry from the resolved schema"),
            main_resolved_schema.catalog(),
        )
        .combine_diag_msgs_with(&diag_msgs)?;

        // Check the policies against the resolved registry (`PolicyState::AfterResolution`).
        let errs = check_policy_stage::<ResolvedRegistry, ()>(
            policy_engine,
            PolicyStage::AfterResolution,
            &registry_path.to_string(),
            &main_resolved_registry,
            &[],
        );
        logger.success(&format!(
            "All `after_resolution` policies checked ({} violations found)",
            errs.len()
        ));

        // Append the policy errors to the ongoing list of diagnostic messages and if there are
        // any errors, return them immediately.
        if let Err(err) = handle_errors(errs) {
            diag_msgs.extend(err.into());
        }

        if let (Some(baseline_registry_repo), Some(baseline_semconv_specs)) =
            (baseline_registry_repo, baseline_semconv_specs)
        {
            let mut baseline_registry = SemConvRegistry::from_semconv_specs(
                baseline_registry_repo.id(),
                baseline_semconv_specs,
            );
            let baseline_resolved_schema =
                resolve_semconv_specs(&mut baseline_registry, logger.clone())
                    .combine_diag_msgs_with(&diag_msgs)?;
            let baseline_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
                baseline_resolved_schema
                    .registry(baseline_registry_repo.id())
                    .expect("Failed to get the registry from the baseline resolved schema"),
                baseline_resolved_schema.catalog(),
            )
            .combine_diag_msgs_with(&diag_msgs)?;

            // Check the policies against the resolved registry (`PolicyState::AfterResolution`).
            let errs = check_policy_stage(
                policy_engine,
                PolicyStage::ComparisonAfterResolution,
                &registry_path.to_string(),
                &main_resolved_registry,
                &[baseline_resolved_registry],
            );
            logger.success(&format!(
                "All `comparison_after_resolution` policies checked ({} violations found)",
                errs.len()
            ));

            // Append the policy errors to the ongoing list of diagnostic messages and if there are
            // any errors, return them immediately.
            if let Err(err) = handle_errors(errs) {
                diag_msgs.extend(err.into());
            }
        }

        if !diag_msgs.is_empty() {
            return Err(diag_msgs);
        }
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
        semconv_registry, RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand,
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
                        registry_git_sub_dir: None,
                    },
                    baseline_registry: None,
                    policies: vec![],
                    skip_policies: true,
                    display_policy_coverage: false,
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
                        registry_git_sub_dir: None,
                    },
                    baseline_registry: None,
                    policies: vec![],
                    skip_policies: false,
                    display_policy_coverage: false,
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
                    registry_git_sub_dir: None,
                },
                baseline_registry: None,
                policies: vec![],
                skip_policies: false,
                display_policy_coverage: false,
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
                13 /* before resolution */
                    + 3 /* metric after resolution */
                    + 9 /* http after resolution */
            );
        }
    }
}
