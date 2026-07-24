// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.

pub mod conversion;
pub mod otlp_ingester;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
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
use log::warn;
use miette::Diagnostic;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::net::{AddrParseError, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::sleep;
use tonic::codegen::tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};

/// How long `/stop` waits for the report, and how long the admin server's
/// graceful shutdown gets to finish delivering it (see
/// [`ShutdownCoordinator::wait_for_admin_shutdown`]).
const ADMIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// A poisoned lock means one of the threads responsible for reporting
/// status on shutdown has crashed. We kill the process here too, because
/// we're not sure whether our memory is still safe to use.
const LOCK_POISONED_MSG: &str = "one of the threads responsible for reporting status on \
     shutdown has crashed; killing the process because we're not sure whether our memory is \
     still safe to use";

/// Coordinates delivering the live-check report to a waiting `/stop`
/// request and confirming the admin server has finished writing it before
/// the process exits. All locking lives here; call sites only use these
/// methods.
#[derive(Clone)]
pub struct ShutdownCoordinator {
    /// `true` when `--output http` is set: `/stop` waits for and returns
    /// the report instead of responding immediately.
    expect_report: Arc<AtomicBool>,
    /// `None` when no `/stop` request is currently waiting; `.take()` in
    /// `deliver_report` ensures a report is only ever delivered once.
    report_slot: Arc<Mutex<Option<oneshot::Sender<(String, String)>>>>,
    /// Tells axum's `with_graceful_shutdown` to start draining.
    admin_shutdown_trigger_slot: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    /// Both halves of the admin-done signal, produced together in `new`
    /// and held behind one lock. Kept as independent `Option`s rather than
    /// `Option<(Sender, Receiver)>` because each half is consumed
    /// independently — by a different thread, at a different time.
    admin_done: Arc<Mutex<(Option<oneshot::Sender<()>>, Option<oneshot::Receiver<()>>)>>,
}

impl ShutdownCoordinator {
    /// Returns the coordinator plus the shutdown-trigger receiver, which
    /// the admin server wires into axum's `with_graceful_shutdown`.
    fn new() -> (Self, oneshot::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (admin_done_tx, admin_done_rx) = oneshot::channel();
        let coordinator = Self {
            expect_report: Arc::new(AtomicBool::new(false)),
            report_slot: Arc::new(Mutex::new(None)),
            admin_shutdown_trigger_slot: Arc::new(Mutex::new(Some(shutdown_tx))),
            admin_done: Arc::new(Mutex::new((Some(admin_done_tx), Some(admin_done_rx)))),
        };
        (coordinator, shutdown_rx)
    }

    /// Set when `--output http` is in effect. Pairs with the `Acquire` load
    /// in `expects_report` so `/stop` can never observe a stale `false`.
    pub fn set_expect_report(&self, expect: bool) {
        self.expect_report.store(expect, Ordering::Release);
    }

    pub fn expects_report(&self) -> bool {
        self.expect_report.load(Ordering::Acquire)
    }

    /// Registers a slot for the report and returns the receiver to await.
    fn begin_report_wait(&self) -> oneshot::Receiver<(String, String)> {
        let (tx, rx) = oneshot::channel();
        *self.report_slot.lock().expect(LOCK_POISONED_MSG) = Some(tx);
        rx
    }

    /// True while an HTTP client is registered and waiting for a report.
    pub fn is_report_pending(&self) -> bool {
        self.report_slot.lock().expect(LOCK_POISONED_MSG).is_some()
    }

    /// Hands the report to a waiting `/stop` request, if any.
    pub fn deliver_report(&self, content_type: String, body: String) {
        let sender = self.report_slot.lock().expect(LOCK_POISONED_MSG).take();
        if let Some(sender) = sender {
            let _ = sender.send((content_type, body));
        }
    }

    /// Starts the admin server's graceful shutdown.
    fn trigger_admin_shutdown(&self) {
        let tx = self
            .admin_shutdown_trigger_slot
            .lock()
            .expect(LOCK_POISONED_MSG)
            .take();
        if let Some(tx) = tx {
            let _ = tx.send(());
        }
    }

    /// Called once the admin server's graceful shutdown has finished.
    fn signal_admin_shutdown_complete(&self) {
        let tx = self.admin_done.lock().expect(LOCK_POISONED_MSG).0.take();
        if let Some(tx) = tx {
            let _ = tx.send(());
        }
    }

