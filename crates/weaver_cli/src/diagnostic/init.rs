// SPDX-License-Identifier: Apache-2.0

//! Module for diagnostic init sub-command.

use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;

/// Parameters for the `diagnostic init` sub-command
#[derive(Debug, Args)]
pub struct DiagnosticInitArgs {
    /// Optional target to initialize the diagnostic templates for. If empty, all default templates will be extracted.
    #[arg(default_value = "")]
    pub target: String,

    /// Optional path where the diagnostic templates directory should be created.
    #[arg(short = 't', long, default_value = "diagnostic_templates")]
    pub diagnostic_templates_dir: PathBuf,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
