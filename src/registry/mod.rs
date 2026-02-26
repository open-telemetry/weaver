// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use emit::RegistryEmitArgs;
use miette::Diagnostic;
use serde::Serialize;

use crate::registry::diff::RegistryDiffArgs;
use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::infer::RegistryInferArgs;
use crate::registry::json_schema::RegistryJsonSchemaArgs;
use crate::registry::live_check::RegistryLiveCheckArgs;
use crate::registry::mcp::RegistryMcpArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
use crate::CmdResult;
use check::RegistryCheckArgs;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::vdir::VirtualDirectoryPath;

mod check;
mod diff;
mod emit;
mod generate;
mod infer;
mod json_schema;
mod live_check;
mod mcp;
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

    #[error(transparent)]
    Schema(#[from] weaver_resolved_schema::error::Error),
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
    /// - Resolving references and extends clauses within the specifications.
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
    /// DEPRECATED - Searches a registry. This command is deprecated and will be removed in a future version.
    /// It is not compatible with V2 schema. Please search the generated documentation instead.
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
    /// Generate a diff between two versions of a semantic convention registry.
    ///
    /// This diff can then be rendered in multiple formats:
    /// - a console-friendly format (default: ansi),
    /// - a structured document in JSON format,
    /// - ...
    #[clap(verbatim_doc_comment)]
    Diff(RegistryDiffArgs),

    /// Emits a semantic convention registry as example signals to your OTLP receiver.
    ///
    /// This uses the standard OpenTelemetry SDK, defaulting to OTLP gRPC on localhost:4317.
    #[clap(verbatim_doc_comment)]
    Emit(RegistryEmitArgs),
    /// Perform a live check on sample telemetry by comparing it to a semantic convention registry.
    ///
    /// Includes: Flexible input ingestion, configurable assessment, and template-based output.
    #[clap(verbatim_doc_comment)]
    LiveCheck(RegistryLiveCheckArgs),

    /// Run an MCP (Model Context Protocol) server for the semantic convention registry.
    ///
    /// This server exposes the registry to LLMs, enabling natural language
    /// queries for finding and understanding semantic conventions while writing
    /// instrumentation code.
    ///
    /// The server communicates over stdio using JSON-RPC.
    #[clap(verbatim_doc_comment)]
    Mcp(RegistryMcpArgs),

    /// Generates a schema file by inferring the schema from a OTLP message.
    #[clap(verbatim_doc_comment)]
    Infer(RegistryInferArgs),
}

/// Set of parameters used to specify a semantic convention registry.
#[derive(Args, Debug)]
pub struct RegistryArgs {
    /// Local folder, Git repo URL, or Git archive URL of the semantic
    /// convention registry. For Git URLs, a reference can be specified
    /// using the `@refspec` syntax and a sub-folder can be specified
    /// using the `[sub-folder]` syntax after the URL.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git[model]"
    )]
    pub registry: VirtualDirectoryPath,

    /// Boolean flag to specify whether to follow symlinks when loading the registry.
    /// Default is false.
    #[arg(short = 's', long)]
    pub(crate) follow_symlinks: bool,

    /// Boolean flag to include signals and attributes defined in dependency registries,
    /// even if they are not explicitly referenced in the current (custom) registry.
    #[arg(long)]
    pub(crate) include_unreferenced: bool,

    /// Whether or not to output version 2 of the schema.
    /// Note: this will impact both output to templates *and* policies.
    #[arg(long, default_value = "false")]
    pub v2: bool,
}

/// Set of common parameters used for policy checks.
#[derive(Args, Debug)]
pub struct PolicyArgs {
    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry.  If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<VirtualDirectoryPath>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Display the policy coverage report (useful for debugging).
    #[arg(long, default_value = "false")]
    pub display_policy_coverage: bool,
}

/// Manage a semantic convention registry and return the exit code.
pub fn semconv_registry(command: &RegistryCommand) -> CmdResult {
    match &command.command {
        RegistrySubCommand::Check(args) => {
            CmdResult::new(check::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Generate(args) => {
            CmdResult::new(generate::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Stats(args) => {
            CmdResult::new(stats::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Resolve(args) => {
            CmdResult::new(resolve::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Search(args) => {
            CmdResult::new(search::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::UpdateMarkdown(args) => CmdResult::new(
            update_markdown::command(args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::JsonSchema(args) => {
            CmdResult::new(json_schema::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Diff(args) => {
            CmdResult::new(diff::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::LiveCheck(args) => {
            CmdResult::new(live_check::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Emit(args) => {
            CmdResult::new(emit::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Mcp(args) => {
            CmdResult::new(mcp::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Infer(args) => {
            CmdResult::new(infer::command(args), Some(args.diagnostic.clone()))
        }
    }
}
