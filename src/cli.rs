// SPDX-License-Identifier: Apache-2.0

//! Manage command line arguments

use crate::diagnostic::DiagnosticCommand;
use crate::registry::RegistryCommand;
use crate::serve::ServeCommand;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

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
    /// Turn debugging information on. Use twice (--debug --debug) for trace-level logs.
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

    /// Allow git credential helpers when cloning registries from private repositories.
    /// By default, git operations are isolated and cannot access global git config
    /// or credential helpers. Enable this flag to authenticate with private registries
    /// using your system's configured git credential helpers (e.g., osxkeychain,
    /// git-credential-manager).
    #[arg(long, global = true)]
    pub allow_git_credentials: bool,

    /// Path to a `.weaver.toml` project config file. When set, skips the
    /// upward-walk discovery from the current working directory.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Directory in which to cache Git registries so they are cloned once and
    /// reused across invocations, instead of re-cloned on every command. Only
    /// a source pinned with `@<refspec>` that resolves to a commit or a tag is
    /// cached; a branch, and a URL with no refspec, keep tracking the remote.
    /// When omitted, git registries are cloned into a throwaway temporary
    /// directory (the default behavior).
    #[arg(long, global = true, value_name = "PATH")]
    pub registry_cache_dir: Option<PathBuf>,

    /// Serve cached Git registries without network access: a cache miss becomes
    /// an error instead of a clone. Does not restrict any other download.
    #[arg(long, global = true, requires = "registry_cache_dir")]
    pub registry_cache_offline: bool,

    /// Re-fetch and atomically replace a cached Git registry entry even on a
    /// cache hit. Ignored when `--registry-cache-offline` is set.
    #[arg(long, global = true, requires = "registry_cache_dir")]
    pub registry_cache_refresh: bool,

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
    /// Start the API server (Experimental)
    Serve(ServeCommand),
    /// Generate markdown help documentation
    #[command(hide = true)]
    MarkdownHelp,
}

#[derive(Args)]
pub struct CompletionCommand {
    /// The shell to generate the completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,

    /// (Optional) The file to write the completions to. Defaults to STDOUT.
    #[arg(long, hide = true)]
    pub completion_file: Option<PathBuf>,
}
