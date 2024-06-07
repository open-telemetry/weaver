// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Subcommand};
use miette::Diagnostic;
use serde::Serialize;

use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::json_schema::RegistryJsonSchemaArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
use crate::CmdResult;
use check::RegistryCheckArgs;
use weaver_cache::Cache;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;

mod check;
mod generate;
mod json_schema;
mod resolve;
mod search;
mod stats;
mod update_markdown;

/// Errors emitted by the `registry` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Invalid parameter passed to the command line
    #[error("The parameter `--param {param}` is invalid. {error}")]
    InvalidParam { param: String, error: String },

    /// Invalid params file passed to the command line
    #[error("The params file `{params_file}` is invalid. {error}")]
    InvalidParams { params_file: PathBuf, error: String },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Parameters for the `registry` command
#[derive(Debug, Args)]
pub struct RegistryCommand {
    /// Define the sub-commands for the `registry` command
    #[clap(subcommand)]
    pub command: RegistrySubCommand,
}

/// Sub-commands to manage a `registry`.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum RegistrySubCommand {
    /// Validates a semantic convention registry.
    ///
    /// The validation process for a semantic convention registry involves several steps:
    /// - Loading the semantic convention specifications from a local directory or a git repository.
    /// - Parsing the loaded semantic convention specifications.
    /// - Resolving references, extends clauses, and constraints within the specifications.
    /// - Checking compliance with specified Rego policies, if provided.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the registry validation is successful.
    #[clap(verbatim_doc_comment)]
    Check(RegistryCheckArgs),
    /// Generates artifacts from a semantic convention registry.
    ///
    /// Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the generation is successful.
    #[clap(verbatim_doc_comment)]
    Generate(RegistryGenerateArgs),
    /// Resolves a semantic convention registry.
    ///
    /// Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the resolution is successful.
    #[clap(verbatim_doc_comment)]
    Resolve(RegistryResolveArgs),
    /// Searches a registry (requires interactive terminal).
    Search(RegistrySearchArgs),
    /// Calculate a set of general statistics on a semantic convention registry.
    Stats(RegistryStatsArgs),
    /// Update markdown files that contain markers indicating the templates used to update the specified sections.
    UpdateMarkdown(RegistryUpdateMarkdownArgs),
    /// Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.
    ///
    /// The produced JSON Schema can be used to generate documentation of the resolved registry format or to generate code in your language of choice if you need to interact with the resolved registry format for any reason.
    #[clap(verbatim_doc_comment)]
    JsonSchema(RegistryJsonSchemaArgs),
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
            Ok(Self::Url(s.to_owned()))
        } else {
            Ok(Self::Local(s.to_owned()))
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

/// Manage a semantic convention registry and return the exit code.
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) -> CmdResult {
    let cache = match Cache::try_new() {
        Ok(cache) => cache,
        Err(e) => return CmdResult::new(Err(e.into()), None),
    };

    match &command.command {
        RegistrySubCommand::Check(args) => CmdResult::new(
            check::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Generate(args) => CmdResult::new(
            generate::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Stats(args) => CmdResult::new(
            stats::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Resolve(args) => CmdResult::new(
            resolve::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Search(args) => CmdResult::new(
            search::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::UpdateMarkdown(args) => CmdResult::new(
            update_markdown::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::JsonSchema(args) => CmdResult::new(
            json_schema::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
    }
}
