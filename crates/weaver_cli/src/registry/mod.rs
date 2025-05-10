// SPDX-License-Identifier: Apache-2.0

//! Weaver registry sub-command.

use crate::registry::check::RegistryCheckArgs;
use crate::registry::diff::RegistryDiffArgs;
use crate::registry::emit::RegistryEmitArgs;
use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::json_schema::RegistryJsonSchemaArgs;
use crate::registry::live_check::RegistryLiveCheckArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
use clap::{Args, Subcommand};
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::vdir::VirtualDirectoryPath;

pub mod check;
pub mod diff;
pub mod emit;
pub mod generate;
pub mod json_schema;
pub mod live_check;
pub mod resolve;
pub mod search;
pub mod stats;
pub mod update_markdown;

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
    pub registry: VirtualDirectoryPath,

    /// Boolean flag to specify whether to follow symlinks when loading the registry.
    /// Default value: `false`.
    #[arg(short = 's', long)]
    pub follow_symlinks: bool,

    /// Boolean flag to include signals and attributes defined in dependency registries,
    /// even if they are not explicitly referenced in the current (custom) registry.
    #[arg(long)]
    pub include_unreferenced: bool,
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

/// Errors emitted by the `registry` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Invalid parameter passed to the command line
    #[error("The parameter `--param {param}` is invalid. {error}")]
    InvalidParam {
        /// The parameter passed to the command line
        param: String,
        /// The error message
        error: String,
    },

    /// Invalid params file passed to the command line
    #[error("The params file `{params_file}` is invalid. {error}")]
    InvalidParams {
        /// The params file passed to the command line
        params_file: PathBuf,
        /// The error message
        error: String,
    },

    /// Failed to render the registry diff
    #[error("Failed to render the registry diff: {error}")]
    DiffRender {
        /// The error message
        error: String,
    },
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
    /// Rego policies present in the registry or specified using `-p` or `--policy` will be automatically validated by the policy engine before the artifact generation phase.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the generation is successful.
    #[clap(verbatim_doc_comment)]
    Generate(RegistryGenerateArgs),
    /// Resolves a semantic convention registry.
    ///
    /// Rego policies present in the registry or specified using `-p` or `--policy` will be automatically validated by the policy engine before the artifact generation phase.
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
    /// Generate a diff between two versions of a semantic convention registry.
    ///
    /// This diff can then be rendered in multiple formats:
    /// - a console-friendly format (default: `ansi`),
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
}
