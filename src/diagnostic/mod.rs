// SPDX-License-Identifier: Apache-2.0

//! Command to manage diagnostic messages

mod init;

use crate::CmdResult;
use clap::{Args, Subcommand};
use include_dir::{include_dir, Dir};
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;

/// Embedded default diagnostic templates
pub(crate) static DEFAULT_DIAGNOSTIC_TEMPLATES: Dir<'_> =
    include_dir!("defaults/diagnostic_templates");

/// Errors emitted by the `diagnostic` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Failed to initialize diagnostic templates
    #[error("Failed to initialize diagnostic templates at {path}: {error}")]
    InitDiagnosticError { path: PathBuf, error: String },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Parameters for the `diagnostic` command
#[derive(Debug, Args)]
pub struct DiagnosticCommand {
    /// Define the sub-commands for the `diagnostic` command
    #[clap(subcommand)]
    pub command: DiagnosticSubCommand,
}

/// Sub-commands to manage `diagnostic` messages.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum DiagnosticSubCommand {
    /// Initializes a `diagnostic_templates` directory to define or override diagnostic output
    /// formats.
    Init(init::DiagnosticInitArgs),
}

/// Manage diagnostic messages.
#[cfg(not(tarpaulin_include))]
pub fn diagnostic(log: impl Logger + Sync + Clone, command: &DiagnosticCommand) -> CmdResult {
    match &command.command {
        DiagnosticSubCommand::Init(args) => {
            CmdResult::new(init::command(log, args), Some(args.diagnostic.clone()))
        }
    }
}
