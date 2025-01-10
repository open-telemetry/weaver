// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.

mod infer;
mod check;

use clap::{Args, Subcommand};
use miette::Diagnostic;
use serde::Serialize;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;
use crate::CmdResult;
use crate::otlp_receiver::check::CheckRegistryArgs;
use crate::otlp_receiver::infer::InferRegistryArgs;

/// Errors emitted by the `otlp-receiver` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Parameters for the `otlp-receiver` command
#[derive(Debug, Args)]
pub struct OtlpReceiverCommand {
    /// Define the sub-commands for the `otlp-receiver` command
    #[clap(subcommand)]
    pub command: OtlpReceiverSubCommand,
}

/// Sub-commands to manage a `otlp-receiver`.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum OtlpReceiverSubCommand {
    /// Infer a semantic convention registry from an OTLP traffic.
    #[clap(verbatim_doc_comment)]
    InferRegistry(InferRegistryArgs),
    /// Detect the gap between a semantic convention registry and an OTLP traffic.
    #[clap(verbatim_doc_comment)]
    CheckRegistry(CheckRegistryArgs),
}

/// Start the OTLP receiver and process the sub-command.
pub fn otlp_receiver(log: impl Logger + Sync + Clone, command: &OtlpReceiverCommand) -> CmdResult {
    match &command.command {
        OtlpReceiverSubCommand::InferRegistry(args) => CmdResult::new(
            infer::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        OtlpReceiverSubCommand::CheckRegistry(args) => CmdResult::new(
            check::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
    }
}