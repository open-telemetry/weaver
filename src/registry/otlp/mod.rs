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
use miette::Diagnostic;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::net::{AddrParseError, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tonic::codegen::tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::log_warn;

/// Upper bound on how long the background thread has to drain the gRPC and
/// HTTP admin servers before shutdown is considered complete.
pub(crate) const ADMIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// State of the live-check report, published by `command()` and observed by
/// any number of `GET /live-check/report` callers.
#[derive(Clone)]
enum ReportState {
    NotRequested,
    Pending,
    Ready(Arc<(String, String)>), // (content_type, body)
}

/// Coordinates `/live-check/report` and shutdown across the gRPC server,
/// HTTP admin server, and background tasks. No lock here is ever held
/// across an `.await`; `begin_shutdown` is idempotent.
#[derive(Clone)]
pub struct AdminController {
    report: watch::Sender<ReportState>,
    /// When true, `/live-check/report` is enabled (`--output http`).
    expect_report: Arc<AtomicBool>,
    /// Cancelled once any stop signal fires.
    shutdown: CancellationToken,
}

impl AdminController {
    fn new() -> Self {
        Self {
            report: watch::Sender::new(ReportState::NotRequested),
            expect_report: Arc::new(AtomicBool::new(false)),
            shutdown: CancellationToken::new(),
        }
    }

    /// Enable `GET /live-check/report` (`--output http`).
    pub fn enable_http_report(&self) {
        self.expect_report.store(true, Ordering::Relaxed);
    }

    fn wants_report(&self) -> bool {
        self.expect_report.load(Ordering::Relaxed)
    }

    fn report_receiver(&self) -> watch::Receiver<ReportState> {
        self.report.subscribe()
    }

    /// Atomically transitions `NotRequested -> Pending`; `true` only for the
    /// caller that made the transition, so finalization triggers once.
    fn try_request_finalize(&self) -> bool {
        self.report.send_if_modified(|state| {
            if matches!(state, ReportState::NotRequested) {
                *state = ReportState::Pending;
                true
            } else {
                false
            }
        })
    }

    /// Publish the finalized report. Called unconditionally by `command()`
    /// once built, whether or not anyone ever requested it.
    pub fn publish_report(&self, content_type: String, body: String) {
        let _ = self.report.send_if_modified(|state| {
            *state = ReportState::Ready(Arc::new((content_type, body)));
            true
        });
    }

    /// Begin graceful shutdown. Idempotent — only the first call has an effect.
    pub fn begin_shutdown(&self) {
        self.shutdown.cancel();
    }

    /// A clone of the shutdown token, for tasks that need to race against it.
    fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }
}

/// Signals shutdown and drains the background thread on drop — guarantees
/// cleanup no matter how the owning scope exits, even before any stop
/// signal was sent.
pub(crate) struct AdminDrainGuard {
    controller: AdminController,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl AdminDrainGuard {
    pub(crate) fn new(controller: AdminController, handle: std::thread::JoinHandle<()>) -> Self {
        Self {
            controller,
            handle: Some(handle),
        }
    }
}

impl Drop for AdminDrainGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.controller.begin_shutdown();
            wait_for_admin_shutdown(handle);
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
    /// HTTP GET to /live-check/report
    ReportRequested,
}

impl Display for StopSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StopSignal::Sigint => f.write_str("SIGINT"),
            StopSignal::Sighup => f.write_str("SIGHUP"),
            StopSignal::AdminStop => f.write_str("ADMIN_STOP"),
            StopSignal::Inactivity => f.write_str("INACTIVITY"),
            StopSignal::ReportRequested => f.write_str("REPORT_REQUESTED"),
        }
    }
}

/// Bundles the admin controller, background-thread handle, and a
/// shutdown-requested signal for coordinating shutdown.
pub struct OtlpAdmin {
    pub controller: AdminController,
    pub handle: std::thread::JoinHandle<()>,
    /// Resolves once shutdown is requested via any path. Unbounded — it's
    /// up to the client when that happens.
    pub shutdown_requested: oneshot::Receiver<()>,
}

