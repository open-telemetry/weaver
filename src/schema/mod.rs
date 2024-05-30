// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a telemetry schema.

mod resolve;

use crate::schema::resolve::SchemaResolveArgs;
use crate::CmdResult;
use clap::{Args, Subcommand};
use weaver_cache::Cache;
use weaver_common::Logger;

/// Parameters for the `registry` command
#[derive(Debug, Args)]
pub struct SchemaCommand {
    /// Define the sub-commands for the `schema` command
    #[clap(subcommand)]
    pub command: SchemaSubCommand,
}

/// Sub-commands to manage a `schema`.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum SchemaSubCommand {
    /// Resolves a telemetry schema.
    ///
    /// Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the resolution is successful.
    #[clap(verbatim_doc_comment)]
    Resolve(SchemaResolveArgs),
}

/// Manage a telemetry schema and return the exit code.
pub fn telemetry_schema(log: impl Logger + Sync + Clone, command: &SchemaCommand) -> CmdResult {
    let cache = match Cache::try_new() {
        Ok(cache) => cache,
        Err(e) => return CmdResult::new(Err(e.into()), None),
    };

    match &command.command {
        SchemaSubCommand::Resolve(args) => CmdResult::new(
            resolve::command(log.clone(), &cache, args),
            Some(args.diagnostic.clone()),
        ),
    }
}
