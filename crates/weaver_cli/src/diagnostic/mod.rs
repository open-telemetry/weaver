// SPDX-License-Identifier: Apache-2.0

//! The weaver CLI diagnostic command module

pub mod init;

use clap::{Args, Subcommand};

/// Parameters for the `diagnostic` command
#[derive(Debug, Args)]
pub struct DiagnosticCommand {
    /// Define the sub-commands for the `diagnostic` command
    #[clap(subcommand)]
    pub command: DiagnosticSubCommand,
}

/// Sub-commands to manage `diagnostic` messages.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum DiagnosticSubCommand {
    /// Initializes a `diagnostic_templates` directory to define or override diagnostic output
    /// formats.
    Init(init::DiagnosticInitArgs),
}