/// Start an OTLP receiver and return an iterator of requests alongside an
/// [`OtlpAdmin`] for shutdown coordination. The server is ready when this
/// returns `Ok`.
pub fn listen_otlp_requests(
    grpc_addr: &str,
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
) -> Result<(impl Iterator<Item = OtlpRequest>, OtlpAdmin), Error> {
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
    let (activity_tx, activity_rx) = watch::channel(Instant::now());
    let controller = AdminController::new();
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
    let (shutdown_requested_tx, shutdown_requested_rx) = oneshot::channel();

    // Background OS thread running a single-threaded Tokio runtime.
    let controller_clone = controller.clone();
    let handle = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build Tokio Runtime")
            .block_on(async {
                let mut tasks = JoinSet::new();

                // Signal shutdown_requested_rx once shutdown is requested (see OtlpAdmin).
                let shutdown_token_for_signal = controller_clone.shutdown_token();
                let _ = tasks.spawn(async move {
                    shutdown_token_for_signal.cancelled().await;
                    let _ = shutdown_requested_tx.send(());
                });

                // Every stop path races against the same shutdown token.
                spawn_stop_signal_handlers(
                    stop_tx.clone(),
                    controller_clone.shutdown_token(),
                    &mut tasks,
                );
                spawn_http_admin_handler(
                    stop_tx.clone(),
                    admin_port,
                    controller_clone.clone(),
                    &mut tasks,
                )
                .await;
                // Only spawn the inactivity monitor if the timeout is greater than zero
                if inactivity_timeout.as_secs() > 0 {
                    spawn_inactivity_monitor(
                        stop_tx.clone(),
                        activity_rx,
                        inactivity_timeout,
                        controller_clone.clone(),
                        &mut tasks,
                    );
                }

                let tokio_listener = TcpListener::from_std(listener)
                    .expect("Failed to convert std listener to tokio listener");
                let inbound = TcpListenerStream::new(tokio_listener);

                // Drains in-flight calls on shutdown instead of running forever.
                let server_future = Server::builder()
                    .add_service(LogsServiceServer::new(logs_service))
                    .add_service(MetricsServiceServer::new(metrics_service))
                    .add_service(TraceServiceServer::new(trace_service))
                    .serve_with_incoming_shutdown(
                        inbound,
                        controller_clone.shutdown_token().cancelled_owned(),
                    );

                ready_tx
                    .send(())
                    .expect("Failed to signal that the server is ready");

                // Bound the drain from when shutdown actually begins, not from
                // startup — otherwise long-running sessions would get
                // force-closed early.
                let shutdown_token = controller_clone.shutdown_token();
                tokio::pin!(server_future);
                let (grpc_result, drain_deadline) = tokio::select! {
                    () = shutdown_token.cancelled() => {
                        let deadline = tokio::time::Instant::now() + ADMIN_REQUEST_TIMEOUT;
                        (tokio::time::timeout_at(deadline, &mut server_future).await, deadline)
                    }
                    result = &mut server_future => {
                        // Listener died on its own; treat that as shutdown too.
                        shutdown_token.cancel();
                        (Ok(result), tokio::time::Instant::now() + ADMIN_REQUEST_TIMEOUT)
                    }
                };

                match grpc_result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        let _ = tx
                            .send(OtlpRequest::Error(Error::OtlpError {
                                error: format!("The OTLP listener encountered an error: {e}"),
                            }))
                            .await;
                    }
                    Err(_) => {
                        log_warn(
                            "Timed out waiting for the OTLP gRPC server to gracefully shut \
                             down; forcing it closed.",
                        );
                    }
                }

                // Bound how long we wait for tasks to finish; dropping the
                // JoinSet aborts any stragglers, so this always completes.
                if tokio::time::timeout_at(drain_deadline, tasks.join_all())
                    .await
                    .is_err()
                {
                    log_warn(
                        "Timed out waiting for OTLP receiver background tasks to shut down; \
                         aborting remaining tasks.",
                    );
                }
            });
    });

    // Wait until the server is ready
    ready_rx.blocking_recv().map_err(|e| Error::OtlpError {
        error: format!("OTLP server dropped before signaling readiness (error: {e})"),
    })?;

    Ok((
        SyncReceiver { receiver: rx },
        OtlpAdmin {
            controller,
            handle,
            shutdown_requested: shutdown_requested_rx,
        },
    ))
}

/// Outcome of [`join_with_timeout`].
#[derive(Debug, PartialEq, Eq)]
enum JoinOutcome {
    /// The thread finished normally within the timeout.
    Joined,
    /// The thread panicked (message already logged).
    Panicked,
    /// The timeout elapsed before the thread finished.
    TimedOut,
}

/// Join `handle` on a watchdog thread, bounded by `timeout`.
///
/// A plain blocking `JoinHandle::join` would have no way to give up, so this
/// runs it on its own thread and races that against `timeout` instead.
fn join_with_timeout(handle: std::thread::JoinHandle<()>, timeout: Duration) -> JoinOutcome {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let _ = std::thread::spawn(move || {
        let _ = done_tx.send(handle.join());
    });
    match done_rx.recv_timeout(timeout) {
        Ok(Ok(())) => JoinOutcome::Joined,
        Ok(Err(panic)) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_owned());
            log_warn(format!(
                "The OTLP receiver's background thread panicked: {message}"
            ));
            JoinOutcome::Panicked
        }
        Err(_) => JoinOutcome::TimedOut,
    }
}

