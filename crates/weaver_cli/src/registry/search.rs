// SPDX-License-Identifier: Apache-2.0

//! Weaver registry search sub-command.

use crate::registry::RegistryArgs;
use crate::DiagnosticArgs;
use clap::Args;

/// Parameters for the `registry search` sub-command
#[derive(Debug, Args)]
pub struct RegistrySearchArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Flag to indicate if lineage information should be included in the
    /// resolved schema (not yet implemented)
    #[arg(long, default_value = "false")]
    lineage: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// An (optional) search string to use.  If specified, will return matching values on the command line.
    /// Otherwise, runs an interactive terminal UI.
    pub search_string: Option<String>,
}
