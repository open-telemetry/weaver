// SPDX-License-Identifier: Apache-2.0

//! Detect the gap between a semantic convention registry and an OTLP traffic.

use crate::otlp_receiver::{listen_otlp_requests, OtlpRequest};
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;

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
    logger: impl Logger + Sync + Clone,
    args: &CheckRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let mut request_count = 0;
    let policy = PolicyArgs::skip();
    let (_resolved_registry, _) =
        prepare_main_registry(&args.registry, &policy, logger.clone(), &mut diag_msgs)?;
    let otlp_requests = listen_otlp_requests(args.port, logger.clone());
    
    logger.loading(&format!("Checking OTLP traffic on port {}.", args.port));

    for otlp_request in otlp_requests {
        match otlp_request {
            OtlpRequest::Logs(logs) => {
                request_count += 1;
                dbg!(logs);
            }
            OtlpRequest::Metrics(metrics) => {
                request_count += 1;
                dbg!(metrics);
            }
            OtlpRequest::Traces(traces) => {
                request_count += 1;
                dbg!(traces);
            }
            OtlpRequest::Stop => {
                break;
            }
            OtlpRequest::Error(error) => {
                logger.error(&error);
                break;
            }
        }
    }

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    logger.success(&format!(
        "{request_count} OTLP requests received and checked."
    ));

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
