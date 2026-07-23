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
use std::sync::{Arc, Mutex};
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

/// Upper bound on how long the admin server waits for a report during
/// `/stop`, and on how long the OTLP receiver's background thread is given
/// to gracefully drain the gRPC and HTTP admin servers before shutdown is
/// considered complete.
pub(crate) const ADMIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Extra time given to the background thread's drain beyond
/// `ADMIN_REQUEST_TIMEOUT`, so it can never abort the HTTP admin task while
/// `/stop` is still writing out its response (a delivered report, or its
/// own timeout-triggered 504) after its own `ADMIN_REQUEST_TIMEOUT`-bounded
/// wait for the report resolves.
///
/// Both clocks start at essentially the same moment (when shutdown begins),
/// so without this margin they'd expire together, leaving no time for
/// `/stop` to actually flush a response — reproducing the same stop-race
/// this whole module exists to prevent, just triggered by a slow report
/// instead of a missing thread-join.
pub(crate) const SHUTDOWN_DRAIN_GRACE: Duration = Duration::from_secs(30);

/// Coordinates the `/stop` report handshake and graceful shutdown across the
/// gRPC server, the HTTP admin server, and the background signal/inactivity
/// tasks that all run on the OTLP receiver's background thread.
///
/// This is the single owner of the cross-thread synchronization state for
/// that thread: no method here holds a lock across an `.await` or a blocking
/// wait, and `begin_shutdown` is idempotent so every stop path (CTRL+C,
/// SIGHUP, `/stop`, inactivity) can call it safely.
#[derive(Clone)]
pub struct AdminController {
    /// Slot for the oneshot sender the `/stop` handler registers while it
    /// waits for a report. `None` when no HTTP client is currently blocked
    /// on `/stop`. Single-consumer by construction: a live-check process
    /// only ever services one `/stop` call before it exits.
    report_tx: Arc<Mutex<Option<oneshot::Sender<(String, String)>>>>,
    /// When true, `/stop` waits for [`Self::deliver_report`] and returns the
    /// report as its response body. When false (the default), `/stop`
    /// returns 200 immediately.
    expect_report: Arc<AtomicBool>,
    /// Cancelled once any stop signal fires. Every long-lived task on the
    /// background thread races against this token so the thread is
    /// guaranteed to terminate within a bounded time.
    shutdown: CancellationToken,
}

impl AdminController {
    fn new() -> Self {
        Self {
            report_tx: Arc::new(Mutex::new(None)),
            expect_report: Arc::new(AtomicBool::new(false)),
            shutdown: CancellationToken::new(),
        }
    }

    /// Enable report-via-HTTP mode (`--output http`): `/stop` will wait for
    /// [`Self::deliver_report`] instead of returning `200` immediately.
    pub fn enable_http_report(&self) {
        self.expect_report.store(true, Ordering::Relaxed);
    }

    fn wants_report(&self) -> bool {
        self.expect_report.load(Ordering::Relaxed)
    }

    /// Register interest in the next report. Called once by the `/stop`
    /// handler; the returned receiver resolves when [`Self::deliver_report`]
    /// is called.
    fn register_report_waiter(&self) -> oneshot::Receiver<(String, String)> {
        let (tx, rx) = oneshot::channel();
        *self.report_tx.lock().expect("Report sender lock poisoned") = Some(tx);
        rx
    }

    /// Returns `true` if `/stop` has registered a waiter and is currently
    /// blocked on [`Self::deliver_report`]. Used to decide whether it's
    /// worth formatting a report at all.
    pub fn has_report_waiter(&self) -> bool {
        self.report_tx
            .lock()
            .expect("Report sender lock poisoned")
            .is_some()
    }

    /// Deliver the formatted report to a waiting `/stop` handler, if any.
    /// Returns `true` if a waiter was registered and the report was handed
    /// off successfully.
    pub fn deliver_report(&self, content_type: String, body: String) -> bool {
        let waiter = self
            .report_tx
            .lock()
            .expect("Report sender lock poisoned")
            .take();
        waiter.is_some_and(|tx| tx.send((content_type, body)).is_ok())
    }

    /// Begin graceful shutdown: the gRPC and HTTP admin listeners stop
    /// accepting new connections, and background signal/inactivity tasks
    /// exit. Idempotent — safe to call from multiple stop paths; only the
    /// first call has an effect.
    pub fn begin_shutdown(&self) {
        self.shutdown.cancel();
    }

    /// A clone of the shutdown token, for tasks that need to race against it.
    fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }
}

