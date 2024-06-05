// SPDX-License-Identifier: Apache-2.0

//! Manage command line arguments

use crate::diagnostic::DiagnosticCommand;
use crate::registry::RegistryCommand;
use clap::{Parser, Subcommand};

/// Command line arguments.
#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = None,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(long, action = clap::ArgAction::Count, global = true)]
    pub debug: u8,

    /// Turn the quiet mode on (i.e., minimal output)
    #[arg(long, global = true)]
    pub quiet: bool,

    /// List of supported commands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Supported commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Manage Semantic Convention Registry
    Registry(RegistryCommand),
    /// Manage Diagnostic Messages
    Diagnostic(DiagnosticCommand),
}
