// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.

pub mod conversion;
pub mod otlp_ingester;

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
use std::fmt::{Display, Formatter};
use std::net::{AddrParseError, SocketAddr};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::sleep;
use tonic::codegen::tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};

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
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[path = "opentelemetry.proto.collector.logs.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod metrics {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[path = "opentelemetry.proto.collector.metrics.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod trace {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[path = "opentelemetry.proto.collector.trace.v1.rs"]
                pub mod v1;
            }
        }

        #[path = ""]
        pub mod logs {
            #[allow(rustdoc::invalid_html_tags)]
            #[path = "opentelemetry.proto.logs.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod metrics {
            #[allow(rustdoc::invalid_html_tags)]
            #[path = "opentelemetry.proto.metrics.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod trace {
            #[allow(rustdoc::invalid_html_tags)]
            #[path = "opentelemetry.proto.trace.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod common {
            #[allow(clippy::enum_variant_names)]
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
    /// An OTLP error occurred.
    #[error("The following OTLP error occurred: {error}")]
    OtlpError { error: String },

    /// An HTTP error occurred on the admin port.
    #[error("The following HTTP error occurred: {error}")]
    HttpAdminError { error: String },
}

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

    Error(Error),
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

impl Display for StopSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StopSignal::Sigint => f.write_str("SIGINT"),
            StopSignal::Sighup => f.write_str("SIGHUP"),
            StopSignal::AdminStop => f.write_str("ADMIN_STOP"),
            StopSignal::Inactivity => f.write_str("INACTIVITY"),
        }
    }
}

/// Start an OTLP receiver listening to a specific port on all IPv4 interfaces
/// and return an iterator of received OTLP requests.
///
/// This function guarantees that the OTLP server is started and ready when the
/// result is Ok(iterator).
pub fn listen_otlp_requests(
    grpc_addr: &str,
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
) -> Result<impl Iterator<Item = OtlpRequest>, Error> {
    let addr: SocketAddr =
        format!("{grpc_addr}:{grpc_port}")
            .parse()
            .map_err(|e: AddrParseError| Error::OtlpError {
                error: e.to_string(),
            })?;

    let listener = std::net::TcpListener::bind(addr).map_err(|e| Error::OtlpError {
        error: e.to_string(),
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::OtlpError {
            error: e.to_string(),
        })?;

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

    let (ready_tx, ready_rx) = oneshot::channel();

    // Start an OS thread and run a single threaded Tokio runtime inside.
    // The async OTLP receiver sends the received OTLP messages to the Tokio channel.
    let _ = std::thread::spawn(move || {
        // Start a current threaded Tokio runtime
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build Tokio Runtime")
            .block_on(async {
                let mut tasks = JoinSet::new();

                // Spawn tasks to handle different stop signals
                spawn_stop_signal_handlers(stop_tx.clone(), &mut tasks);
                spawn_http_stop_handler(stop_tx.clone(), admin_port, &mut tasks).await;
                spawn_inactivity_monitor(
                    stop_tx.clone(),
                    activity_rx,
                    inactivity_timeout,
                    &mut tasks,
                );

                let tokio_listener = TcpListener::from_std(listener)
                    .expect("Failed to convert std listener to tokio listener");
                let inbound = TcpListenerStream::new(tokio_listener);

                // Serve the OTLP services
                let server_future = Server::builder()
                    .add_service(LogsServiceServer::new(logs_service))
                    .add_service(MetricsServiceServer::new(metrics_service))
                    .add_service(TraceServiceServer::new(trace_service))
                    .serve_with_incoming(inbound);

                ready_tx
                    .send(())
                    .expect("Failed to signal that the server is ready");

                let result = server_future.await;
                if let Err(e) = result {
                    let _ = tx
                        .send(OtlpRequest::Error(Error::OtlpError {
                            error: format!("The OTLP listener encountered an error: {e}"),
                        }))
                        .await;
                }

                let _ = tasks.join_all().await;
            });
    });

    // Wait until the server is ready
    ready_rx.blocking_recv().map_err(|e| Error::OtlpError {
        error: format!(
            "OTLP server dropped before signaling readiness (error: {e})"
        ),
    })?;

    Ok(SyncReceiver { receiver: rx })
}

/// Spawn tasks to handle CTRL+C and SIGHUP signals.
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
fn spawn_stop_signal_handlers(stop_tx: mpsc::Sender<OtlpRequest>, tasks: &mut JoinSet<()>) {
    // Handle CTRL+C
    let ctrl_c_tx = stop_tx.clone();
    let _ = tasks.spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C");
        let _ = ctrl_c_tx
            .send(OtlpRequest::Stop(StopSignal::Sigint))
            .await
            .ok();
    });

    // Handle SIGHUP
    #[cfg(unix)]
    {
        let sighup_tx = stop_tx;
        let _ = tasks.spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to create SIGHUP signal handler");

            let _ = sighup.recv().await;
            let _ = sighup_tx
                .send(OtlpRequest::Stop(StopSignal::Sighup))
                .await
                .ok();
        });
    }
}

