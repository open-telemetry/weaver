// SPDX-License-Identifier: Apache-2.0

//! Weaver registry check sub-command.

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::DiagnosticArgs;
use clap::Args;
use weaver_common::vdir::VirtualDirectoryPath;

/// Parameters for the `registry check` sub-command
#[derive(Debug, Args)]
pub struct RegistryCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    pub baseline_registry: Option<VirtualDirectoryPath>,

    /// Policy parameters
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
