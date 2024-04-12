// SPDX-License-Identifier: Apache-2.0

//! Update markdown files that contain markers indicating the templates used to
//! update the specified sections.

use crate::error::ExitIfError;
use crate::registry::{semconv_registry_path_from, RegistryPath};
use clap::Args;
use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_semconv_gen::{update_markdown, ResolvedSemconvRegistry};

/// Parameters for the `registry update-markdown` sub-command
#[derive(Debug, Args)]
pub struct RegistryUpdateMarkdownArgs {
    /// Path to the directory where the markdown files are located.
    pub markdown_dir: String,

    /// Local path or Git URL of the semantic convention registry to check.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: RegistryPath,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,

    /// Whether or not to run updates in dry-run mode.
    #[arg(long, default_value = "false")]
    pub dry_run: bool,

    /// Optional path to the attribute registry.
    /// If provided, all attributes will be linked here.
    #[arg(long)]
    pub attribute_registry_base_url: Option<String>,
}

/// Update markdown files.
pub(crate) fn command(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryUpdateMarkdownArgs,
) {
    fn is_markdown(entry: &walkdir::DirEntry) -> bool {
        let path = entry.path();
        let extension = path.extension().unwrap_or_else(|| std::ffi::OsStr::new(""));
        path.is_file() && extension == "md"
    }

    let registry = ResolvedSemconvRegistry::try_from_url(
        semconv_registry_path_from(&args.registry, &args.registry_git_sub_dir),
        cache,
    )
    .exit_if_error(|e| {
        log.error("Failed to resolve the semantic convention registry");
        log.error(&e.to_string());
    });
    log.success("Registry resolved successfully");
    let operation = if args.dry_run {
        "Validating"
    } else {
        "Updating"
    };
    let mut has_error = false;
    for entry in walkdir::WalkDir::new(args.markdown_dir.clone())
        .into_iter()
        .filter_map(|e| match e {
            Ok(v) if is_markdown(&v) => Some(v),
            _ => None,
        })
    {
        log.info(&format!("{}: ${}", operation, entry.path().display()));
        if let Err(error) = update_markdown(
            &entry.path().display().to_string(),
            &registry,
            args.dry_run,
            args.attribute_registry_base_url.as_deref(),
        ) {
            has_error = true;
            log.error(&format!("{error}"));
        }
    }
    if has_error {
        panic!("weaver registry update-markdown failed.");
    }
}
