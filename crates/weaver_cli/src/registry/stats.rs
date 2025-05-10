// SPDX-License-Identifier: Apache-2.0

//! Weaver registry stats sub-command.

use crate::registry::RegistryArgs;
use crate::DiagnosticArgs;
use clap::Args;

/// Parameters for the `registry stats` sub-command
#[derive(Debug, Args)]
pub struct RegistryStatsArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