/// Signals shutdown and drains the OTLP receiver's background thread when
/// dropped.
///
/// Shared by every place that owns a `JoinHandle` returned from
/// [`listen_otlp_requests`] (directly, or via [`otlp_ingester::OtlpIngester::ingest_otlp`])
/// and needs to guarantee it's drained no matter how its own scope ends —
/// an early return can happen before any stop signal was ever sent, so
/// `Drop` requests shutdown itself first (`begin_shutdown` is idempotent,
/// so this is a no-op on the normal path where one already fired).
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
/// and return an iterator of received OTLP requests, an admin controller, and
/// the background thread's join handle.
///
/// The `AdminController` allows the caller to send a formatted report back
/// through the `/stop` HTTP endpoint. When `/stop` is called, the HTTP handler
/// registers a oneshot waiter and waits for the report.
///
/// The returned `JoinHandle` only resolves once the gRPC server, the HTTP
/// admin server, and all background tasks have gracefully shut down (bounded
/// by `ADMIN_REQUEST_TIMEOUT`). Callers that want to guarantee no in-flight
/// request is cut short by process exit should join it after handling the
/// stop signal.
///
/// This function guarantees that the OTLP server is started and ready when the
/// result is Ok(iterator).
pub fn listen_otlp_requests(
    grpc_addr: &str,
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
) -> Result<
    (
        impl Iterator<Item = OtlpRequest>,
        AdminController,
        std::thread::JoinHandle<()>,
    ),
    Error,
