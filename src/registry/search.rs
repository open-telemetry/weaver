// SPDX-License-Identifier: Apache-2.0

//! Search a semantic convention registry.

use crate::registry::RegistryArgs;
use clap::Args;

/// Parameters for the `registry search` sub-command
#[derive(Debug, Args)]
pub struct RegistrySearchArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Flag to indicate if lineage information should be included in the
    /// resolved schema (not yet implemented)
    #[arg(long, default_value = "false")]
    lineage: bool,

    /// The telemetry schema containing the versions (url or file)
    #[arg(short, long)]
    pub schema: Option<String>,
}
