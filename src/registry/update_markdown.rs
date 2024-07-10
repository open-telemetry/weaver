// SPDX-License-Identifier: Apache-2.0

//! Update markdown files that contain markers indicating the templates used to
//! update the specified sections.

use crate::registry::RegistryArgs;
use crate::util::semconv_registry_path_from;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::config::Params;
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::TemplateEngine;
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
    pub target: Option<String>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Update markdown files.
pub(crate) fn command(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryUpdateMarkdownArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    fn is_markdown(entry: &walkdir::DirEntry) -> bool {
        let path = entry.path();
        let extension = path.extension().unwrap_or_else(|| std::ffi::OsStr::new(""));
        path.is_file() && extension == "md"
    }
    // Construct a generator if we were given a `--target` argument.
    let generator = match args.target.as_ref() {
        None => None,
        Some(target) => {
            let loader = FileSystemFileLoader::try_new(
                format!("{}/registry/{}", args.templates, target).into(),
            )?;
            Some(TemplateEngine::try_new(loader, Params::default())?)
        }
    };

    let generator = SnippetGenerator::try_from_url(
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir),
        cache,
        generator,
    )?;
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
    use crate::run_command;

    #[test]
    fn test_registry_update_markdown() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: "data/update_markdown/markdown".to_owned(),
                    registry: RegistryArgs {
                        registry: RegistryPath::Local("data/update_markdown/registry".to_owned()),
                        registry_git_sub_dir: None,
                    },
                    dry_run: true,
                    attribute_registry_base_url: Some("/docs/attributes-registry".to_owned()),
                    templates: "data/update_markdown/templates".to_owned(),
                    diagnostic: Default::default(),
                    target: Some("markdown".to_owned()),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
