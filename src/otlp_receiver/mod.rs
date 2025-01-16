// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.

use grpc_stubs::proto::collector::logs::v1::logs_service_server::{LogsService, LogsServiceServer};
use grpc_stubs::proto::collector::logs::v1::{ExportLogsServiceRequest, ExportLogsServiceResponse};
use grpc_stubs::proto::collector::metrics::v1::metrics_service_server::{
    MetricsService, MetricsServiceServer,
};
use grpc_stubs::proto::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use grpc_stubs::proto::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use grpc_stubs::proto::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use miette::Diagnostic;
use serde::Serialize;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;

/// Expose the OTLP gRPC services.
/// See the build.rs file for more information.
pub mod grpc_stubs {
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
pub enum Error {}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}


// Enum to represent received OTLP requests.
#[derive(Debug)]
pub enum OtlpRequest {
    Logs(ExportLogsServiceRequest),
    Metrics(ExportMetricsServiceRequest),
    Traces(ExportTraceServiceRequest),

    Error(String),
    Stop(StopSignal),
}

/// Enum to represent stop signals.
#[derive(Debug)]
pub enum StopSignal {
    /// CTRL+C
    Sigint,
    /// SIGHUP
    Sighup,
    /// HTTP POST to /stop
    AdminStop,
    /// Inactivity timeout
    Inactivity,
}

/// Start an OTLP receiver listening to a specific port on all IPv4 interfaces
/// and return an iterator of received OTLP requests.
pub fn listen_otlp_requests(
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
    logger: impl Logger + Sync + Clone,
) -> impl Iterator<Item = OtlpRequest> {
    let addr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("Failed to parse address");
    let (tx, rx) = mpsc::channel(100);
    let stop_tx = tx.clone();
    // Create a watch channel for the last activity timestamp
    let (activity_tx, activity_rx) = watch::channel(Instant::now());
    let logs_service = LogsServiceImpl {
        tx: tx.clone(),
        activity_tx: activity_tx.clone(),
    };
    let metrics_service = MetricsServiceImpl {
        tx: tx.clone(),
        activity_tx: activity_tx.clone(),
    };
    let trace_service = TraceServiceImpl {
        tx: tx.clone(),
        activity_tx: activity_tx.clone(),
    };

    logger.log("To stop the OTLP receiver:");
    logger.log("  - press CTRL+C,");
    logger.log(&format!(
        "  - send a SIGHUP signal to the weaver process or run this command kill -SIGHUP {}",
        std::process::id()
    ));
    logger.log(&format!("  - or send a POST request to the /stop endpoint via the following command curl -X POST http://localhost:{}/stop.", grpc_port + 1));
    logger.log(&format!(
        "The OTLP receiver will stop after {} seconds of inactivity.",
        inactivity_timeout.as_secs()
    ));

    // Start an OS thread and run a single threaded Tokio runtime inside.
    // The async OTLP receiver sends the received OTLP messages to the Tokio channel.
    let _ = std::thread::spawn(move || {
        // Start a current threaded Tokio runtime
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                // Spawn tasks to handle different stop signals
                spawn_stop_signal_handlers(stop_tx.clone());
                spawn_http_stop_handler(stop_tx.clone(), admin_port).await;
                spawn_inactivity_monitor(stop_tx.clone(), activity_rx, inactivity_timeout);

                // Serve the OTLP services
                let server = Server::builder()
                    .add_service(LogsServiceServer::new(logs_service))
                    .add_service(MetricsServiceServer::new(metrics_service))
                    .add_service(TraceServiceServer::new(trace_service))
                    .serve(addr)
                    .await;

                if let Err(e) = server {
                    let _ = tx
                        .send(OtlpRequest::Error(format!(
                            "OTLP server encountered an error: {e}"
                        )))
                        .await;
                }
            });
    });

    SyncReceiver { receiver: rx }
}

