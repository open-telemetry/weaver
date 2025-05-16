// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use crate::CmdResult;
use weaver_cli::registry::{RegistryCommand, RegistrySubCommand};

mod check;
mod diff;
mod emit;
mod generate;
mod json_schema;
mod live_check;
mod otlp;
mod resolve;
mod search;
mod stats;
mod update_markdown;

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
    }
}
