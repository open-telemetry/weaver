// SPDX-License-Identifier: Apache-2.0

//! Compute stats on a semantic convention registry.

use crate::registry::{load_config, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use include_dir::{include_dir, Dir};
use log::info;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_config::{
    excluded_args, override_if_set, CliOverrides, EffectiveDiagnosticConfig,
    EffectiveRegistryConfig, StatsConfig, WeaverConfig,
};
use weaver_forge::{OutputProcessor, OutputTarget};

/// Embedded default stats templates
pub(crate) static DEFAULT_STATS_TEMPLATES: Dir<'_> = include_dir!("defaults/stats_templates");

/// Parameters for the `registry stats` sub-command
#[derive(Debug, Args)]
pub struct RegistryStatsArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Output format for the stats.
    /// Predefined formats are: text, json, yaml, jsonl, mute.
    #[arg(long)]
    format: Option<String>,

    /// Path to the directory where the stats templates are located.
    #[arg(long)]
    templates: Option<PathBuf>,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the stats are printed to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

impl CliOverrides for RegistryStatsArgs {
    type Config = StatsConfig;
    const SUBCOMMAND: &'static str = "stats";

    fn extract_config(weaver_config: &WeaverConfig) -> StatsConfig {
        weaver_config.stats.clone()
    }

    fn excluded_args() -> &'static [&'static str] {
        excluded_args!(RegistryArgs::EXCLUDED_ARGS, DiagnosticArgs::EXCLUDED_ARGS,)
    }

    fn apply_overrides(&self, config: &mut StatsConfig) {
        override_if_set!(config.format, self.format);
        override_if_set!(config.templates, self.templates);
        override_if_set!(config.output, self.output, optional);
    }

    fn apply_registry_overrides(&self, config: &mut EffectiveRegistryConfig) {
        self.registry.apply_to(config);
    }

    fn apply_diagnostic_overrides(&self, config: &mut EffectiveDiagnosticConfig) {
        self.diagnostic.apply_to(config);
    }

    fn uses_policy() -> bool {
        false
    }
}

/// Thin wrapper that adds a `version` field for the template to branch on.
/// The raw stats are flattened into the same JSON level via `#[serde(flatten)]`.
#[derive(Serialize)]
struct StatsContext<T> {
    version: &'static str,
    #[serde(flatten)]
    stats: T,
}

/// Compute stats on a semantic convention registry.
pub(crate) fn command(
    args: &RegistryStatsArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    info!(
        "Compute statistics on the registry `{}`",
        cmd_config.registry.registry
    );

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    let format = &cmd_config.config.format;
    let templates = cmd_config.config.templates;
    let target = OutputTarget::from_optional_dir(cmd_config.config.output.as_ref());
    let mut output = OutputProcessor::new(
        format,
        "stats",
        Some(&DEFAULT_STATS_TEMPLATES),
        Some(templates),
        target,
    )
    .map_err(DiagnosticMessages::from)?;

    match resolved {
        crate::weaver::Resolved::V1(v) => {
            let context = StatsContext {
                version: "v1",
                stats: v.resolved_schema().stats(),
            };
            output
                .generate(&context)
                .map_err(DiagnosticMessages::from)?;
        }
        crate::weaver::Resolved::V2(v) => {
            let context = StatsContext {
                version: "v2",
                stats: v.resolved_schema().stats(),
            };
            output
                .generate(&context)
                .map_err(DiagnosticMessages::from)?;
        }
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::registry::stats::RegistryStatsArgs;

    #[test]
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryStatsArgs>();
    }
}
