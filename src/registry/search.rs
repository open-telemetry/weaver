// SPDX-License-Identifier: Apache-2.0

//! Search a semantic convention registry.

use clap::Args;

/// Parameters for the `registry search` sub-command
#[derive(Debug, Args)]
pub struct SearchRegistry {
    /// Git URL of the semantic convention registry
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,

    /// The telemetry schema containing the versions (url or file)
    #[arg(short, long)]
    schema: Option<String>,
}
