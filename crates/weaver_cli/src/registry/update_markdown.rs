// SPDX-License-Identifier: Apache-2.0

//! Weaver registry update-markdown sub-command.

use crate::registry::RegistryArgs;
use crate::DiagnosticArgs;
use clap::Args;
use weaver_common::vdir::VirtualDirectoryPath;

/// Parameters for the `registry update-markdown` sub-command
#[derive(Debug, Args)]
pub struct RegistryUpdateMarkdownArgs {
    /// Path to the directory where the markdown files are located.
    pub markdown_dir: String,

    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Whether or not to run updates in dry-run mode.
    #[arg(long, default_value = "false")]
    pub dry_run: bool,

    /// Optional path to the attribute registry.
    /// If provided, all attributes will be linked here.
    #[arg(long)]
    pub attribute_registry_base_url: Option<String>,

    /// Path to the directory where the templates are located.
    /// Default is the `templates` directory.
    /// Note: `registry update-markdown` will look for a specific jinja template:
    ///   `{templates}/{target}/snippet.md.j2.`
    #[arg(short = 't', long, default_value = "templates")]
    pub templates: VirtualDirectoryPath,

    /// If provided, the target to generate snippets with.
    /// Note: `registry update-markdown` will look for a specific jinja template:
    ///   `{templates}/{target}/snippet.md.j2.`
    #[arg(long)]
    pub target: String,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}
