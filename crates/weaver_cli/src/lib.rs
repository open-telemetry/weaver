// SPDX-License-Identifier: Apache-2.0

//! Weaver command and subcommands

use clap::Args;
use std::path::PathBuf;

pub mod cli;
pub mod diagnostic;
pub mod format;
pub mod registry;

/// Set of parameters used to specify the diagnostic format.
#[derive(Args, Debug, Clone)]
pub struct DiagnosticArgs {
    /// Format used to render the diagnostic messages. Predefined formats are: `ansi`, `json`,
    /// `gh_workflow_command`.
    #[arg(long, default_value = "ansi")]
    pub diagnostic_format: String,

    /// Path to the directory where the diagnostic templates are located.
    #[arg(long, default_value = "diagnostic_templates")]
    pub diagnostic_template: PathBuf,

    /// Send the output to stdout instead of stderr.
    #[arg(long)]
    pub diagnostic_stdout: bool,
}

impl Default for DiagnosticArgs {
    fn default() -> Self {
        Self {
            diagnostic_format: "ansi".to_owned(),
            diagnostic_template: PathBuf::from("diagnostic_templates"),
            diagnostic_stdout: false,
        }
    }
}
