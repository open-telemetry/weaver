// SPDX-License-Identifier: Apache-2.0

//! Generate a diff between two versions of a semantic convention registry.

use crate::registry::RegistryArgs;
use crate::util::{load_semconv_specs, resolve_semconv_specs};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_cache::registry_path::RegistryPath;
use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages, ResultExt};
use weaver_common::Logger;
use weaver_resolved_schema::{ResolvedTelemetrySchema, SchemaChange};
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::semconv::SemConvSpec;

/// Parameters for the `registry diff` sub-command
#[derive(Debug, Args)]
pub struct RegistryDiffArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: RegistryPath,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
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
    let main_semconv_specs = load_semconv_specs(&main_registry_repo, logger.clone())
        .capture_non_fatal_errors(&mut diag_msgs)?;
    let baseline_semconv_specs = load_semconv_specs(&baseline_registry_repo, logger.clone())
        .capture_non_fatal_errors(&mut diag_msgs)?;
    let main_resolved_schema = resolve_telemetry_schema(
        main_registry_repo,
        main_semconv_specs,
        logger.clone(),
        &mut diag_msgs,
    )?;
    let baseline_resolved_schema = resolve_telemetry_schema(
        baseline_registry_repo,
        baseline_semconv_specs,
        logger.clone(),
        &mut diag_msgs,
    )?;

    let changes = main_resolved_schema.diff(&baseline_resolved_schema);
    //dbg!(&changes);
    let yaml_changes = serde_json::to_string_pretty(&changes).expect("Failed to serialize changes");
    println!("{}", yaml_changes);

    let mut added_attributes = 0;
    let mut renamed_to_new_attributes = 0;
    let mut renamed_to_existing_attributes = 0;
    let mut removed_attributes = 0;
    let mut deprecated_attributes = 0;
    let mut added_metrics = 0;
    let mut renamed_to_new_metrics = 0;
    let mut renamed_to_existing_metrics = 0;
    let mut removed_metrics = 0;
    let mut deprecated_metrics = 0;

    for change in changes {
        match change {
            SchemaChange::AddedAttribute { .. } => {
                added_attributes += 1;
            }
            SchemaChange::RenamedToNewAttribute { .. } => {
                renamed_to_new_attributes += 1;
            }
            SchemaChange::RemovedAttribute { .. } => {
                removed_attributes += 1;
            }
            SchemaChange::RenamedToExistingAttribute { .. } => {
                renamed_to_existing_attributes += 1;
            }
            SchemaChange::DeprecatedAttribute { .. } => {
                deprecated_attributes += 1;
            }
            SchemaChange::AddedMetric { .. } => {
                added_metrics += 1;
            }
            SchemaChange::RenamedToNewMetric { .. } => {
                renamed_to_new_metrics += 1;
            }
            SchemaChange::RenamedToExistingMetric { .. } => {
                renamed_to_existing_metrics += 1;
            }
            SchemaChange::DeprecatedMetric { .. } => {
                deprecated_metrics += 1;
            }
            SchemaChange::RemovedMetric { .. } => {
                removed_metrics += 1;
            }
        }
    }
    dbg!(
        added_attributes,
        renamed_to_new_attributes,
        renamed_to_existing_attributes,
        removed_attributes,
        deprecated_attributes
    );
    dbg!(
        added_metrics,
        renamed_to_new_metrics,
        renamed_to_existing_metrics,
        removed_metrics,
        deprecated_metrics
    );

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

fn resolve_telemetry_schema(
    registry_repo: RegistryRepo,
    semconv_specs: Vec<(String, SemConvSpec)>,
    logger: impl Logger + Sync + Clone,
    diag_msgs: &mut DiagnosticMessages,
) -> Result<ResolvedTelemetrySchema, DiagnosticMessages> {
    let mut registry = SemConvRegistry::from_semconv_specs(registry_repo.id(), semconv_specs);
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    let resolved_schema =
        resolve_semconv_specs(&mut registry, logger.clone()).combine_diag_msgs_with(&diag_msgs)?;

    Ok(resolved_schema)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_registry_diff() {}
}
