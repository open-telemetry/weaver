// SPDX-License-Identifier: Apache-2.0

//! Update markdown files that contain markers indicating the templates used to
//! update the specified sections.

use crate::registry::RegistryArgs;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use weaver_common::diagnostic::{is_future_mode_enabled, DiagnosticMessages};
use weaver_common::vdir::VirtualDirectory;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_common::{log_error, log_info, log_success, Error};
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::TemplateEngine;
use weaver_semconv::registry_repo::RegistryRepo;
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
    pub templates: VirtualDirectoryPath,

    /// If provided, the target to generate snippets with.
    /// Note: `registry update-markdown` will look for a specific jinja template:
    ///   {templates}/{target}/snippet.md.j2.
    #[arg(long)]
    pub target: String,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Update markdown files.
pub(crate) fn command(
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
        let templates_dir = VirtualDirectory::try_new(&args.templates).map_err(|e| {
            Error::InvalidVirtualDirectory {
                path: args.templates.to_string(),
                error: e.to_string(),
            }
        })?;
        let loader =
            FileSystemFileLoader::try_new(templates_dir.path().join("registry"), &args.target)?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        TemplateEngine::new(config, loader, Params::default())
    };

    let registry_path = &args.registry.registry;

    let registry_repo = RegistryRepo::try_new("main", registry_path)?;
    let generator = SnippetGenerator::try_from_registry_repo(
        &registry_repo,
        generator,
        &mut diag_msgs,
        args.registry.follow_symlinks,
        args.registry.include_unreferenced,
    )?;

    if is_future_mode_enabled() && !diag_msgs.is_empty() {
        // If we are in future mode and there are diagnostics, return them
        // without generating any snippets.
        return Err(diag_msgs);
    }

    log_success("Registry resolved successfully");
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
        log_info(format!("{}: ${}", operation, entry.path().display()));
        if let Err(error) = update_markdown(
            &entry.path().display().to_string(),
            &generator,
            args.dry_run,
            args.attribute_registry_base_url.as_deref(),
        ) {
            has_error = true;
            log_error(error);
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
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
    use crate::registry::{RegistryArgs, RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_registry_update_markdown() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: "data/update_markdown/markdown".to_owned(),
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "data/update_markdown/registry".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                    },
                    dry_run: true,
                    attribute_registry_base_url: Some("/docs/attributes-registry".to_owned()),
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "data/update_markdown/templates".to_owned(),
                    },
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
