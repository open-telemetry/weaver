// SPDX-License-Identifier: Apache-2.0

//! Check a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use weaver_cache::Cache;
use weaver_checker::PolicyStage;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::error::handle_errors;
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::registry::SemConvRegistry;

use crate::{DiagnosticArgs, ExitDirectives};
use crate::registry::RegistryArgs;
use crate::util::{check_policies, check_policy_stage, init_policy_engine, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from};

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct RegistryCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Optional list of policy files to check against the files of the semantic
    /// convention registry.
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
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryCheckArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    logger.loading(&format!("Checking registry `{}`", args.registry.registry));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;
    let mut policy_engine = if !args.skip_policies {
        Some(init_policy_engine(
            &registry_path,
            cache,
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
        _ = check_policies(
            policy_engine,
            &semconv_specs,
            logger.clone(),
        ).capture_diag_msgs_into(&mut diag_msgs);
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    let resolved_schema = resolve_semconv_specs(&mut registry, logger.clone())
        .combine_diag_msgs_with(&diag_msgs)?;

    if let Some(policy_engine) = policy_engine.as_mut() {
        // Convert the resolved schemas into a resolved registry.
        // If there are any policy violations, they should be captured into the ongoing list of
        // diagnostic messages and returned immediately because there is no point in continuing
        // as the registry resolution is a prerequisite for the next stages.
        let resolved_registry = ResolvedRegistry::try_from_resolved_registry(
            resolved_schema
                .registry(registry_id)
                .expect("Failed to get the registry from the resolved schema"),
            resolved_schema.catalog(),
        ).combine_diag_msgs_with(&diag_msgs)?;

        // Check the policies against the resolved registry (`PolicyState::AfterResolution`).
        let errs = check_policy_stage(
            policy_engine,
            PolicyStage::AfterResolution,
            &registry_path.to_string(),
            &resolved_registry,
        );

        // Append the policy errors to the ongoing list of diagnostic messages and if there are
        // any errors, return them immediately.
        if let Err(err) = handle_errors(errs) {
            diag_msgs.extend(err.into());
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
    use crate::registry::{RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand, semconv_registry};
    use crate::registry::check::RegistryCheckArgs;
    use crate::run_command;

    #[test]
    fn test_registry_check_exit_code() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
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
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Check(RegistryCheckArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
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
                    registry: RegistryPath::Local(
                        "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                    ),
                    registry_git_sub_dir: None,
                },
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
            assert_eq!(diag_msgs.len(),
                       13 /* before resolution */
                           + 3 /* metric after resolution */
                           + 9 /* http after resolution */);
        }
    }
}
