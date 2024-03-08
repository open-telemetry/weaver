// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use clap::{Args, Subcommand};

use crate::registry::generate::GenerateRegistry;
use crate::registry::resolve::ResolveRegistry;
use crate::registry::search::SearchRegistry;
use check::CheckRegistry;
use weaver_cache::Cache;
use weaver_logger::Logger;

use crate::registry::stats::StatsRegistry;

mod check;
mod generate;
mod resolve;
mod search;
mod stats;

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
    Check(CheckRegistry),
    /// Generates documentation or code for a registry (not yet implemented).
    Generate(GenerateRegistry),
    /// Resolves a registry (not yet implemented).
    Resolve(ResolveRegistry),
    /// Searches a registry (not yet implemented).
    Search(SearchRegistry),
    /// Calculate and display a set of general statistics on a registry (not yet implemented).
    Stats(StatsRegistry),
}

/// Manage a semantic convention registry.
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        std::process::exit(1);
    });

    match &command.command {
        RegistrySubCommand::Check(args) => check::check_registry_command(log, &cache, args),
        RegistrySubCommand::Generate(_) => {
            unimplemented!()
        }
        RegistrySubCommand::Stats(args) => stats::stats_registry_command(log, &cache, args),
        RegistrySubCommand::Resolve(_) => {
            unimplemented!()
        }
        RegistrySubCommand::Search(_) => {
            unimplemented!()
        }
    }
}