/// Wait for the background thread from [`listen_otlp_requests`] to finish,
/// bounded so `process::exit` can't cut off an in-flight request but also
/// can't hang forever waiting for this.
pub fn wait_for_admin_shutdown(handle: std::thread::JoinHandle<()>) {
    const SHUTDOWN_WATCHDOG_GRACE: Duration = Duration::from_secs(5);

    match join_with_timeout(handle, ADMIN_REQUEST_TIMEOUT + SHUTDOWN_WATCHDOG_GRACE) {
        // Joined cleanly, or already logged its own diagnostic (panic).
        JoinOutcome::Joined | JoinOutcome::Panicked => {}
        JoinOutcome::TimedOut => {
            log_warn(
                "Timed out waiting for the OTLP receiver to shut down; exiting without \
                 confirming all in-flight requests were fully drained.",
            );
        }
    }
}

/// Spawn tasks to handle CTRL+C and SIGHUP, each racing the signal against
/// `shutdown` so it exits promptly if some other path stops first.
fn spawn_stop_signal_handlers(
    stop_tx: mpsc::Sender<OtlpRequest>,
    shutdown: CancellationToken,
    tasks: &mut JoinSet<()>,
) {
    // Handle CTRL+C
    let ctrl_c_tx = stop_tx.clone();
    let ctrl_c_shutdown = shutdown.clone();
    let _ = tasks.spawn(async move {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.expect("Failed to listen for CTRL+C");
                ctrl_c_shutdown.cancel();
                let _ = ctrl_c_tx
                    .send(OtlpRequest::Stop(StopSignal::Sigint))
                    .await
                    .ok();
            }
            _ = ctrl_c_shutdown.cancelled() => {}
        }
    });

    // Handle SIGHUP
    #[cfg(unix)]
    {
        let sighup_tx = stop_tx;
        let sighup_shutdown = shutdown;
        let _ = tasks.spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to create SIGHUP signal handler");

            tokio::select! {
                _ = sighup.recv() => {
                    sighup_shutdown.cancel();
                    let _ = sighup_tx
                        .send(OtlpRequest::Stop(StopSignal::Sighup))
                        .await
                        .ok();
                }
                _ = sighup_shutdown.cancelled() => {}
            }
        });
    }
}

/// Shared state for the admin HTTP handler.
#[derive(Clone)]
struct AdminState {
    stop_tx: mpsc::Sender<OtlpRequest>,
    controller: AdminController,
}

/// GET /health — returns a simple JSON status.
async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ready"}))
}

/// POST /stop — terminates the process. Carries no report; see
/// `GET /live-check/report` to retrieve one first.
async fn stop_handler(State(state): State<AdminState>) -> impl IntoResponse {
    state.controller.begin_shutdown();
    let _ = state
        .stop_tx
        .send(OtlpRequest::Stop(StopSignal::AdminStop))
        .await;
    StatusCode::OK.into_response()
}

/// GET /live-check/report — ends ingestion if needed and returns the
/// finalized report. Idempotent; requires `--output http`.
async fn report_handler(State(state): State<AdminState>) -> impl IntoResponse {
    if !state.controller.wants_report() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let mut rx = state.controller.report_receiver();
    if state.controller.try_request_finalize() {
        let _ = state
            .stop_tx
            .send(OtlpRequest::Stop(StopSignal::ReportRequested))
            .await;
    }

    // No timeout: the client decides how long to wait for finalization.
    let report = rx
        .wait_for(|s| matches!(s, ReportState::Ready(_)))
        .await
        .expect("report sender dropped");
    let ReportState::Ready(report) = &*report else {
        unreachable!("wait_for only resolves once state is Ready")
    };
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, report.0.clone())],
        report.1.clone(),
    )
        .into_response()
}

