// SPDX-License-Identifier: Apache-2.0

//! Weaver registry json-schema sub-command.

use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;

/// Parameters for the `registry json-schema` sub-command
#[derive(Debug, Args)]
pub struct RegistryJsonSchemaArgs {
    /// Output file to write the JSON schema to
    /// If not specified, the JSON schema is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
