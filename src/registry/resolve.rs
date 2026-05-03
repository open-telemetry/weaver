// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use log::info;
use miette::Diagnostic;
use weaver_common::diagnostic::{is_future_mode_enabled, DiagnosticMessage, DiagnosticMessages};
use weaver_forge::{OutputProcessor, OutputTarget};
use weaver_semconv::registry_repo::RegistryRepo;

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::{PolicyError, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::http_auth::HttpAuthResolver;
use weaver_config::{EffectivePolicyConfig, EffectiveRegistryConfig, WeaverConfig};

#[derive(thiserror::Error, Debug, serde::Serialize, Diagnostic)]
enum Error {
    #[error("The 'weaver registry resolve' command is deprecated and will be removed in a future version. Please use 'weaver registry generate' or 'weaver registry package' instead.")]
    Deprecated,
}

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
pub(crate) fn command(
    args: &RegistryResolveArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    // Display deprecation warning
    if is_future_mode_enabled() {
        return Err(DiagnosticMessages::from_error(Error::Deprecated));
    }

    log::warn!("The 'weaver registry resolve' command is deprecated and will be removed in a future version.");
    log::warn!("Please use 'weaver registry generate' or 'weaver registry package' instead.");

    let mut registry = EffectiveRegistryConfig::default();
    if let Some(wc) = cfg {
        registry.layer_config(&wc.registry);
    }
    args.registry.apply_to(&mut registry);

    let mut policy = EffectivePolicyConfig::default();
    if let Some(wc) = cfg {
        policy.layer_config(&wc.policy);
    }
    args.policy.apply_to(&mut policy);

    info!("Resolving registry `{}`", registry.registry);
    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&registry, &policy, auth);
    let registry_path = &registry.registry;

    let mut nfes = vec![];
    let main_registry_repo = RegistryRepo::try_new_with_auth(None, registry_path, &mut nfes, auth)?;

    diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

    let loaded = weaver.load_definitions(main_registry_repo, &mut diag_msgs)?;
    // TODO - only do this in weaver check?
    if registry.v2 {
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

    resolved.check_after_resolution_policy(&mut diag_msgs)?;
    match &resolved {
        crate::weaver::Resolved::V1(v) => output.generate(v.template_schema()),
        crate::weaver::Resolved::V2(v) => output.generate(v.template_schema()),
    }
    .map_err(DiagnosticMessages::from)?;

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
            allow_git_credentials: false,
            config: None,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Resolve(RegistryResolveArgs {
                    registry: RegistryArgs {
                        registry: Some(VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        }),
                        ..Default::default()
                    },
                    lineage: true,
                    output: None,
                    format: "yaml".to_owned(),
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
                command: RegistrySubCommand::Resolve(RegistryResolveArgs {
                    registry: RegistryArgs {
                        registry: Some(VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        }),
                        ..Default::default()
                    },
                    lineage: true,
                    output: None,
                    format: "json".to_owned(),
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
}
