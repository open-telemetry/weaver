// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use clap::{Args, Subcommand};

use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use check::RegistryCheckArgs;
use weaver_cache::Cache;
use weaver_logger::Logger;

use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;

mod check;
mod generate;
mod resolve;
mod search;
mod stats;
mod update_markdown;

/// Parameters for the `registry` command
#[derive(Debug, Args)]
pub struct RegistryCommand {
    /// Define the sub-commands for the `registry` command
    #[clap(subcommand)]
    pub command: RegistrySubCommand,
}

/// Sub-commands to manage a `registry`.
#[derive(Debug, Subcommand)]
pub enum RegistrySubCommand {
    /// Validates a registry (i.e., parsing, resolution of references, extends clauses, and constraints).
    Check(RegistryCheckArgs),
    /// Generates artifacts from a registry.
    Generate(RegistryGenerateArgs),
    /// Resolves a registry.
    Resolve(RegistryResolveArgs),
    /// Searches a registry (not yet implemented).
    Search(RegistrySearchArgs),
    /// Calculate and display a set of general statistics on a registry (not yet implemented).
    Stats(RegistryStatsArgs),
    /// Update markdown files that contain markers indicating the templates used to update the specified sections.
    UpdateMarkdown(RegistryUpdateMarkdownArgs),
}

/// Set of parameters used to specify a semantic convention registry.
#[derive(Args, Debug)]
pub struct RegistryArgs {
    /// Local path or Git URL of the semantic convention registry.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: String,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,
}

/// Manage a semantic convention registry.
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        #[allow(clippy::exit)]  // Expected behavior
        std::process::exit(1);
    });

    match &command.command {
        RegistrySubCommand::Check(args) => check::command(log, &cache, args),
        RegistrySubCommand::Generate(args) => generate::command(log, &cache, args),
        RegistrySubCommand::Stats(args) => stats::command(log, &cache, args),
        RegistrySubCommand::Resolve(args) => resolve::command(log, &cache, args),
        RegistrySubCommand::Search(_) => {
            unimplemented!()
        }
        RegistrySubCommand::UpdateMarkdown(args) => update_markdown::command(log, &cache, args),
    }
}