/// Spawn the HTTP admin server (/health, /stop, /live-check/report).
async fn spawn_http_admin_handler(
    stop_tx: mpsc::Sender<OtlpRequest>,
    port: u16,
    controller: AdminController,
    tasks: &mut JoinSet<()>,
) {
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Failed to parse HTTP admin port");

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let shutdown = controller.shutdown_token();
            let state = AdminState {
                stop_tx,
                controller,
            };

            let app = Router::new()
                .route("/health", get(health_handler))
                .route("/stop", post(stop_handler))
                .route("/live-check/report", get(report_handler))
                .with_state(state);

            let _ = tasks.spawn(async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move { shutdown.cancelled().await })
                    .await;
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

/// Monitors for inactivity and triggers shutdown. With `--output http`,
/// finalizes the report but leaves the shutdown token uncancelled, so the
/// process waits for an explicit stop instead of exiting before retrieval.
fn spawn_inactivity_monitor(
    stop_tx: mpsc::Sender<OtlpRequest>,
    activity_rx: watch::Receiver<Instant>,
    timeout: Duration,
    controller: AdminController,
    tasks: &mut JoinSet<()>,
) {
    let shutdown = controller.shutdown_token();
    let _ = tasks.spawn(async move {
        loop {
            tokio::select! {
                _ = sleep(timeout) => {}
                _ = shutdown.cancelled() => break,
            }

            let last_activity = *activity_rx.borrow();
            if last_activity.elapsed() >= timeout {
                if !controller.wants_report() {
                    shutdown.cancel();
                }
                let _ = stop_tx
                    .send(OtlpRequest::Stop(StopSignal::Inactivity))
                    .await
                    .ok();
                break;
            }

            // Sender dropped — stop monitoring.
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

        let (mut receiver, admin) =
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

        assert_joins_within(admin.handle, Duration::from_secs(5), "an inactivity stop");
    }

    /// Assert `handle` joins cleanly within `timeout` (a short one here, so
    /// a regression fails the test fast).
    fn assert_joins_within(handle: thread::JoinHandle<()>, timeout: Duration, context: &str) {
        match join_with_timeout(handle, timeout) {
            JoinOutcome::Joined => {}
            JoinOutcome::Panicked => panic!(
                "OTLP receiver background thread panicked instead of shutting down \
                 cleanly after {context}"
            ),
            JoinOutcome::TimedOut => panic!(
                "OTLP receiver background thread did not shut down within {timeout:?} \
                 after {context}"
            ),
        }
    }

    #[test]
    fn test_report_endpoint() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, admin) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

        // Enable report-via-HTTP mode (simulates --output http)
        admin.controller.enable_http_report();

        // Give the server a little time to finish binding the port.
        thread::sleep(Duration::from_millis(200));

        // Blocks until finalized, so it needs its own thread.
        let response_handle = thread::spawn(move || {
            let url = format!("http://127.0.0.1:{admin_port}/live-check/report");
            ureq::get(&url)
                .call()
                .expect("GET /live-check/report failed")
        });

        // Decoupled from shutdown: sends ReportRequested, not AdminStop.
        match receiver.next() {
            Some(OtlpRequest::Stop(StopSignal::ReportRequested)) => {
                admin
                    .controller
                    .publish_report("text/plain".into(), "test report".into());
            }
            other => {
                panic!("Expected OtlpRequest::Stop(ReportRequested), got {other:?}");
            }
        }

        let response = response_handle.join().expect("HTTP thread panicked");
        assert_eq!(
            response.status(),
            200,
            "Report endpoint returned non-200 status"
        );
        let body = response.into_body().read_to_string().unwrap();
        assert_eq!(body, "test report");

        // Idempotent: a second call serves the cached report.
        let url = format!("http://127.0.0.1:{admin_port}/live-check/report");
        let second = ureq::get(&url)
            .call()
            .expect("second GET /live-check/report failed");
        let second_body = second.into_body().read_to_string().unwrap();
        assert_eq!(second_body, "test report");

        // Retrieval must not shut anything down on its own.
        let health_url = format!("http://127.0.0.1:{admin_port}/health");
        assert!(
            ureq::get(&health_url).call().is_ok(),
            "admin server should still be reachable after /live-check/report"
        );

        // Now actually stop it.
        let stop_url = format!("http://127.0.0.1:{admin_port}/stop");
        let stop_response = ureq::post(&stop_url)
            .send("")
            .expect("HTTP POST to /stop failed");
        assert_eq!(stop_response.status(), 200);

        assert_joins_within(
            admin.handle,
            Duration::from_secs(5),
            "/stop after /live-check/report",
        );
    }

    #[test]
    fn test_stop_endpoint() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, admin) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

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

        assert_joins_within(admin.handle, Duration::from_secs(5), "/stop");
    }

    #[test]
    fn test_inactivity_with_report_mode_waits_for_stop() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(1);

        let (mut receiver, admin) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();
        admin.controller.enable_http_report();

        // Ends ingestion but must not cancel shutdown in report mode.
        match receiver.next() {
            Some(OtlpRequest::Stop(StopSignal::Inactivity)) => {}
            other => {
                panic!("Expected OtlpRequest::Stop(Inactivity), got {other:?}");
            }
        }

        thread::sleep(Duration::from_millis(200));
        let health_url = format!("http://127.0.0.1:{admin_port}/health");
        assert!(
            ureq::get(&health_url).call().is_ok(),
            "admin server should still be running after inactivity in report mode"
        );

        let stop_url = format!("http://127.0.0.1:{admin_port}/stop");
        let _ = ureq::post(&stop_url)
            .send("")
            .expect("HTTP POST to /stop failed");

        assert_joins_within(
            admin.handle,
            Duration::from_secs(5),
            "inactivity in report mode, then /stop",
        );
    }

    #[test]
    fn test_health_endpoint() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (_receiver, _admin) =
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
}
