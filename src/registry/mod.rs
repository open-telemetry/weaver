// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use miette::Diagnostic;
use serde::Serialize;

use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::json_schema::RegistryJsonSchemaArgs;
use crate::registry::live_check::CheckRegistryArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
use crate::CmdResult;
use check::RegistryCheckArgs;
use weaver_cache::registry_path::RegistryPath;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;

mod check;
mod generate;
mod json_schema;
mod live_check;
mod otlp;
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
    /// Searches a registry (Note: Experimental and subject to change).
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

    /// Check the conformance level of an OTLP stream against a semantic convention registry.
    ///
    /// This command starts an OTLP listener and compares each received OTLP message with the
    /// registry provided as a parameter. When the command is stopped (see stop conditions),
    /// a conformance/coverage report is generated. The purpose of this command is to be used
    /// in a CI/CD pipeline to validate the telemetry stream from an application or service
    /// against a registry.
    ///
    /// The currently supported stop conditions are: CTRL+C (SIGINT), SIGHUP, the HTTP /stop
    /// endpoint, and a maximum duration of no OTLP message reception.
    #[clap(verbatim_doc_comment)]
    LiveCheck(CheckRegistryArgs),
}

/// Set of parameters used to specify a semantic convention registry.
#[derive(Args, Debug)]
pub struct RegistryArgs {
    /// Local folder, Git repo URL, or Git archive URL of the semantic
    /// convention registry. For Git URLs, a sub-folder can be specified
    /// using the `[sub-folder]` syntax after the URL.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git[model]"
    )]
    pub registry: RegistryPath,

    /// Boolean flag to specify whether to follow symlinks when loading the registry.
    /// Default is false.
    #[arg(short = 's', long)]
    pub(crate) follow_symlinks: bool,
}

/// Set of common parameters used for policy checks.
#[derive(Args, Debug)]
pub struct PolicyArgs {
    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry.  If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Display the policy coverage report (useful for debugging).
    #[arg(long, default_value = "false")]
    pub display_policy_coverage: bool,
}

impl PolicyArgs {
    /// Create a new empty `PolicyArgs` with the skip flag set to true.
    pub fn skip() -> Self {
        Self {
            policies: Vec::new(),
            skip_policies: true,
            display_policy_coverage: false,
        }
    }
}

/// Manage a semantic convention registry and return the exit code.
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) -> CmdResult {
    match &command.command {
        RegistrySubCommand::Check(args) => CmdResult::new(
            check::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Generate(args) => CmdResult::new(
            generate::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Stats(args) => CmdResult::new(
            stats::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Resolve(args) => CmdResult::new(
            resolve::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::Search(args) => CmdResult::new(
            search::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::UpdateMarkdown(args) => CmdResult::new(
            update_markdown::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::JsonSchema(args) => CmdResult::new(
            json_schema::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::LiveCheck(args) => CmdResult::new(
            live_check::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
    }
}
