// SPDX-License-Identifier: Apache-2.0

//! Initializes a `diagnostic_templates` directory to define or override diagnostic output formats.

use crate::diagnostic::{Error, DEFAULT_DIAGNOSTIC_TEMPLATES};
use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;

/// Parameters for the `diagnostic init` sub-command
#[derive(Debug, Args)]
pub struct DiagnosticInitArgs {
    /// Optional path where the diagnostic templates directory should be created.
    #[arg(short = 't', long, default_value = "diagnostic_templates")]
    pub diagnostic_templates_dir: PathBuf,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Initializes a `diagnostic_templates` directory to define or override diagnostic output formats.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &DiagnosticInitArgs,
) -> Result<(), DiagnosticMessages> {
    DEFAULT_DIAGNOSTIC_TEMPLATES
        .extract(args.diagnostic_templates_dir.clone())
        .map_err(|e| Error::InitDiagnosticError {
            path: args.diagnostic_templates_dir.clone(),
            error: e.to_string(),
        })?;
    logger.success(&format!(
        "Diagnostic templates initialized at {:?}",
        args.diagnostic_templates_dir
    ));
    Ok(())
}
