// SPDX-License-Identifier: Apache-2.0

//! Resolve a semantic convention registry.

use clap::Args;
use std::path::PathBuf;

/// Parameters for the `registry resolve` sub-command
#[derive(Debug, Args)]
pub struct ResolveRegistry {
    /// Registry to resolve
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,

    /// Output file to write the resolved schema to
    /// If not specified, the resolved schema is printed to stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}
