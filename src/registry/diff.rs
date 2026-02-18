// SPDX-License-Identifier: Apache-2.0

//! Generate a diff between two versions of a semantic convention registry.

use crate::registry::Error::DiffRender;
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use include_dir::{include_dir, Dir};
use log::info;
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_semconv::registry_repo::RegistryRepo;

/// Embedded default schema changes templates
pub(crate) static DEFAULT_DIFF_TEMPLATES: Dir<'_> = include_dir!("defaults/diff_templates");

/// Parameters for the `registry diff` sub-command
#[derive(Debug, Args)]
pub struct RegistryDiffArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: VirtualDirectoryPath,

    /// Format used to render the schema changes. Predefined formats are: ansi, json,
    /// and markdown.
    #[arg(long, default_value = "ansi")]
    diff_format: String,

    /// Path to the directory where the schema changes templates are located.
    #[arg(long, default_value = "diff_templates")]
    diff_template: PathBuf,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the diff report is printed to stdout
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub(crate) diagnostic: DiagnosticArgs,
}

/// An error that can occur while generating the diff between two versions of the same
/// semantic convention registry.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Writing to the file failed.
    #[error("Writing to the file ‘{file}’ failed for the following reason: {error}")]
    WriteError {
        /// The path to the output file.
        file: PathBuf,
        /// The error that occurred.
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

enum VersionedDiff {
    V1(crate::weaver::Diff),
    V2(crate::weaver::DiffV2),
}

/// Generate a diff between two versions of a semantic convention registry.
pub(crate) fn command(args: &RegistryDiffArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut output = PathBuf::from("output");
    let output_directive = if let Some(path_buf) = &args.output {
        output = path_buf.clone();
        OutputDirective::File
    } else {
        OutputDirective::Stdout
    };

    let mut diag_msgs = DiagnosticMessages::empty();
    // TODO - make sure policy is disabled.
    let policy_config = PolicyArgs {
        policies: vec![],
        skip_policies: true,
        display_policy_coverage: false,
    };
    let weaver = WeaverEngine::new(&args.registry, &policy_config);

    info!("Weaver Registry Diff");
    info!("Checking registry `{}`", args.registry.registry);

    let registry_path = args.registry.registry.clone();
    let main_registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
    let baseline_registry_repo = RegistryRepo::try_new(None, &args.baseline_registry, &mut vec![])?;

    let main = weaver.load_definitions(main_registry_repo, &mut diag_msgs)?;
    let baseline = weaver.load_definitions(baseline_registry_repo, &mut diag_msgs)?;
    let main_resolved = weaver.resolve(main, &mut diag_msgs)?;
    let baseline_resolved = weaver.resolve(baseline, &mut diag_msgs)?;
    // Generate diff.
    let diff = if args.registry.v2 {
        VersionedDiff::V2(
            main_resolved
                .try_into_v2()?
                .diff(&baseline_resolved.try_into_v2()?),
        )
    } else {
        VersionedDiff::V1(main_resolved.diff(&baseline_resolved))
    };

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    let loader = EmbeddedFileLoader::try_new(
        &DEFAULT_DIFF_TEMPLATES,
        args.diff_template.clone(),
        &args.diff_format,
    )
    .expect("Failed to create the embedded file loader for the diff templates");
    let config = WeaverConfig::try_from_loader(&loader)
        .expect("Failed to load `defaults/diff_templates/weaver.yaml`");
    let engine = TemplateEngine::try_new(config, loader, Params::default())?;

    let result = match diff {
        VersionedDiff::V1(d) => {
            engine.generate(d.as_template_context(), output.as_path(), &output_directive)
        }
        VersionedDiff::V2(d) => {
            engine.generate(d.as_template_context(), output.as_path(), &output_directive)
        }
    };
    match result {
        Ok(_) => {}
        Err(e) => {
            return Err(DiagnosticMessages::from(DiffRender {
                error: e.to_string(),
            }));
        }
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::registry::diff::RegistryDiffArgs;
    use crate::registry::{
        semconv_registry, RegistryArgs, RegistryCommand, RegistrySubCommand, VirtualDirectoryPath,
    };
    use crate::run_command;
    use std::fs::OpenOptions;
    use weaver_version::schema_changes::SchemaChanges;

    #[test]
    fn test_registry_diff_exit_code() {
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Diff(RegistryDiffArgs {
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "tests/diff/registry_head/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    baseline_registry: VirtualDirectoryPath::LocalFolder {
                        path: "tests/diff/registry_baseline/".to_owned(),
                    },
                    diff_format: "json".to_owned(),
                    diff_template: Default::default(),
                    output: None,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }

    #[test]
    fn test_registry_diff_cmd() {
        let temp_dir = tempfile::Builder::new()
            .prefix("output")
            .tempdir()
            .expect("Failed to create temp file");

        let registry_cmd = RegistryCommand {
            command: RegistrySubCommand::Diff(RegistryDiffArgs {
                registry: RegistryArgs {
                    registry: VirtualDirectoryPath::LocalFolder {
                        path: "tests/diff/registry_head/".to_owned(),
                    },
                    follow_symlinks: false,
                    include_unreferenced: false,
                    v2: false,
                },
                baseline_registry: VirtualDirectoryPath::LocalFolder {
                    path: "tests/diff/registry_baseline/".to_owned(),
                },
                diff_format: "json".to_owned(),
                diff_template: Default::default(),
                output: Some(temp_dir.path().to_path_buf()),
                diagnostic: Default::default(),
            }),
        };

        let cmd_result = semconv_registry(&registry_cmd);
        assert_eq!(
            cmd_result
                .command_result
                .expect("Command should complete successfully")
                .exit_code,
            0
        );

        // Read the output file and check that it contains the expected JSON.
        let output_file = temp_dir.path().join("diff.json");
        let schema_changes: SchemaChanges = {
            let file = OpenOptions::new()
                .read(true)
                .open(&output_file)
                .expect("Failed to open file");
            serde_json::from_reader(file).expect("Failed to parse JSON")
        };
        // Note: span differences have disappeared.
        assert_eq!(
            schema_changes.count_changes(),
            25,
            "Expected 25 total changes in {:?}",
            &schema_changes
        );
        assert_eq!(schema_changes.count_registry_attribute_changes(), 5);
        assert_eq!(schema_changes.count_added_registry_attributes(), 1);
        assert_eq!(schema_changes.count_removed_registry_attributes(), 1);
        assert_eq!(schema_changes.count_obsoleted_registry_attributes(), 1);
        assert_eq!(schema_changes.count_uncategorized_registry_attributes(), 1);
        assert_eq!(schema_changes.count_renamed_registry_attributes(), 1);
        assert_eq!(schema_changes.count_metric_changes(), 5);
        assert_eq!(schema_changes.count_span_changes(), 5);
        assert_eq!(schema_changes.count_event_changes(), 5);
        assert_eq!(schema_changes.count_resource_changes(), 5);
    }
}