/// Spawn a minimal HTTP server that handles the /stop endpoint
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
async fn spawn_http_stop_handler(
    stop_tx: mpsc::Sender<OtlpRequest>,
    port: u16,
    tasks: &mut JoinSet<()>,
) {
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Failed to parse HTTP stop port");

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let _ = tasks.spawn(async move {
                let mut stop_signal_received = false;
                while !stop_signal_received {
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
                                        stop_signal_received = true;
                                    } else {
                                        // Send HTTP 404 Not Found for any other request
                                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
                                        let _ = socket.write_all(response.as_bytes()).await.ok();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            stop_tx
                                .send(OtlpRequest::Error(Error::HttpAdminError {error: format!("Failed to accept HTTP connection: {e}")}))
                                .await
                                .expect("Failed to send an OtlpRequest::Error");
                        }
                    }
                }
            });
        }
        Err(e) => {
            stop_tx
                .send(OtlpRequest::Error(Error::HttpAdminError {
                    error: format!("Failed to bind HTTP stop port {port}: {e}"),
                }))
                .await
                .expect("Failed to send an OtlpRequest::Error");
        }
    }
}

/// Spawn a task that monitors for inactivity and triggers shutdown if timeout is reached
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
fn spawn_inactivity_monitor(
    stop_tx: mpsc::Sender<OtlpRequest>,
    activity_rx: watch::Receiver<Instant>,
    timeout: Duration,
    tasks: &mut JoinSet<()>,
) {
    let _ = tasks.spawn(async move {
        loop {
            // Wait for the timeout duration
            sleep(timeout).await;

            // Check if we've exceeded the inactivity timeout
            let last_activity = *activity_rx.borrow();
            if last_activity.elapsed() >= timeout {
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
        .map_err(|e| Status::resource_exhausted(format!("Channel full: {e}")))
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
    use crate::registry::otlp::grpc_stubs::proto::collector::logs::v1::logs_service_client::LogsServiceClient;
    use crate::registry::otlp::grpc_stubs::proto::collector::metrics::v1::metrics_service_client::MetricsServiceClient;
    use crate::registry::otlp::grpc_stubs::proto::collector::trace::v1::trace_service_client::TraceServiceClient;
    use std::thread;

    #[test]
    fn test_inactivity_stop_after_1_second() {
        let grpc_port = portpicker::pick_unused_port().expect("No free ports");
        let admin_port = portpicker::pick_unused_port().expect("No free ports");
        let inactivity_timeout = Duration::from_millis(500);

        let mut receiver =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();
        let grpc_endpoint = format!("http://127.0.0.1:{grpc_port}");
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
                            .inspect_err(|e| {
                                eprintln!(
                                    "Unable to connect to {grpc_endpoint_clone}. Error: {e}"
                                );
                            })
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
                    let mut traces_client = TraceServiceClient::connect(grpc_endpoint.clone())
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
                }
                other => {
                    panic!("Unexpected request: {other:?}");
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

        let mut receiver =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

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
                panic!("Expected OtlpRequest::Stop, got {other:?}");
            }
        }
    }
}
