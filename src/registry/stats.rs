// SPDX-License-Identifier: Apache-2.0

//! Compute stats on a semantic convention registry.

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::{ResolvedV2, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use include_dir::{include_dir, Dir};
use log::info;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::DiagnosticMessages;
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
    #[arg(long, default_value = "text")]
    format: String,

    /// Path to the directory where the stats templates are located.
    #[arg(long, default_value = "stats_templates")]
    templates: PathBuf,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the stats are printed to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
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
pub(crate) fn command(args: &RegistryStatsArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!(
        "Compute statistics on the registry `{}`",
        args.registry.registry
    );

    if args.registry.v2 {
        compute_v2(args)?;
    } else {
        compute_v1(args)?;
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

fn compute_v2(args: &RegistryStatsArgs) -> Result<(), DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let policy_config = PolicyArgs {
        policies: vec![],
        skip_policies: true,
        display_policy_coverage: false,
    };
    let weaver = WeaverEngine::new(&args.registry, &policy_config);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;
    let resolved_v2: ResolvedV2 = resolved.try_into()?;

    let target = OutputTarget::from_optional_dir(args.output.as_ref());
    let mut output = OutputProcessor::new(
        &args.format,
        "stats",
        Some(&DEFAULT_STATS_TEMPLATES),
        Some(args.templates.clone()),
        target,
    )
    .map_err(DiagnosticMessages::from)?;

    let context = StatsContext {
        version: "v2",
        stats: resolved_v2.resolved_schema().stats(),
    };
    output
        .generate(&context)
        .map_err(DiagnosticMessages::from)?;
    Ok(())
}

fn compute_v1(args: &RegistryStatsArgs) -> Result<(), DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let policy_config = PolicyArgs {
        policies: vec![],
        skip_policies: true,
        display_policy_coverage: false,
    };
    let weaver = WeaverEngine::new(&args.registry, &policy_config);
    let loaded = weaver.load_main_definitions(&mut diag_msgs)?;

    let resolved = weaver.resolve(loaded, &mut diag_msgs)?;
    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    let target = OutputTarget::from_optional_dir(args.output.as_ref());
    let mut output = OutputProcessor::new(
        &args.format,
        "stats",
        Some(&DEFAULT_STATS_TEMPLATES),
        Some(args.templates.clone()),
        target,
    )
    .map_err(DiagnosticMessages::from)?;

    let context = StatsContext {
        version: "v1",
        stats: resolved.resolved_schema().stats(),
    };
    output
        .generate(&context)
        .map_err(DiagnosticMessages::from)?;
    Ok(())
}
