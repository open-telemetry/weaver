// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.

mod infer;
mod check;

use clap::{Args, Subcommand};
use miette::Diagnostic;
use serde::Serialize;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;
use crate::CmdResult;
use check::CheckRegistryArgs;
use infer::InferRegistryArgs;
use tonic::transport::Server;
use grpc_server::proto::collector::logs::v1::{ExportLogsServiceRequest, ExportLogsServiceResponse};
use grpc_server::proto::collector::logs::v1::logs_service_server::{LogsService, LogsServiceServer};
use grpc_server::proto::collector::metrics::v1::{ExportMetricsServiceRequest, ExportMetricsServiceResponse};
use grpc_server::proto::collector::metrics::v1::metrics_service_server::{MetricsService, MetricsServiceServer};
use grpc_server::proto::collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse};
use grpc_server::proto::collector::trace::v1::trace_service_server::{TraceService, TraceServiceServer};

/// Expose the OTLP gRPC services.
/// See the build.rs file for more information.
pub mod grpc_server {
    #[path = ""]
    pub mod proto {
        #[path = ""]
        pub mod collector {
            #[path = ""]
            pub mod logs {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[path = "opentelemetry.proto.collector.logs.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod metrics {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[path = "opentelemetry.proto.collector.metrics.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod trace {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[path = "opentelemetry.proto.collector.trace.v1.rs"]
                pub mod v1;
            }
        }

        #[path = ""]
        pub mod logs {
            #[path = "opentelemetry.proto.logs.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod metrics {
            #[path = "opentelemetry.proto.metrics.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod trace {
            #[path = "opentelemetry.proto.trace.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod common {
            #[path = "opentelemetry.proto.common.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod resource {
            #[path = "opentelemetry.proto.resource.v1.rs"]
            pub mod v1;
        }
    }
}

/// Errors emitted by the `otlp-receiver` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Parameters for the `otlp-receiver` command
#[derive(Debug, Args)]
pub struct OtlpReceiverCommand {
    /// Define the sub-commands for the `otlp-receiver` command
    #[clap(subcommand)]
    pub command: OtlpReceiverSubCommand,
}

/// Sub-commands to manage a `otlp-receiver`.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum OtlpReceiverSubCommand {
    /// Infer a semantic convention registry from an OTLP traffic.
    #[clap(verbatim_doc_comment)]
    InferRegistry(InferRegistryArgs),
    /// Detect the gap between a semantic convention registry and an OTLP traffic.
    #[clap(verbatim_doc_comment)]
    CheckRegistry(CheckRegistryArgs),
}

/// Start the OTLP receiver and process the sub-command.
pub fn otlp_receiver(log: impl Logger + Sync + Clone, command: &OtlpReceiverCommand) -> CmdResult {
    match &command.command {
        OtlpReceiverSubCommand::InferRegistry(args) => CmdResult::new(
            infer::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
        OtlpReceiverSubCommand::CheckRegistry(args) => CmdResult::new(
            check::command(log.clone(), args),
            Some(args.diagnostic.clone()),
        ),
    }
}

// Enum to represent received OTLP requests.
pub enum OtlpRequest {
    Logs(ExportLogsServiceRequest),
    Metrics(ExportMetricsServiceRequest),
    Traces(ExportTraceServiceRequest),
    
    Error(String),
    Stop,
}

/// Start an OTLP receiver listening to a specific port on all IPv4 interfaces
/// and return an iterator of received OTLP requests.
pub fn listen_otlp_requests(port: u16, logger: impl Logger + Sync + Clone) -> impl Iterator<Item = OtlpRequest> {
    let addr = format!("0.0.0.0:{port}").parse().expect("Failed to parse address");
    let (tx, rx) = mpsc::channel(100);
    let stop_tx = tx.clone();
    let logs_service = LogsServiceImpl { tx: tx.clone() };
    let metrics_service = MetricsServiceImpl {  tx: tx.clone() };
    let trace_service = TraceServiceImpl {  tx: tx.clone() };
    
    logger.info("To stop the OTLP receiver:");
    logger.info("- press CTRL+C,");
    logger.info("- send a SIGHUP signal to the process,");
    logger.info("- or send a POST request to the /stop endpoint.");
    
    // Start an OS thread and run a single threaded Tokio runtime inside.
    // The async OTLP receiver sends the received OTLP messages to the Tokio channel.
    let _ = std::thread::spawn(move || {
        // Start a current threaded Tokio runtime
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                // Spawn a task to handle CTRL+C and send a stop signal.
                let _ = tokio::spawn(async move {
                    tokio::signal::ctrl_c().await.expect("Failed to listen for CTRL+C");
                    let _ = stop_tx.send(OtlpRequest::Stop).await;
                });
                
                // Serve the OTLP services
                let server = Server::builder()
                    .add_service(LogsServiceServer::new(logs_service))
                    .add_service(MetricsServiceServer::new(metrics_service))
                    .add_service(TraceServiceServer::new(trace_service))
                    .serve(addr)
                    .await;

                if let Err(e) = server {
                    tx.send(OtlpRequest::Error(format!("OTLP server encountered an error: {e}"))).await;
                }
            });
    });

    SyncReceiver { receiver: rx }
}

// Synchronous iterator wrapping a Tokio mpsc::Receiver.
pub struct SyncReceiver<T> {
    receiver: mpsc::Receiver<T>,
}

impl<T> Iterator for SyncReceiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.blocking_recv()
    }
}

async fn forward_to_channel<T>(
    sender: &mpsc::Sender<OtlpRequest>,
    otlp_request: T,
    wrapper: fn(T) -> OtlpRequest,
) -> Result<(), Status> {
    sender
        .send(wrapper(otlp_request))
        .await
        .map_err(|e| Status::resource_exhausted(format!("Channel full: {}", e)))
}

pub struct LogsServiceImpl {
    tx: mpsc::Sender<OtlpRequest>,
}
pub struct MetricsServiceImpl {
    tx: mpsc::Sender<OtlpRequest>,
}
pub struct TraceServiceImpl {
    tx: mpsc::Sender<OtlpRequest>,
}

#[tonic::async_trait]
impl LogsService for LogsServiceImpl {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Logs).await?;
        Ok(Response::new(ExportLogsServiceResponse { partial_success: None }))
    }
}

#[tonic::async_trait]
impl MetricsService for MetricsServiceImpl {
    async fn export(
        &self,
        request: Request<ExportMetricsServiceRequest>,
    ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Metrics).await?;
        Ok(Response::new(ExportMetricsServiceResponse { partial_success: None }))
    }
}

#[tonic::async_trait]
impl TraceService for TraceServiceImpl {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Traces).await?;
        Ok(Response::new(ExportTraceServiceResponse { partial_success: None }))
    }
}