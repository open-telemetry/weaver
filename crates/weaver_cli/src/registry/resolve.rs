// SPDX-License-Identifier: Apache-2.0

//! Weaver registry resolve sub-command.

use crate::format::Format;
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;

/// Parameters for the `registry resolve` sub-command
#[derive(Debug, Args)]
pub struct RegistryResolveArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Flag to indicate if lineage information should be included in the
    /// resolved schema (not yet implemented)
    #[arg(long, default_value = "false")]
    pub lineage: bool,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format for the resolved schema
    /// If not specified, the resolved schema is printed in YAML format
    /// Supported formats: `yaml`, `json`
    /// Default format: `yaml`
    /// Example: `--format json`
    #[arg(short, long, default_value = "yaml")]
    pub format: Format,

    /// Policy parameters
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
