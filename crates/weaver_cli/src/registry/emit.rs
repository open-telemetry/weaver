// SPDX-License-Identifier: Apache-2.0

//! Weaver registry emit sub-command.

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::DiagnosticArgs;
use clap::Args;

/// Parameters for the `registry emit` sub-command
#[derive(Debug, Args)]
pub struct RegistryEmitArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Write the telemetry to standard output
    #[arg(long)]
    pub stdout: bool,

    /// Endpoint for the OTLP receiver. OTEL_EXPORTER_OTLP_ENDPOINT env var will override this.
    #[arg(long, default_value = weaver_emit::DEFAULT_OTLP_ENDPOINT)]
    pub endpoint: String,
}