> {
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
    // Create the admin controller for /stop response and shutdown coordination
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

    // Start an OS thread and run a single threaded Tokio runtime inside.
    // The async OTLP receiver sends the received OTLP messages to the Tokio channel.
    let controller_clone = controller.clone();
    let handle = std::thread::spawn(move || {
        // Start a current threaded Tokio runtime
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build Tokio Runtime")
            .block_on(async {
                let mut tasks = JoinSet::new();

                // Spawn tasks to handle different stop signals. Every one of
                // them, plus the gRPC and HTTP admin servers below, races
                // against the same shutdown token so that whichever signal
                // fires first drains the whole receiver.
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
                        controller_clone.shutdown_token(),
                        &mut tasks,
                    );
                }

                let tokio_listener = TcpListener::from_std(listener)
                    .expect("Failed to convert std listener to tokio listener");
                let inbound = TcpListenerStream::new(tokio_listener);

                // Serve the OTLP services, gracefully draining in-flight
                // calls once shutdown is signalled instead of running forever.
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

                // `server_future` only resolves once a stop signal cancels
                // the shutdown token (or the listener hits a fatal error) —
                // a session otherwise runs indefinitely serving traffic, so
                // this must NOT be time-bounded from process startup (that
                // would force-close every long-running session regardless
                // of activity). Wait for shutdown to actually begin, then
                // bound how long the graceful drain — gRPC in-flight calls
                // first, then the background tasks below — may take,
                // measured from that moment rather than from startup.
                let shutdown_token = controller_clone.shutdown_token();
                tokio::pin!(server_future);
                let (grpc_result, drain_deadline) = tokio::select! {
                    () = shutdown_token.cancelled() => {
                        let deadline = tokio::time::Instant::now() + ADMIN_REQUEST_TIMEOUT;
                        (tokio::time::timeout_at(deadline, &mut server_future).await, deadline)
                    }
                    result = &mut server_future => {
                        // The gRPC listener ended on its own (a fatal
                        // transport error) without any stop signal ever
                        // being requested. Treat that as shutdown too, so
                        // the HTTP admin server and other tasks start
                        // draining right away instead of waiting on some
                        // other trigger (inactivity, a signal) — or, worst
                        // case, the drain timeout below — to notice.
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

                // Bound how long we wait for background tasks (signal
                // handlers, inactivity monitor, HTTP admin server) to notice
                // cancellation and finish. If this elapses, drop the
                // JoinSet anyway: Tokio aborts any still-running tasks on
                // drop, so this thread — and the JoinHandle we return to the
                // caller — is always guaranteed to complete.
                //
                // `SHUTDOWN_DRAIN_GRACE` on top of `drain_deadline` (rather
                // than reusing it as-is) gives the HTTP admin task — and
                // `/stop` in particular — room to actually flush its
                // response after its own ADMIN_REQUEST_TIMEOUT-bounded wait
                // for the report resolves, instead of racing to finish in
                // the same window.
                if tokio::time::timeout_at(drain_deadline + SHUTDOWN_DRAIN_GRACE, tasks.join_all())
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

    Ok((SyncReceiver { receiver: rx }, controller, handle))
}

/// Outcome of [`join_with_timeout`].
#[derive(Debug, PartialEq, Eq)]
enum JoinOutcome {
    /// The thread finished normally within the timeout.
    Joined,
    /// The thread panicked. `handle.join()`'s `Err` payload is logged where
    /// this is produced, not carried further, since callers only need to
    /// know shutdown didn't finish cleanly.
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

/// Wait for the OTLP receiver's background thread (as returned by
/// [`listen_otlp_requests`]) to finish shutting down.
///
/// Callers should call this after handling the stop signal and before
/// exiting the process, so `std::process::exit` can't cut off an in-flight
/// gRPC call or HTTP response (e.g. a `/stop` report) that's still being
/// flushed.
///
/// The thread is internally bounded to finish within `ADMIN_REQUEST_TIMEOUT`
/// plus `SHUTDOWN_DRAIN_GRACE` (its worst case: the gRPC drain step taking
/// its full budget, then the task-join step taking its own full grace on
/// top of that), so waiting a little longer than that here is a generous
/// backstop against an unanticipated hang; if it doesn't report back in
/// time we log a warning and move on rather than block process exit
/// forever.
pub fn wait_for_admin_shutdown(handle: std::thread::JoinHandle<()>) {
    const SHUTDOWN_WATCHDOG_GRACE: Duration = Duration::from_secs(5);

    match join_with_timeout(
        handle,
        ADMIN_REQUEST_TIMEOUT + SHUTDOWN_DRAIN_GRACE + SHUTDOWN_WATCHDOG_GRACE,
    ) {
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

/// Spawn tasks to handle CTRL+C and SIGHUP signals.
///
/// Each task races the signal against `shutdown` so that if some other stop
/// path (e.g. `/stop` or inactivity) triggers shutdown first, this task
/// still exits promptly instead of waiting forever for a signal that will
/// never come.
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
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

/// POST /stop — sends a stop signal. If `--output=http` was set, waits for
/// the report and returns it as the response body; otherwise returns 200
/// immediately.
async fn stop_handler(State(state): State<AdminState>) -> impl IntoResponse {
    // Begin shutdown immediately: this stops the gRPC/HTTP listeners from
    // accepting new connections, but hyper's graceful shutdown still drains
    // this in-flight request/response before the admin server task exits —
    // so it's safe to do this before we even know whether a report needs to
    // be produced.
    state.controller.begin_shutdown();

    if state.controller.wants_report() {
        let rx = state.controller.register_report_waiter();

        let _ = state
            .stop_tx
            .send(OtlpRequest::Stop(StopSignal::AdminStop))
            .await;

        match tokio::time::timeout(ADMIN_REQUEST_TIMEOUT, rx).await {
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
        }
    } else {
        let _ = state
            .stop_tx
            .send(OtlpRequest::Stop(StopSignal::AdminStop))
            .await;

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

/// Spawn a task that monitors for inactivity and triggers shutdown if timeout is reached.
///
/// Races the inactivity `sleep` against `shutdown` so that if some other
/// stop path triggers shutdown first, this task exits promptly instead of
/// sleeping out the rest of its timeout.
///
/// Note: All the tasks created in this function are recorded into a
/// JoinSet. `JoinSet::spawn` returns a `AbortHandle` that we can
/// ignore as we don't need to abort these tasks.
fn spawn_inactivity_monitor(
    stop_tx: mpsc::Sender<OtlpRequest>,
    activity_rx: watch::Receiver<Instant>,
    timeout: Duration,
    shutdown: CancellationToken,
    tasks: &mut JoinSet<()>,
) {
    let _ = tasks.spawn(async move {
        loop {
            tokio::select! {
                _ = sleep(timeout) => {}
                _ = shutdown.cancelled() => break,
            }

            // Check if we've exceeded the inactivity timeout
            let last_activity = *activity_rx.borrow();
            if last_activity.elapsed() >= timeout {
                shutdown.cancel();
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

        let (mut receiver, _report_sender, handle) =
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

        // Inactivity is one of the four stop paths that must cascade into a
        // full graceful shutdown of the gRPC + HTTP admin servers (gRPC has
        // no other shutdown trigger of its own). The background thread must
        // therefore become joinable within a bounded time.
        assert_joins_within(handle, Duration::from_secs(5), "an inactivity stop");
    }

    /// Assert `handle` joins cleanly within `timeout`, using the same
    /// watchdog primitive `wait_for_admin_shutdown` uses in production
    /// (with a much shorter timeout here, so a regression fails the test
    /// fast).
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
    fn test_http_stop_endpoint_with_report() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, report_sender, handle) =
            listen_otlp_requests("127.0.0.1", grpc_port, admin_port, inactivity_timeout).unwrap();

        // Enable report-via-HTTP mode (simulates --output http)
        report_sender.enable_http_report();

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
                let delivered =
                    report_sender.deliver_report("text/plain".into(), "test report".into());
                assert!(delivered, "Expected a report waiter to be registered");
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

        // The whole point of this coordination: the background thread must
        // only finish (and thus only let the process exit) once the report
        // response above has been fully handled.
        assert_joins_within(handle, Duration::from_secs(5), "/stop with a report");
    }

    #[test]
    fn test_http_stop_endpoint_immediate() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (mut receiver, _report_sender, handle) =
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

        assert_joins_within(handle, Duration::from_secs(5), "an immediate /stop");
    }

    #[test]
    fn test_health_endpoint() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let inactivity_timeout = Duration::from_secs(5);

        let (_receiver, _report_sender, _handle) =
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
