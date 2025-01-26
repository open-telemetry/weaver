// SPDX-License-Identifier: Apache-2.0

//! Emit a semantic convention registry to an OTLP receiver.

use clap::Args;

use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_emit::{emit, ExporterConfig};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

/// Parameters for the `registry emit` sub-command
#[derive(Debug, Args)]
pub struct RegistryEmitArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Write the telemetry to standard output
    #[arg(long)]
    stdout: bool,
}

/// Emit all spans in the resolved registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryEmitArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    if args.stdout {
        logger.mute();
    }
    logger.log("Weaver Registry Emit");
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!("Emitting registry `{}`", args.registry.registry));

    let exporter_config = if args.stdout {
        ExporterConfig::Stdout
    } else {
        ExporterConfig::Otlp
    };

    emit(
        &registry,
        &args.registry.registry.to_string(),
        &exporter_config,
    );

    logger.success(&format!("Emitted registry `{}`", args.registry.registry));

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: args.stdout,
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::emit::RegistryEmitArgs;
    use crate::registry::{
        PolicyArgs, RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand,
    };
    use crate::run_command;

    #[test]
    fn test_registry_emit() {
        let logger = TestLogger::new();

        let cli = Cli {
            debug: 1,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Emit(RegistryEmitArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_emit/data/".to_owned(),
                        },
                        follow_symlinks: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                    stdout: true,
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
