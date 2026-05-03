// SPDX-License-Identifier: Apache-2.0

//! Emit a semantic convention registry to an OTLP receiver.

use clap::Args;

use log::info;
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::log_success;
use weaver_emit::{emit, ExporterConfig, RegistryVersion};

use crate::registry::{load_config, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::http_auth::HttpAuthResolver;
use weaver_config::{WeaverCommand, WeaverConfig};

/// Parameters for the `registry emit` sub-command
#[derive(Debug, Args, WeaverCommand)]
#[weaver_command(section = "emit")]
pub struct RegistryEmitArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    #[shared(registry)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    #[shared(policy)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,

    /// Write the telemetry to standard output
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    #[config(default = "false")]
    stdout: Option<bool>,

    /// Endpoint for the OTLP receiver. OTEL_EXPORTER_OTLP_ENDPOINT env var will override this.
    #[arg(long)]
    #[config(default = "http://localhost:4317")]
    endpoint: Option<String>,
}

/// Emit all spans in the resolved registry.
pub(crate) fn command(
    args: &RegistryEmitArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    info!("Weaver Registry Emit");
    info!("Resolving registry `{}`", cmd_config.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();

    let stdout = cmd_config.config.stdout;
    let endpoint = cmd_config.config.endpoint;
    let exporter_config = if stdout {
        ExporterConfig::Stdout
    } else {
        ExporterConfig::Otlp { endpoint }
    };
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;
    match resolved {
        crate::weaver::Resolved::V2(v) => {
            info!("Emitting v2 registry `{}`", cmd_config.registry.registry);
            emit(
                RegistryVersion::V2(v.template_schema()),
                &cmd_config.registry.registry.to_string(),
                &exporter_config,
            )
            .combine_diag_msgs_with(&diag_msgs)?;
        }
        crate::weaver::Resolved::V1(v) => {
            info!("Emitting v1 registry `{}`", cmd_config.registry.registry);
            emit(
                RegistryVersion::V1(v.template_schema()),
                &cmd_config.registry.registry.to_string(),
                &exporter_config,
            )
            .combine_diag_msgs_with(&diag_msgs)?;
        }
    }
    log_success(format!(
        "Emitted registry `{}`",
        cmd_config.registry.registry
    ));

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
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryEmitArgs>();
    }

    #[test]
    fn test_registry_emit() {
        let cli = Cli {
            debug: 1,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            config: None,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Emit(RegistryEmitArgs {
                    registry: RegistryArgs {
                        registry: Some(VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_emit/data/".to_owned(),
                        }),
                        ..Default::default()
                    },
                    policy: PolicyArgs {
                        skip_policies: Some(true),
                        ..Default::default()
                    },
                    diagnostic: Default::default(),
                    stdout: Some(true),
                    endpoint: Some("".to_owned()),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