/// Spawn tasks to handle CTRL+C and SIGHUP signals
fn spawn_stop_signal_handlers(stop_tx: mpsc::Sender<OtlpRequest>) {
    // Handle CTRL+C
    let ctrl_c_tx = stop_tx.clone();
    let _ = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C");
        let _ = ctrl_c_tx
            .send(OtlpRequest::Stop(StopSignal::Sigint))
            .await
            .ok();
    });

    // Handle SIGHUP
    let sighup_tx = stop_tx;
    let _ = tokio::spawn(async move {
        let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            .expect("Failed to create SIGHUP signal handler");

        let _ = sighup.recv().await;
        let _ = sighup_tx
            .send(OtlpRequest::Stop(StopSignal::Sighup))
            .await
            .ok();
    });
}

/// Spawn a minimal HTTP server that handles the /stop endpoint
async fn spawn_http_stop_handler(stop_tx: mpsc::Sender<OtlpRequest>, port: u16) {
    let addr: std::net::SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Failed to parse HTTP stop port");

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let _ = tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut socket, _)) => {
                            let mut buffer = [0; 1024];
                            if let Ok(n) = socket.read(&mut buffer).await {
                                let request = String::from_utf8_lossy(&buffer[..n]);

                                // Parse the request - very basic HTTP parsing
                                let lines: Vec<&str> = request.lines().collect();
                                if let Some(first_line) = lines.first() {
                                    let parts: Vec<&str> = first_line.split_whitespace().collect();
                                    if parts.len() >= 2 && parts[0] == "POST" && parts[1] == "/stop"
                                    {
                                        // Send stop signal
                                        let _ = stop_tx
                                            .send(OtlpRequest::Stop(StopSignal::AdminStop))
                                            .await
                                            .ok();

                                        // Send HTTP 200 OK response
                                        let response =
                                            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                                        let _ = socket.write_all(response.as_bytes()).await.ok();
                                    } else {
                                        // Send HTTP 404 Not Found for any other request
                                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
                                        let _ = socket.write_all(response.as_bytes()).await.ok();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to accept HTTP connection: {}", e);
                        }
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("Failed to bind HTTP stop port {}: {}", port, e);
        }
    }
}

/// Spawn a task that monitors for inactivity and triggers shutdown if timeout is reached
fn spawn_inactivity_monitor(
    stop_tx: mpsc::Sender<OtlpRequest>,
    activity_rx: watch::Receiver<Instant>,
    timeout: Duration,
) {
    let _ = tokio::spawn(async move {
        loop {
            // Wait for the timeout duration
            sleep(timeout).await;

            // Check if we've exceeded the inactivity timeout
            let last_activity = *activity_rx.borrow();
            if last_activity.elapsed() >= timeout {
                eprintln!("Shutting down due to inactivity timeout");
                let _ = stop_tx
                    .send(OtlpRequest::Stop(StopSignal::Inactivity))
                    .await
                    .ok();
                break;
            }

            // Check if we should stop monitoring (channel closed)
            if activity_rx.has_changed().is_err() {
                break;
            }
        }
    });
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
    activity_tx: watch::Sender<Instant>,
}
pub struct MetricsServiceImpl {
    tx: mpsc::Sender<OtlpRequest>,
    activity_tx: watch::Sender<Instant>,
}
pub struct TraceServiceImpl {
    tx: mpsc::Sender<OtlpRequest>,
    activity_tx: watch::Sender<Instant>,
}

