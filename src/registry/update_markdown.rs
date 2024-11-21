// SPDX-License-Identifier: Apache-2.0

//! Update markdown files that contain markers indicating the templates used to
//! update the specified sections.

use crate::registry::RegistryArgs;
use crate::{registry, CommonRegistryArgs, DiagnosticArgs, ExitDirectives};
use clap::Args;
use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::{is_future_mode_enabled, DiagnosticMessages};
use weaver_common::Logger;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::{TemplateEngine, SEMCONV_JQ};
use weaver_semconv_gen::{update_markdown, SnippetGenerator};

/// Parameters for the `registry update-markdown` sub-command
#[derive(Debug, Args)]
pub struct RegistryUpdateMarkdownArgs {
    /// Path to the directory where the markdown files are located.
    pub markdown_dir: String,

    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

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
    ///   {templates}/{target}/snippet.md.j2.
    #[arg(short = 't', long, default_value = "templates")]
    pub templates: String,

    /// If provided, the target to generate snippets with.
    /// Note: `registry update-markdown` will look for a specific jinja template:
    ///   {templates}/{target}/snippet.md.j2.
    #[arg(long)]
    pub target: String,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Weaver parameters
    #[command(flatten)]
    pub common_registry_args: CommonRegistryArgs,
}

/// Update markdown files.
pub(crate) fn command(
    log: impl Logger + Sync + Clone,
    args: &RegistryUpdateMarkdownArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    fn is_markdown(entry: &walkdir::DirEntry) -> bool {
        let path = entry.path();
        let extension = path.extension().unwrap_or_else(|| std::ffi::OsStr::new(""));
        path.is_file() && extension == "md"
    }

    let mut diag_msgs = DiagnosticMessages::empty();

    // Construct a generator if we were given a `--target` argument.
    let generator = {
        let loader = FileSystemFileLoader::try_new(
            format!("{}/registry", args.templates).into(),
            &args.target,
        )?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        let mut engine = TemplateEngine::new(config, loader, Params::default());
        engine.import_jq_package(SEMCONV_JQ)?;
        engine
    };

    let mut registry_path = args.registry.registry.clone();
    // Support for --registry-git-sub-dir (should be removed in the future)
    if let registry::RegistryPath::GitRepo { sub_folder, .. } = &mut registry_path {
        if sub_folder.is_none() {
            sub_folder.clone_from(&args.registry.registry_git_sub_dir);
        }
    }
    let registry_repo = RegistryRepo::try_new("main", &registry_path)?;
    let generator = SnippetGenerator::try_from_registry_repo(
        &registry_repo,
        generator,
        &mut diag_msgs,
        args.common_registry_args.follow_symlinks,
    )?;

    if is_future_mode_enabled() && !diag_msgs.is_empty() {
        // If we are in future mode and there are diagnostics, return them
        // without generating any snippets.
        return Err(diag_msgs);
    }

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
            &generator,
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

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
    use crate::registry::{RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand};
    use crate::{run_command, CommonRegistryArgs};

    #[test]
    fn test_registry_update_markdown() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: "data/update_markdown/markdown".to_owned(),
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "data/update_markdown/registry".to_owned(),
                        },
                        registry_git_sub_dir: None,
                    },
                    dry_run: true,
                    attribute_registry_base_url: Some("/docs/attributes-registry".to_owned()),
                    templates: "data/update_markdown/templates".to_owned(),
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                    common_registry_args: CommonRegistryArgs {
                        follow_symlinks: false,
                    },
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
