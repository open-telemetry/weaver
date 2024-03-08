// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use clap::Args;
use std::path::PathBuf;

/// Parameters for the `registry resolve` sub-command
#[derive(Debug, Args)]
pub struct RegistryResolveArgs {
    /// Local path or Git URL of the semantic convention registry.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: String,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    pub output: Option<PathBuf>,
}