#[tonic::async_trait]
impl LogsService for LogsServiceImpl {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        // Update last activity time
        self.activity_tx
            .send(Instant::now())
            .map_err(|_| Status::internal("Failed to update activity timestamp"))?;

        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Logs).await?;
        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl MetricsService for MetricsServiceImpl {
    async fn export(
        &self,
        request: Request<ExportMetricsServiceRequest>,
    ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
        // Update last activity time
        self.activity_tx
            .send(Instant::now())
            .map_err(|_| Status::internal("Failed to update activity timestamp"))?;

        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Metrics).await?;
        Ok(Response::new(ExportMetricsServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl TraceService for TraceServiceImpl {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        // Update last activity time
        self.activity_tx
            .send(Instant::now())
            .map_err(|_| Status::internal("Failed to update activity timestamp"))?;

        forward_to_channel(&self.tx, request.into_inner(), OtlpRequest::Traces).await?;
        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otlp_receiver::grpc_stubs::proto::collector::logs::v1::logs_service_client::LogsServiceClient;
    use crate::otlp_receiver::grpc_stubs::proto::collector::metrics::v1::metrics_service_client::MetricsServiceClient;
    use std::thread;
    use weaver_common::TestLogger;
    use crate::otlp_receiver::grpc_stubs::proto::collector::trace::v1::trace_service_client::TraceServiceClient;

    #[test]
    fn test_inactivity_stop_after_1_second() {
        let grpc_port = portpicker::pick_unused_port().expect("No free ports");
        let admin_port = portpicker::pick_unused_port().expect("No free ports");
        let inactivity_timeout = Duration::from_millis(500);
        let logger = TestLogger::default();

        let mut receiver = listen_otlp_requests(grpc_port, admin_port, inactivity_timeout, logger);

        // Give the server a little time to start up
        thread::sleep(Duration::from_millis(100));

        let grpc_endpoint = format!("http://0.0.0.0:{grpc_port}");
        let expected_metrics_count = 3;
        let expected_logs_count = 4;
        let expected_traces_count = 5;

        let _ = thread::spawn(move || {
            let grpc_endpoint_clone = grpc_endpoint.clone();
            //let grpc_endpoint = grpc_endpoint_clone.as_str();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    // Send 3 metrics
                    let mut metrics_client =
                        MetricsServiceClient::connect(grpc_endpoint_clone.clone())
                            .await
                            .unwrap();
                    for _ in 0..expected_metrics_count {
                        let _ = metrics_client
                            .export(ExportMetricsServiceRequest::default())
                            .await;
                    }

                    // Send 4 logs
                    let mut logs_client = LogsServiceClient::connect(grpc_endpoint.clone())
                        .await
                        .unwrap();
                    for _ in 0..expected_logs_count {
                        let _ = logs_client
                            .export(ExportLogsServiceRequest::default())
                            .await;
                    }

                    // Send 5 traces
                    let mut traces_client =
                        TraceServiceClient::connect(grpc_endpoint.clone())
                            .await
                            .unwrap();
                    for _ in 0..expected_traces_count {
                        let _ = traces_client
                            .export(ExportTraceServiceRequest::default())
                            .await;
                    }
                });
        })
        .join();

        // We expect 3 metrics, 4 logs, and 5 traces to be received and then the server to stop
        // due to inactivity.
        let mut metrics_count = 0;
        let mut logs_count = 0;
        let mut traces_count = 0;
        
        loop {
            let request = receiver.next().unwrap();
            match request {
                OtlpRequest::Metrics(_) => metrics_count += 1,
                OtlpRequest::Logs(_) => logs_count += 1,
                OtlpRequest::Traces(_) => traces_count += 1,
                OtlpRequest::Stop(StopSignal::Inactivity) => {
                    break;
                },
                other => {
                    panic!("Unexpected request: {:?}", other);
                }
            }
        }

        assert_eq!(
            metrics_count, expected_metrics_count,
            "The number of metrics received is incorrect"
        );
        assert_eq!(
            logs_count, expected_logs_count,
            "The number of logs received is incorrect"
        );
        assert_eq!(
            traces_count, expected_traces_count,
            "The number of traces received is incorrect"
        );
    }

    #[test]
    fn test_http_stop_endpoint() {
        let grpc_port = portpicker::pick_unused_port().expect("No free ports");
        let admin_port = portpicker::pick_unused_port().expect("No free ports");
        let inactivity_timeout = Duration::from_secs(5);
        let logger = TestLogger::default();

        let mut receiver = listen_otlp_requests(grpc_port, admin_port, inactivity_timeout, logger);

        // Give the server a little time to finish binding the port.
        thread::sleep(Duration::from_millis(200));

        // Send a POST request to /stop on the admin port to stop the server.
        let url = format!("http://127.0.0.1:{admin_port}/stop");
        let response = ureq::post(&url).call().expect("HTTP POST to /stop failed");
        assert_eq!(
            response.status(),
            200,
            "Stop endpoint returned non-200 status"
        );

        match receiver.next() {
            Some(OtlpRequest::Stop(StopSignal::AdminStop)) => {
                eprintln!("Test: Received Stop as expected");
            }
            other => {
                panic!("Expected OtlpRequest::Stop, got {:?}", other);
            }
        }
    }
}
