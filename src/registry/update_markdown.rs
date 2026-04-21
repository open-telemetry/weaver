// SPDX-License-Identifier: Apache-2.0

//! Update markdown files that contain markers indicating the templates used to
//! update the specified sections.

use crate::registry::generate::generate_params_shared;
use crate::registry::{discover_auth_resolver, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use miette::Diagnostic;
use serde_yaml::Value;
use std::path::PathBuf;
use weaver_common::diagnostic::{is_future_mode_enabled, DiagnosticMessage, DiagnosticMessages};
use weaver_common::vdir::VirtualDirectory;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_common::{log_error, log_info, log_success, Error};
use weaver_forge::config::WeaverConfig;
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::{OutputProcessor, OutputTarget};
use weaver_semconv_gen::{MarkdownSnippetGenerator, SnipperGeneratorV2, SnippetGenerator};

#[derive(thiserror::Error, Debug, serde::Serialize, Diagnostic)]
enum UpdateMarkdownError {
    /// The update-markdown command found differences in dry-run.
    #[error("The update-markdown command found differences in dry-run.")]
    MarkdownNotUpToDate,

    /// The update-markdown command ran into a fatal error.
    #[error("weaver registry update-markdown failed.")]
    MarkdownUpdateFailed,
}

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

    /// Parameters key=value, defined in the command line, to pass to the templates.
    /// The value must be a valid YAML value.
    #[arg(short = 'D', long, value_parser = crate::registry::generate::parse_key_val)]
    pub param: Option<Vec<(String, Value)>>,

    /// Parameters, defined in a YAML file, to pass to the templates.
    #[arg(long)]
    pub params: Option<PathBuf>,

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
    let params = generate_params_shared(&args.param, &args.params)?;
    let auth = discover_auth_resolver();

    // Construct a generator if we were given a `--target` argument.
    let templates_dir = VirtualDirectory::try_new_with_auth(&args.templates, &auth).map_err(
        |e| Error::InvalidVirtualDirectory {
            path: args.templates.to_string(),
            error: e.to_string(),
        },
    )?;
    let output = {
        let loader =
            FileSystemFileLoader::try_new(templates_dir.path().join("registry"), &args.target)?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        OutputProcessor::from_template_config(config, loader, params, OutputTarget::Stdout)?
    };
    let policy_config = PolicyArgs {
        policies: vec![],
        skip_policies: true,
        display_policy_coverage: false,
    };
    let weaver = WeaverEngine::new_with_auth(&args.registry, &policy_config, auth);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;
    let generator: Box<dyn MarkdownSnippetGenerator> = match resolved {
        crate::weaver::Resolved::V2(resolved_v2) => {
            // TODO - extract both resolved and template in one go.
            let template_schema = resolved_v2.template_schema().clone();
            Box::new(SnipperGeneratorV2::new(
                resolved_v2.into_resolved_schema(),
                template_schema,
                output,
            ))
        }
        crate::weaver::Resolved::V1(resolved_v1) => Box::new(SnippetGenerator::new(
            resolved_v1.into_resolved_schema(),
            output,
        )),
    };

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
        if let Err(error) = generator.update_markdown(
            &entry.path().display().to_string(),
            args.dry_run,
            args.attribute_registry_base_url.as_deref(),
        ) {
            has_error = true;
            log_error(error);
        }
    }
    if has_error {
        let error = if args.dry_run {
            UpdateMarkdownError::MarkdownNotUpToDate
        } else {
            UpdateMarkdownError::MarkdownUpdateFailed
        };
        return Err(error.into());
    }

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

