// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use clap::{Args, Subcommand};
use std::fmt::Display;
use std::str::FromStr;

use check::RegistryCheckArgs;
use weaver_cache::Cache;
use weaver_logger::Logger;

use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
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

/// Path to a semantic convention registry.
/// The path can be a local directory or a Git URL.
#[derive(Debug, Clone)]
pub enum RegistryPath {
    Local(String),
    Url(String),
}

/// Implement the `FromStr` trait for `RegistryPath`, so that it can be used as
/// a command-line argument.
impl FromStr for RegistryPath {
    type Err = String;

    /// Parse a string into a `RegistryPath`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(Self::Url(s.to_string()))
        } else {
            Ok(Self::Local(s.to_string()))
        }
    }
}

/// Implement the `Display` trait for `RegistryPath`, so that it can be printed
/// to the console.
impl Display for RegistryPath {
    /// Format the `RegistryPath` as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryPath::Local(path) => write!(f, "{}", path),
            RegistryPath::Url(url) => write!(f, "{}", url),
        }
    }
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
    pub registry: RegistryPath,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,
}

/// Manage a semantic convention registry.
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
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

/// Convert a `RegistryPath` to a `weaver_semconv::path::RegistryPath`.
pub fn semconv_registry_path_from(
    registry: &RegistryPath,
    path: &Option<String>,
) -> weaver_semconv::path::RegistryPath {
    match registry {
        RegistryPath::Local(path) => {
            weaver_semconv::path::RegistryPath::Local { path: path.clone() }
        }
        RegistryPath::Url(url) => weaver_semconv::path::RegistryPath::GitUrl {
            git_url: url.clone(),
            path: path.clone(),
        },
    }
}
