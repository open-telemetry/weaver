// SPDX-License-Identifier: Apache-2.0

//! Infer a semantic convention registry from an OTLP traffic.

use clap::Args;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use crate::{DiagnosticArgs, ExitDirectives};
use crate::otlp_receiver::{listen_otlp_requests, OtlpRequest};

/// Parameters for the `otlp-receiver infer-registry` sub-command
#[derive(Debug, Args)]
pub struct InferRegistryArgs {
    /// Port used by the OTLP receiver
    #[clap(long, default_value = "4317")]
    pub port: u16,
    
    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Infer a semantic convention registry from an OTLP traffic.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &InferRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    for otlp_request in listen_otlp_requests(args.port) {
        match otlp_request {
            OtlpRequest::Logs(logs) => {
                dbg!(logs);
            }
            OtlpRequest::Metrics(metrics) => {
                dbg!(metrics);
            }
            OtlpRequest::Traces(traces) => {
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
    println!("Do something with the received OTLP request");

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
