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
use weaver_config::{WeaverCommand, WeaverConfig};
use weaver_forge::{OutputProcessor, OutputTarget};

/// Embedded default stats templates
pub(crate) static DEFAULT_STATS_TEMPLATES: Dir<'_> = include_dir!("defaults/stats_templates");

/// Compute and display statistics about a semantic convention registry.
#[derive(Debug, Args, WeaverCommand)]
#[weaver_command(section = "stats", no_policy)]
pub struct RegistryStatsArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    #[shared(registry)]
    registry: RegistryArgs,

    /// Output format for the stats.
    /// Predefined formats are: text, json, yaml, jsonl, mute.
    #[arg(long)]
    #[config(default = "text")]
    format: Option<String>,

    /// Path to the directory where the stats templates are located.
    #[arg(long)]
    #[config(default = "stats_templates")]
    templates: Option<PathBuf>,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the stats are printed to stdout.
    #[arg(short, long)]
    #[config]
    output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,
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
    info!("Weaver Registry Stats");
    info!(
        "Computing stats for registry `{}`",
        cmd_config.registry.registry
    );

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    if diag_msgs.has_error() {
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
    )?;

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
