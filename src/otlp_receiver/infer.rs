// SPDX-License-Identifier: Apache-2.0

//! Infer a semantic convention registry from an OTLP traffic.

use std::time::Duration;
use clap::Args;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use crate::{DiagnosticArgs, ExitDirectives};
use crate::otlp_receiver::{listen_otlp_requests, OtlpRequest};

/// Parameters for the `otlp-receiver infer-registry` sub-command
#[derive(Debug, Args)]
pub struct InferRegistryArgs {
    /// Port used by the gRPC OTLP receiver
    #[clap(long, default_value = "4317", short = 'p')]
    pub otlp_grpc_port: u16,

    /// Port used by the admin port (endpoints: /stop)
    #[clap(long, default_value = "4320", short = 'a')]
    pub admin_port: u16,
    
    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Infer a semantic convention registry from an OTLP traffic.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &InferRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let otlp_requests = listen_otlp_requests(
        args.otlp_grpc_port,
        args.admin_port,
        Duration::from_secs(5),
        logger.clone()
    );

    for otlp_request in otlp_requests {
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
            OtlpRequest::Stop(_) => {
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
