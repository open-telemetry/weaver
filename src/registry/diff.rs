// SPDX-License-Identifier: Apache-2.0

//! Generate a diff between two versions of a semantic convention registry.

use crate::registry::Error::DiffRender;
use crate::registry::RegistryArgs;
use crate::util::{load_semconv_specs, resolve_telemetry_schema};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use include_dir::{include_dir, Dir};
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_cache::registry_path::RegistryPath;
use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};

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
    baseline_registry: RegistryPath,

    /// Format used to render the schema changes. Predefined formats are: ansi, json,
    /// yaml, and markdown.
    #[arg(long, default_value = "ansi")]
    diff_format: String,

    /// Path to the directory where the schema changes templates are located.
    #[arg(long, default_value = "diff_templates")]
    diff_template: PathBuf,

    /// Path to the directory where the generated artifacts will be saved.
    /// Default is the `output` directory.
    #[arg(default_value = "output")]
    output: PathBuf,

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

/// Generate a diff between two versions of a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryDiffArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    logger.log("Weaver Registry Diff");
    logger.loading(&format!("Checking registry `{}`", args.registry.registry));

    let registry_path = args.registry.registry.clone();
    let main_registry_repo = RegistryRepo::try_new("main", &registry_path)?;
    let baseline_registry_repo = RegistryRepo::try_new("baseline", &args.baseline_registry)?;
    let main_semconv_specs = load_semconv_specs(
        &main_registry_repo,
        logger.clone(),
        args.registry.follow_symlinks,
    )
    .capture_non_fatal_errors(&mut diag_msgs)?;
    let baseline_semconv_specs = load_semconv_specs(
        &baseline_registry_repo,
        logger.clone(),
        args.registry.follow_symlinks,
    )
    .capture_non_fatal_errors(&mut diag_msgs)?;

    let main_resolved_schema =
        resolve_telemetry_schema(&main_registry_repo, main_semconv_specs, logger.clone())
            .capture_non_fatal_errors(&mut diag_msgs)?;
    let baseline_resolved_schema = resolve_telemetry_schema(
        &baseline_registry_repo,
        baseline_semconv_specs,
        logger.clone(),
    )
    .capture_non_fatal_errors(&mut diag_msgs)?;

    // Generate the diff between the two versions of the registries.
    let changes = main_resolved_schema.diff(&baseline_resolved_schema);

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
    let engine = TemplateEngine::new(config, loader, Params::default());
    let output_directive = if args.diff_format == "ansi" || args.diff_format == "ansi-stats" {
        OutputDirective::Stdout
    } else {
        OutputDirective::File
    };

    match engine.generate(
        logger.clone(),
        &changes,
        args.output.as_path(),
        &output_directive,
    ) {
        Ok(_) => {}
        Err(e) => {
            return Err(DiagnosticMessages::from(DiffRender {
                error: e.to_string(),
            }));
        }
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_registry_diff() {}
}
