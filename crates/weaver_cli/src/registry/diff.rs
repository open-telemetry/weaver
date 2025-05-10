// SPDX-License-Identifier: Apache-2.0

//! Weaver registry diff sub-command.

use crate::registry::RegistryArgs;
use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;
use weaver_common::vdir::VirtualDirectoryPath;

/// Parameters for the `registry diff` sub-command
#[derive(Debug, Args)]
pub struct RegistryDiffArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    pub baseline_registry: VirtualDirectoryPath,

    /// Format used to render the schema changes. Predefined formats are: `ansi`, `json`,
    /// and `markdown`.
    #[arg(long, default_value = "ansi")]
    pub diff_format: String,

    /// Path to the directory where the schema changes templates are located.
    #[arg(long, default_value = "diff_templates")]
    pub diff_template: PathBuf,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the diff report is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
