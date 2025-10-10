// SPDX-License-Identifier: Apache-2.0

//! Emit a semantic convention registry to an OTLP receiver.

use clap::Args;

use log::info;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::log_success;
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

    /// Endpoint for the OTLP receiver. OTEL_EXPORTER_OTLP_ENDPOINT env var will override this.
    #[arg(long, default_value = weaver_emit::DEFAULT_OTLP_ENDPOINT)]
    endpoint: String,
}

/// Emit all spans in the resolved registry.
pub(crate) fn command(args: &RegistryEmitArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Weaver Registry Emit");
    info!("Resolving registry `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) = prepare_main_registry(&args.registry, &args.policy, &mut diag_msgs)?;

    info!("Emitting registry `{}`", args.registry.registry);

    let exporter_config = if args.stdout {
        ExporterConfig::Stdout
    } else {
        ExporterConfig::Otlp {
            endpoint: args.endpoint.clone(),
        }
    };

    // Emit the resolved registry - exit early if there are any errors.
    emit(
        &registry,
        &args.registry.registry.to_string(),
        &exporter_config,
    )
    .combine_diag_msgs_with(&diag_msgs)?;

    log_success(format!("Emitted registry `{}`", args.registry.registry));

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: Some(diag_msgs),
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::registry::emit::RegistryEmitArgs;
    use crate::registry::{PolicyArgs, RegistryArgs, RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_registry_emit() {
        let cli = Cli {
            debug: 1,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Emit(RegistryEmitArgs {
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_emit/data/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        auth_token: None,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                    stdout: true,
                    endpoint: "".to_owned(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
