// SPDX-License-Identifier: Apache-2.0

//! Check the gap between a semantic convention registry and an OTLP traffic.

use crate::registry::otlp::{listen_otlp_requests, OtlpRequest};
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use std::time::Duration;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;

/// Parameters for the `registry live-check` sub-command
#[derive(Debug, Args)]
pub struct CheckRegistryArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    pub otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317", short = 'p')]
    pub otlp_grpc_port: u16,

    /// Port used by the HTTP admin port (endpoints: /stop).
    #[clap(long, default_value = "4320", short = 'a')]
    pub admin_port: u16,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long, default_value = "10", short = 't')]
    pub inactivity_timeout: u64,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Check the conformance level of an OTLP stream against a semantic convention registry.
///
/// This command starts an OTLP listener and compares each received OTLP message with the
/// registry provided as a parameter. When the command is stopped (see stop conditions),
/// a conformance/coverage report is generated. The purpose of this command is to be used
/// in a CI/CD pipeline to validate the telemetry stream from an application or service
/// against a registry.
///
/// The currently supported stop conditions are: CTRL+C (SIGINT), SIGHUP, the HTTP /stop
/// endpoint, and a maximum duration of no OTLP message reception.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &CheckRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let policy = PolicyArgs::skip();
    let otlp_requests = listen_otlp_requests(
        args.otlp_grpc_address.as_str(),
        args.otlp_grpc_port,
        args.admin_port,
        Duration::from_secs(args.inactivity_timeout),
        logger.clone(),
    )?;

    // @ToDo Use the following resolved registry to check the level of compliance of the incoming OTLP messages
    let (_resolved_registry, _) =
        prepare_main_registry(&args.registry, &policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!(
        "Checking OTLP traffic on port {}.",
        args.otlp_grpc_port
    ));

    // @ToDo Implement the checking logic
    for otlp_request in otlp_requests {
        match otlp_request {
            OtlpRequest::Logs(_logs) => {
                // ToDo Implement the checking logic for logs
                println!("Logs Request received");
            }
            OtlpRequest::Metrics(_metrics) => {
                // ToDo Implement the checking logic for metrics
                println!("Metrics Request received");
            }
            OtlpRequest::Traces(_traces) => {
                // ToDo Implement the checking logic for traces
                println!("Trace Request received");
            }
            OtlpRequest::Stop(reason) => {
                logger.warn(&format!("Stopping the listener, reason: {}", reason));
                // ToDo Generate the report here
                break;
            }
            OtlpRequest::Error(error) => {
                diag_msgs.extend(DiagnosticMessages::from_error(error));
                break;
            }
        }
    }

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    logger.success("OTLP requests received and checked.");

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
