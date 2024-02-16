// SPDX-License-Identifier: Apache-2.0

//! Command to check a semantic convention registry or a telemetry schema.

mod registry;

use clap::{Args, Subcommand};
use weaver_cache::Cache;
use weaver_logger::Logger;

/// Parameters for the `check` command
#[derive(Debug, Args)]
pub struct CheckCommand {
    /// Define the sub-commands for the `check` command
    #[clap(subcommand)]
    pub command: CheckSubCommand,
}

/// Sub-commands for the `check` command
#[derive(Debug, Subcommand)]
pub enum CheckSubCommand {
    /// Check a semantic convention registry
    Registry(CheckRegistry),
    // ToDo - Add sub-commands for checking telemetry schemas
}


/// Parameters for the `check registry` sub-command
#[derive(Debug, Args)]
pub struct CheckRegistry {
    /// Local path or Git URL of the semantic convention registry to check.
    pub registry: String,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,
}

/// Check a semantic convention registry or a telemetry schema.
pub fn command_check(log: impl Logger + Sync + Clone, command: &CheckCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        std::process::exit(1);
    });

    match &command.command {
        CheckSubCommand::Registry(args) => registry::check_registry_command(log, &cache, args),
    }
}
