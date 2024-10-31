// SPDX-License-Identifier: Apache-2.0

//! Update an OTEL Schema file with the latest changes observed between two
//! versions of a semantic convention registry.

use crate::registry::RegistryArgs;
use crate::util::{load_semconv_specs, resolve_telemetry_schema};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_cache::registry_path::RegistryPath;
use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;
use weaver_otel_schema::TelemetrySchema;

/// Parameters for the `registry update-schema` sub-command
#[derive(Debug, Args)]
pub struct RegistryUpdateSchemaArgs {
    /// Path to the OpenTelemetry schema to update.
    schema: String,
    
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the baseline semantic convention registry
    #[arg(long)]
    baseline_registry: RegistryPath,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub(crate) diagnostic: DiagnosticArgs,
}

/// Update an OTEL Schema file with the latest changes observed between two
/// versions of a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryUpdateSchemaArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    logger.log("Weaver Registry Schema Update");
    logger.loading(&format!("Checking registry `{}`", args.registry.registry));

    let registry_path = args.registry.registry.clone();
    let main_registry_repo = RegistryRepo::try_new("main", &registry_path)?;
    let baseline_registry_repo = RegistryRepo::try_new("baseline", &args.baseline_registry)?;
    let main_semconv_specs = load_semconv_specs(&main_registry_repo, logger.clone())
        .capture_non_fatal_errors(&mut diag_msgs)?;
    let baseline_semconv_specs = load_semconv_specs(&baseline_registry_repo, logger.clone())
        .capture_non_fatal_errors(&mut diag_msgs)?;
    let main_resolved_schema = resolve_telemetry_schema(
        &main_registry_repo,
        main_semconv_specs,
        logger.clone(),
        &mut diag_msgs,
    )?;
    let baseline_resolved_schema = resolve_telemetry_schema(
        &baseline_registry_repo,
        baseline_semconv_specs,
        logger.clone(),
        &mut diag_msgs,
    )?;

    // Generate the diff between the two versions of the registries.
    let _changes = main_resolved_schema.diff(&baseline_resolved_schema);
    let schema = TelemetrySchema::try_from_file(args.schema.clone())?;
    dbg!(schema);
    
    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_registry_update_schema() {}
}
