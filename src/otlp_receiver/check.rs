// SPDX-License-Identifier: Apache-2.0

//! Detect the gap between a semantic convention registry and an OTLP traffic.

use clap::Args;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use crate::{DiagnosticArgs, ExitDirectives};
use crate::otlp_receiver::listen_otlp_requests;
use crate::registry::RegistryArgs;

/// Parameters for the `otlp-receiver check-registry` sub-command
#[derive(Debug, Args)]
pub struct CheckRegistryArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Port used by the OTLP receiver
    #[clap(long, default_value = "4317")]
    pub port: u16,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Detect the gap between a semantic convention registry and an OTLP traffic.
pub(crate) fn command(
    _logger: impl Logger + Sync + Clone,
    args: &CheckRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    listen_otlp_requests(args.port);
    
    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
