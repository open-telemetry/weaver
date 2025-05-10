// SPDX-License-Identifier: Apache-2.0

//! Manage command line arguments

use crate::diagnostic::DiagnosticCommand;
use crate::registry::RegistryCommand;
use clap::{Args, Parser, Subcommand};

/// Manage semantic convention registry and telemetry schema workflows (OpenTelemetry Project)
#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = None,
    subcommand_required = true,
    arg_required_else_help = true,
    bin_name = "weaver"
)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(long, action = clap::ArgAction::Count, global = true)]
    pub debug: u8,

    /// Turn the quiet mode on (i.e., minimal output)
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Enable the most recent validation rules for the semconv registry. It is recommended
    /// to enable this flag when checking a new registry.
    /// Note: `semantic_conventions` main branch should always enable this flag.
    #[arg(long, global = true)]
    pub future: bool,

    /// List of supported commands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Supported commands.
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Manage Semantic Convention Registry
    Registry(RegistryCommand),
    /// Manage Diagnostic Messages
    Diagnostic(DiagnosticCommand),
    /// Generate shell completions
    Completion(CompletionCommand),
}

/// Commands for generating completions and markdown documentation
#[derive(Args)]
pub struct CompletionCommand {
    /// The shell to generate the completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,

    /// (Optional) The file to write the completions to. Defaults to `STDOUT`.
    #[arg(long, hide = true)]
    pub completion_file: Option<std::path::PathBuf>,
}