    /// Blocks until the admin server confirms its graceful shutdown has
    /// finished, so the process doesn't exit mid-write. No-op if already
    /// consumed.
    pub fn wait_for_admin_shutdown(&self) {
        let rx = self.admin_done.lock().expect(LOCK_POISONED_MSG).1.take();
        if let Some(rx) = rx {
            let _ = rx.blocking_recv();
        }
    }
}

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
                #[allow(dead_code)]
                #[path = "opentelemetry.proto.collector.logs.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod metrics {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[allow(dead_code)]
                #[path = "opentelemetry.proto.collector.metrics.v1.rs"]
                pub mod v1;
            }
            #[path = ""]
            pub mod trace {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[allow(dead_code)]
                #[path = "opentelemetry.proto.collector.trace.v1.rs"]
                pub mod v1;
            }
        }

        #[path = ""]
        pub mod logs {
            #[allow(rustdoc::invalid_html_tags)]
            #[allow(dead_code)]
            #[path = "opentelemetry.proto.logs.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod metrics {
            #[allow(rustdoc::invalid_html_tags)]
            #[allow(dead_code)]
            #[path = "opentelemetry.proto.metrics.v1.rs"]
            pub mod v1;
        }

        #[path = ""]
        pub mod trace {
            #[allow(rustdoc::invalid_html_tags)]
            #[allow(dead_code)]
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
/// and return an iterator of received OTLP requests and a shutdown coordinator.
///
/// The `ShutdownCoordinator` sends the report back through `/stop`, and lets
/// the caller wait for the admin server to finish delivering it before exiting.
///
/// This function guarantees that the OTLP server is started and ready when the
/// result is Ok(iterator).
pub fn listen_otlp_requests(
    grpc_addr: &str,
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
) -> Result<(impl Iterator<Item = OtlpRequest>, ShutdownCoordinator), Error> {
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
    let (coordinator, admin_shutdown_rx) = ShutdownCoordinator::new();
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
    let coordinator_clone = coordinator.clone();
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
                spawn_http_admin_handler(
                    stop_tx.clone(),
                    admin_port,
                    coordinator_clone,
                    admin_shutdown_rx,
                    &mut tasks,
                )
                .await;
                // Only spawn the inactivity monitor if the timeout is greater than zero
                if inactivity_timeout.as_secs() > 0 {
                    spawn_inactivity_monitor(
                        stop_tx.clone(),
                        activity_rx,
                        inactivity_timeout,
                        &mut tasks,
                    );
                }

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
        error: format!("OTLP server dropped before signaling readiness (error: {e})"),
    })?;

    Ok((SyncReceiver { receiver: rx }, coordinator))
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

/// Shared state for the admin HTTP handler.
#[derive(Clone)]
struct AdminState {
    stop_tx: mpsc::Sender<OtlpRequest>,
    coordinator: ShutdownCoordinator,
}

/// GET /health — returns a simple JSON status.
async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ready"}))
}

/// POST /stop — sends a stop signal. If `--output=http` was set, waits for
/// the report and returns it as the response body; otherwise returns 200
/// immediately.
async fn stop_handler(State(state): State<AdminState>) -> impl IntoResponse {
    if state.coordinator.expects_report() {
        let rx = state.coordinator.begin_report_wait();

        let _ = state
            .stop_tx
            .send(OtlpRequest::Stop(StopSignal::AdminStop))
            .await;

        let response = match tokio::time::timeout(ADMIN_REQUEST_TIMEOUT, rx).await {
            Ok(Ok((content_type, body))) => {
                (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], body).into_response()
            }
            Ok(Err(_)) => {
                // Channel dropped — report generation failed
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Report generation failed"})),
                )
                    .into_response()
            }
            Err(_) => {
                // Timeout
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(serde_json::json!({"error": "Timed out waiting for report"})),
                )
                    .into_response()
            }
        };

        state.coordinator.trigger_admin_shutdown();

        response
    } else {
        let _ = state
            .stop_tx
            .send(OtlpRequest::Stop(StopSignal::AdminStop))
            .await;

        state.coordinator.trigger_admin_shutdown();

        StatusCode::OK.into_response()
    }
}