/// Converts from our local error to a diagnostic message response.
impl From<UpdateMarkdownError> for DiagnosticMessages {
    fn from(error: UpdateMarkdownError) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
    use crate::registry::{RegistryArgs, RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use std::io::Write;
    use weaver_common::vdir::VirtualDirectoryPath;
    use zip::write::FileOptions;

    #[test]
    fn test_registry_update_markdown() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: "data/update_markdown/markdown".to_owned(),
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "data/update_markdown/registry".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    dry_run: true,
                    attribute_registry_base_url: Some("/docs/attributes-registry".to_owned()),
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "data/update_markdown/templates".to_owned(),
                    },
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                    param: None,
                    params: None,
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }

    #[test]
    fn test_registry_update_markdown_dryrun() {
        let markdown_dir = "tests/markdown_update_dryrun/current_output";
        let template_dir = "tests/markdown_update_dryrun/templates";
        let schema_dir = "tests/markdown_update_dryrun/model";

        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: markdown_dir.to_owned(),
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: schema_dir.to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    dry_run: true,
                    attribute_registry_base_url: None,
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: template_dir.to_owned(),
                    },
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                    param: None,
                    params: None,
                }),
            })),
        };
        let exit_directive = run_command(&cli);
        // The command should not fail
        assert_ne!(exit_directive.exit_code, 0);
    }

    // Helper that will construct a zip file from a directory.
    // This means we can test zip-file extraction and lifetimes but don't need to check in
    // a binary zip file into git, which is hard to inspect the contents of.
    fn create_zip_from_dir(src_dir: &str, zip_path: &std::path::Path) {
        let file = std::fs::File::create(zip_path).expect("failed to create zip file");
        let mut zip = zip::ZipWriter::new(file);

        for entry in walkdir::WalkDir::new(src_dir) {
            let entry = entry.expect("failed to read directory entry");
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(src_dir).expect("failed to strip prefix");
                let zip_path_in_archive = format!("templates/{}", relative_path.to_str().unwrap());
                zip.start_file(zip_path_in_archive, FileOptions::<()>::default())
                    .expect("failed to add file to zip");
                let content = std::fs::read(path).expect("failed to read file");
                zip.write_all(&content)
                    .expect("failed to write to zip file");
            }
        }
        let _ = zip.finish().expect("failed to finish zip file");
    }

    #[test]
    fn test_registry_update_markdown_zip_templates() {
        let schema_dir = "tests/markdown_update_dryrun/model";

        let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let markdown_dir = temp_dir.path().to_path_buf();
        let zip_path = temp_dir.path().join("templates.zip");

        // Create a zip file for templates from test_data/test_template_package
        create_zip_from_dir("test_data/test_template_package", &zip_path);

        // Copy test.md to temp_dir so WalkDir finds it.
        let _ = std::fs::copy(
            "tests/markdown_update_dryrun/current_output/test.md",
            markdown_dir.join("test.md"),
        )
        .expect("failed to copy test file");

        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: markdown_dir.to_str().unwrap().to_owned(),
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: schema_dir.to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    dry_run: false, // Generate files first
                    attribute_registry_base_url: None,
                    templates: VirtualDirectoryPath::LocalArchive {
                        path: zip_path.to_str().unwrap().to_owned(),
                        sub_folder: None,
                    },
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                    param: None,
                    params: None,
                }),
            })),
        };
        let exit_directive = run_command(&cli);
        assert_eq!(exit_directive.exit_code, 0, "First run (generation) failed");

        // Second run: verify no differences
        let cli_verify = Cli {
            debug: 0,
            quiet: false,
            future: false,
            allow_git_credentials: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::UpdateMarkdown(RegistryUpdateMarkdownArgs {
                    markdown_dir: markdown_dir.to_str().unwrap().to_owned(),
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: schema_dir.to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    dry_run: true, // Verify
                    attribute_registry_base_url: None,
                    templates: VirtualDirectoryPath::LocalArchive {
                        path: zip_path.to_str().unwrap().to_owned(),
                        sub_folder: None,
                    },
                    diagnostic: Default::default(),
                    target: "markdown".to_owned(),
                    param: None,
                    params: None,
                }),
            })),
        };
        let exit_directive_verify = run_command(&cli_verify);
        assert_eq!(
            exit_directive_verify.exit_code, 0,
            "Second run (verification) failed"
        );
    }
}
