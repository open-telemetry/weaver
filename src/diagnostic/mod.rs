// SPDX-License-Identifier: Apache-2.0

//! Command to manage diagnostic messages

mod init;

use crate::CmdResult;
use include_dir::{include_dir, Dir};
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_cli::diagnostic::{DiagnosticCommand, DiagnosticSubCommand};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
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

/// Manage diagnostic messages.
pub fn diagnostic(command: &DiagnosticCommand) -> CmdResult {
    match &command.command {
        DiagnosticSubCommand::Init(args) => {
            CmdResult::new(init::command(args), Some(args.diagnostic.clone()))
        }
    }
}
