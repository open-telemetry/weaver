// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use clap::Args;
use weaver_cache::Cache;
use weaver_logger::Logger;

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct RegistryGenerateArgs {
    /// Path to the directory where the templates are located.
    /// Default is the `templates` directory.
    #[arg(default_value = "templates")]
    pub templates: String,

    /// Path to the directory where the generated artifacts will be saved.
    /// Default is the `output` directory.
    #[arg(default_value = "output")]
    pub output: String,

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
}

/// Generate artifacts from a semantic convention registry.
pub(crate) fn command(
    _log: impl Logger + Sync + Clone,
    _cache: &Cache,
    args: &RegistryGenerateArgs,
) {
    println!("Args: {:#?}", args);
}
