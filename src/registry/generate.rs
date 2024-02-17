// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use clap::Args;

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct GenerateRegistry {
    /// Registry to resolve
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,
    // ToDo root template directory used to generate the artifacts
}