/// Spawn a minimal HTTP server that handles admin endpoints (/health, /stop).
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
async fn spawn_http_admin_handler(
    stop_tx: mpsc::Sender<OtlpRequest>,
    port: u16,
    coordinator: ShutdownCoordinator,
    admin_shutdown_rx: oneshot::Receiver<()>,
    tasks: &mut JoinSet<()>,
) {
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Failed to parse HTTP admin port");

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let state = AdminState {
                stop_tx,
                coordinator: coordinator.clone(),
            };

            let app = Router::new()
                .route("/health", get(health_handler))
                .route("/stop", post(stop_handler))
                .with_state(state);

            let _ = tasks.spawn(async move {
                // Bounded so a stalled client can't hang the CLI forever.
                let result = tokio::time::timeout(ADMIN_REQUEST_TIMEOUT, async {
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = admin_shutdown_rx.await;
                        })
                        .await
                })
                .await;

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => warn!("Admin HTTP server error: {e}"),
                    Err(_) => warn!(
                        "Admin HTTP server graceful shutdown did not complete within \
                         {ADMIN_REQUEST_TIMEOUT:?}; a client may not have finished reading \
                         a response"
                    ),
                }

                coordinator.signal_admin_shutdown_complete();
            });
        }
        Err(e) => {
            stop_tx
                .send(OtlpRequest::Error(Error::HttpAdminError {
                    error: format!("Failed to bind HTTP admin port {port}: {e}"),
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
    use weaver_test_support::reserve_test_port;

    #[test]
    fn test_inactivity_stop_after_1_second() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(1);

        let (mut receiver, _report_sender) =
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
                                eprintln!("Unable to connect to {grpc_endpoint_clone}. Error: {e}");
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
    fn test_http_stop_endpoint_with_report() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, report_sender) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

        // Enable report-via-HTTP mode (simulates --output http)
        report_sender.set_expect_report(true);

        // Give the server a little time to finish binding the port.
        thread::sleep(Duration::from_millis(200));

        // The HTTP handler now waits for a report before responding, so the
        // POST must be on a separate thread.
        let response_handle = thread::spawn(move || {
            let url = format!("http://127.0.0.1:{admin_port}/stop");
            ureq::post(&url)
                .send("")
                .expect("HTTP POST to /stop failed")
        });

        // Wait for the Stop signal and then send the report
        match receiver.next() {
            Some(OtlpRequest::Stop(StopSignal::AdminStop)) => {
                report_sender.deliver_report("text/plain".into(), "test report".into());
            }
            other => {
                panic!("Expected OtlpRequest::Stop, got {other:?}");
            }
        }

        let response = response_handle.join().expect("HTTP thread panicked");
        assert_eq!(
            response.status(),
            200,
            "Stop endpoint returned non-200 status"
        );
        let body = response.into_body().read_to_string().unwrap();
        assert_eq!(body, "test report");
    }

    #[test]
    fn test_http_stop_endpoint_immediate() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, _report_sender) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

        // expect_report defaults to false — /stop should return 200 immediately

        // Give the server a little time to finish binding the port.
        thread::sleep(Duration::from_millis(200));

        let url = format!("http://127.0.0.1:{admin_port}/stop");
        let response = ureq::post(&url)
            .send("")
            .expect("HTTP POST to /stop failed");
        assert_eq!(
            response.status(),
            200,
            "Stop endpoint returned non-200 status"
        );
        let body = response.into_body().read_to_string().unwrap();
        assert!(body.is_empty(), "Expected empty body, got: {body}");

        // Should still receive the stop signal
        match receiver.next() {
            Some(OtlpRequest::Stop(StopSignal::AdminStop)) => {}
            other => {
                panic!("Expected OtlpRequest::Stop, got {other:?}");
            }
        }
    }

    #[test]
    fn test_health_endpoint() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (_receiver, _report_sender) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

        // Give the server a little time to finish binding the port.
        thread::sleep(Duration::from_millis(200));

        // First health check
        let url = format!("http://127.0.0.1:{admin_port}/health");
        let response = ureq::get(&url).call().expect("GET /health failed");
        assert_eq!(response.status(), 200);
        let body = response.into_body().read_to_string().unwrap();
        assert_eq!(body, r#"{"status":"ready"}"#);

        // Second health check — server should still be running
        let response2 = ureq::get(&url).call().expect("GET /health (2nd) failed");
        assert_eq!(response2.status(), 200);
    }

    #[test]
    fn test_deliver_report_noop_without_pending_request() {
        let (coordinator, _admin_shutdown_rx) = ShutdownCoordinator::new();
        assert!(!coordinator.is_report_pending());
        // Should not panic even though nothing is waiting.
        coordinator.deliver_report("text/plain".into(), "unused".into());
    }

    #[test]
    fn test_report_pending_state_transitions() {
        let (coordinator, _admin_shutdown_rx) = ShutdownCoordinator::new();
        assert!(!coordinator.is_report_pending());

        let report_rx = coordinator.begin_report_wait();
        assert!(coordinator.is_report_pending());

        coordinator.deliver_report("text/plain".into(), "hello".into());
        assert!(!coordinator.is_report_pending());

        let (content_type, body) = report_rx.blocking_recv().expect("report was not delivered");
        assert_eq!(content_type, "text/plain");
        assert_eq!(body, "hello");
    }

    #[test]
    fn test_wait_for_admin_shutdown_blocks_until_signaled() {
        let (coordinator, _admin_shutdown_rx) = ShutdownCoordinator::new();
        let coordinator_clone = coordinator.clone();

        let (signaled_tx, signaled_rx) = std::sync::mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            signaled_tx.send(()).expect("main thread went away");
            coordinator_clone.signal_admin_shutdown_complete();
        });

        // Proves the wait below actually blocks rather than trivially returning.
        assert!(signaled_rx.try_recv().is_err());

        coordinator.wait_for_admin_shutdown();

        assert!(
            signaled_rx.try_recv().is_ok(),
            "wait_for_admin_shutdown returned before signal_admin_shutdown_complete ran"
        );

        handle.join().expect("background thread panicked");
    }
}
