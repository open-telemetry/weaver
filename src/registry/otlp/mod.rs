// SPDX-License-Identifier: Apache-2.0

//! A basic OTLP receiver integrated into Weaver.
//!
//! One axum app carries both the OTLP/gRPC services ([`receiver`]) and the
//! live-check admin API ([`admin`]). It runs on one background thread with one
//! runtime, and stops on a signal, on inactivity, on `POST /stop`, or when the
//! checker is done.

pub mod admin;
pub mod conversion;
pub mod otlp_ingester;
pub mod receiver;

use std::fmt::{Display, Formatter};
use std::future::IntoFuture;
use std::net::{AddrParseError, SocketAddr};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use admin::{AppState, Report};
use axum::Router;
use grpc_stubs::proto::collector::logs::v1::ExportLogsServiceRequest;
use grpc_stubs::proto::collector::metrics::v1::ExportMetricsServiceRequest;
use grpc_stubs::proto::collector::profiles::v1development::ExportProfilesServiceRequest;
use grpc_stubs::proto::collector::trace::v1::ExportTraceServiceRequest;
use log::warn;
use miette::Diagnostic;
use receiver::OtlpReceiver;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::sleep;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};

/// Exports queued ahead of the checker before exporters have to wait.
const CHANNEL_CAPACITY: usize = 100;

/// A bound socket and whether it carries the admin API.
struct BoundListener {
    listener: std::net::TcpListener,
    /// Shut down gracefully, so the in-flight `/shutdown` response completes.
    /// There is no deadline: the client asked for the exit, so anything else it
    /// left in flight is its own to finish. The gRPC listener just stops; after
    /// a stop it has nothing left to deliver, and SDKs keep idle HTTP/2
    /// connections open, which a graceful shutdown would wait for.
    serves_admin: bool,
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
            #[path = ""]
            pub mod profiles {
                #[allow(unused_qualifications)]
                #[allow(unused_results)]
                #[allow(clippy::enum_variant_names)]
                #[allow(rustdoc::invalid_html_tags)]
                #[allow(dead_code)]
                #[path = "opentelemetry.proto.collector.profiles.v1development.rs"]
                pub mod v1development;
            }
        }

        #[path = ""]
        pub mod profiles {
            #[allow(rustdoc::invalid_html_tags)]
            #[allow(dead_code)]
            #[path = "opentelemetry.proto.profiles.v1development.rs"]
            pub mod v1development;
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

/// Errors emitted by the OTLP receiver.
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// An OTLP error occurred.
    #[error("The following OTLP error occurred: {error}")]
    OtlpError { error: String },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// A received OTLP export, or the event that ends a run.
#[derive(Debug)]
pub enum OtlpRequest {
    Logs(ExportLogsServiceRequest),
    Metrics(ExportMetricsServiceRequest),
    Traces(ExportTraceServiceRequest),
    Profiles(ExportProfilesServiceRequest),

    Error(Error),
    Stop(StopSignal),
}

/// Why a run stopped.
#[derive(Debug)]
pub enum StopSignal {
    /// CTRL+C
    Sigint,
    /// SIGHUP
    Sighup,
    /// HTTP POST to /stop or /shutdown
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

/// A running listener.
pub struct OtlpListener {
    /// The exports, ending with the `Stop` or `Error` that ended the run.
    pub requests: SyncReceiver,
    /// Where the gRPC services listen. Tells the real port when `0` was asked for.
    pub grpc_addr: SocketAddr,
    /// Where the admin API listens. Tells the real port when `0` was asked for.
    pub admin_addr: SocketAddr,
    /// Publishes the outcome of the run and ends the listener.
    pub handle: ListenerHandle,
}

/// The checker's side of the listener: says how the run ended, then waits for
/// the listener thread to finish.
pub struct ListenerHandle {
    state: Arc<AppState>,
    thread: Option<JoinHandle<()>>,
}

impl ListenerHandle {
    /// The run is over and its report went to stdout or a directory. A waiting
    /// `/stop` returns, the listeners drain, and this returns once the thread ends.
    pub fn finish(mut self) {
        self.state.stopped(None);
        self.state.request_shutdown();
        self.join();
    }

    /// The run is over and its report is served on `/report` until `/shutdown`
    /// or a signal ends the listener. Whether anyone reads it is the client's
    /// business; it asked for the report over HTTP.
    pub fn serve_report(mut self, report: Report) {
        self.state.stopped(Some(report));
        self.join();
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ListenerHandle {
    /// A handle dropped without `finish` or `serve_report` still ends the listener.
    fn drop(&mut self) {
        self.state.request_shutdown();
        self.join();
    }
}

/// The blocking side of the export channel.
///
/// Once it yields the `Stop` or `Error` that ends a run it closes the channel,
/// so any export still arriving is refused at once instead of waiting on a
/// queue nobody reads.
pub struct SyncReceiver {
    receiver: mpsc::Receiver<OtlpRequest>,
}

impl Iterator for SyncReceiver {
    type Item = OtlpRequest;

    fn next(&mut self) -> Option<Self::Item> {
        let request = self.receiver.blocking_recv()?;
        if matches!(request, OtlpRequest::Stop(_) | OtlpRequest::Error(_)) {
            self.receiver.close();
        }
        Some(request)
    }
}

/// Starts the OTLP/gRPC services and the admin API and returns the exports as
/// an iterator.
///
/// Both ports bind to `grpc_address` and must differ. Port `0` picks a free
/// port for either; read them from [`OtlpListener::grpc_addr`] and
/// [`OtlpListener::admin_addr`]. The sockets are bound before this returns, so
/// exporters can connect at once. An `inactivity_timeout` of zero never stops
/// the run on its own.
pub fn listen_otlp_requests(
    grpc_address: &str,
    grpc_port: u16,
    admin_port: u16,
    inactivity_timeout: Duration,
) -> Result<OtlpListener, Error> {
    let parse = |port: u16| -> Result<SocketAddr, Error> {
        format!("{grpc_address}:{port}")
            .parse()
            .map_err(|e: AddrParseError| otlp_error(e))
    };
    let grpc_addr = parse(grpc_port)?;
    let admin_addr = parse(admin_port)?;
    if grpc_port != 0 && grpc_addr == admin_addr {
        return Err(otlp_error(format!(
            "the OTLP gRPC port and the admin port must differ; both are {grpc_port}"
        )));
    }

    let (exports, requests) = mpsc::channel(CHANNEL_CAPACITY);
    let state = Arc::new(AppState::new(exports));
    let router = OtlpReceiver::new(state.clone())
        .grpc_router()
        .merge(admin::router(state.clone()));

    let bind = |addr: SocketAddr, serves_admin: bool| -> Result<BoundListener, Error> {
        let listener = std::net::TcpListener::bind(addr).map_err(otlp_error)?;
        listener.set_nonblocking(true).map_err(otlp_error)?;
        Ok(BoundListener {
            listener,
            serves_admin,
        })
    };
    let grpc = bind(grpc_addr, false)?;
    let admin = bind(admin_addr, true)?;
    let grpc_addr = grpc.listener.local_addr().map_err(otlp_error)?;
    let admin_addr = admin.listener.local_addr().map_err(otlp_error)?;
    let listeners = vec![grpc, admin];

    // Built here so a failure is an error, not a panic on the thread.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(otlp_error)?;
    let thread_state = state.clone();
    let thread = std::thread::Builder::new()
        .name("otlp-listener".to_owned())
        .spawn(move || {
            runtime.block_on(serve_all(
                listeners,
                router,
                thread_state,
                inactivity_timeout,
            ));
        })
        .map_err(otlp_error)?;

    Ok(OtlpListener {
        requests: SyncReceiver { receiver: requests },
        grpc_addr,
        admin_addr,
        handle: ListenerHandle {
            state,
            thread: Some(thread),
        },
    })
}

/// Serves the router on every listener until shutdown is requested, then lets
/// the admin side finish what is in flight.
async fn serve_all(
    listeners: Vec<BoundListener>,
    router: Router,
    state: Arc<AppState>,
    inactivity_timeout: Duration,
) {
    let mut tasks = JoinSet::new();
    spawn_stop_signal_handlers(state.clone(), &mut tasks);
    if !inactivity_timeout.is_zero() {
        spawn_inactivity_monitor(state.clone(), inactivity_timeout, &mut tasks);
    }

    let mut servers = JoinSet::new();
    for bound in listeners {
        let listener = match TcpListener::from_std(bound.listener) {
            Ok(listener) => listener,
            Err(e) => {
                report_error(&state, format!("The OTLP listener failed to start: {e}")).await;
                return;
            }
        };
        let mut shutdown = state.shutdown.subscribe();
        let serve = axum::serve(listener, router.clone());
        if bound.serves_admin {
            let _ = servers.spawn(
                serve
                    .with_graceful_shutdown(async move {
                        let _ = shutdown.wait_for(|stop| *stop).await;
                    })
                    .into_future(),
            );
        } else {
            // Stop accepting at once. Open connections end with the runtime.
            let _ = servers.spawn(async move {
                tokio::select! {
                    result = serve.into_future() => result,
                    _ = shutdown.wait_for(|stop| *stop) => Ok(()),
                }
            });
        }
    }

    while let Some(result) = servers.join_next().await {
        if let Ok(Err(e)) = result {
            report_error(
                &state,
                format!("The OTLP listener encountered an error: {e}"),
            )
            .await;
        }
    }
    tasks.abort_all();
}

async fn report_error(state: &AppState, error: String) {
    let _ = state
        .exports
        .send(OtlpRequest::Error(Error::OtlpError { error }))
        .await;
}

/// A signal stops the run; once stopped, a signal ends the process.
async fn on_signal(state: &AppState, signal: StopSignal) -> bool {
    if state.is_receiving() {
        let _ = state.exports.send(OtlpRequest::Stop(signal)).await;
        true
    } else {
        state.request_shutdown();
        false
    }
}

fn spawn_stop_signal_handlers(state: Arc<AppState>, tasks: &mut JoinSet<()>) {
    let ctrl_c_state = state.clone();
    let _ = tasks.spawn(async move {
        loop {
            if let Err(e) = tokio::signal::ctrl_c().await {
                warn!("Failed to listen for CTRL+C: {e}");
                return;
            }
            if !on_signal(&ctrl_c_state, StopSignal::Sigint).await {
                return;
            }
        }
    });

    #[cfg(unix)]
    {
        let _ = tasks.spawn(async move {
            let mut sighup =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
                    Ok(sighup) => sighup,
                    Err(e) => {
                        warn!("Failed to listen for SIGHUP: {e}");
                        return;
                    }
                };
            while sighup.recv().await.is_some() {
                if !on_signal(&state, StopSignal::Sighup).await {
                    return;
                }
            }
        });
    }
}

/// Stops the run once `timeout` has passed without an export. It only ever
/// stops receiving; what happens after that is the client's call.
fn spawn_inactivity_monitor(state: Arc<AppState>, timeout: Duration, tasks: &mut JoinSet<()>) {
    // Checking every second keeps the stop close to the timeout itself,
    // rather than up to a whole timeout late.
    let interval = timeout.min(Duration::from_secs(1));
    let _ = tasks.spawn(async move {
        loop {
            sleep(interval).await;
            if !state.is_receiving() {
                return;
            }
            if state.activity.borrow().elapsed() >= timeout {
                let _ = state
                    .exports
                    .send(OtlpRequest::Stop(StopSignal::Inactivity))
                    .await;
                return;
            }
        }
    });
}

fn otlp_error(error: impl ToString) -> Error {
    Error::OtlpError {
        error: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::otlp::grpc_stubs::proto::collector::logs::v1::logs_service_client::LogsServiceClient;
    use crate::registry::otlp::grpc_stubs::proto::collector::metrics::v1::metrics_service_client::MetricsServiceClient;
    use crate::registry::otlp::grpc_stubs::proto::collector::trace::v1::trace_service_client::TraceServiceClient;
    use axum::body::Bytes;
    use std::thread;
    use std::time::Instant;

    /// Both ports `0`: two free ports, reported on the listener.
    fn listen_ephemeral(inactivity_timeout: Duration) -> OtlpListener {
        listen_otlp_requests("127.0.0.1", 0, 0, inactivity_timeout).expect("listen")
    }

    fn client_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("client runtime")
    }

    async fn export_all(endpoint: &str, metrics: usize, logs: usize, traces: usize) {
        let mut client = MetricsServiceClient::connect(endpoint.to_owned())
            .await
            .expect("connect metrics");
        for _ in 0..metrics {
            let _ = client
                .export(ExportMetricsServiceRequest::default())
                .await
                .expect("export metrics");
        }
        let mut client = LogsServiceClient::connect(endpoint.to_owned())
            .await
            .expect("connect logs");
        for _ in 0..logs {
            let _ = client
                .export(ExportLogsServiceRequest::default())
                .await
                .expect("export logs");
        }
        let mut client = TraceServiceClient::connect(endpoint.to_owned())
            .await
            .expect("connect traces");
        for _ in 0..traces {
            let _ = client
                .export(ExportTraceServiceRequest::default())
                .await
                .expect("export traces");
        }
    }

    /// Reads exports until the run stops and returns the stop signal.
    fn read_until_stop(requests: &mut SyncReceiver) -> StopSignal {
        for request in requests.by_ref() {
            if let OtlpRequest::Stop(signal) = request {
                return signal;
            }
        }
        panic!("the requests ended without a stop");
    }

    fn body_of(response: ureq::http::Response<ureq::Body>) -> String {
        response
            .into_body()
            .read_to_string()
            .expect("read the body")
    }

    #[test]
    fn an_ephemeral_port_is_reported_and_inactivity_stops_the_run() {
        let listener = listen_ephemeral(Duration::from_secs(1));
        assert_ne!(listener.grpc_addr.port(), 0, "the bound port is reported");
        let endpoint = format!("http://{}", listener.grpc_addr);

        client_runtime().block_on(export_all(&endpoint, 3, 4, 5));

        let (mut metrics, mut logs, mut traces) = (0, 0, 0);
        for request in listener.requests {
            match request {
                OtlpRequest::Metrics(_) => metrics += 1,
                OtlpRequest::Logs(_) => logs += 1,
                OtlpRequest::Traces(_) => traces += 1,
                OtlpRequest::Stop(StopSignal::Inactivity) => break,
                other => panic!("unexpected request: {other:?}"),
            }
        }
        assert_eq!((metrics, logs, traces), (3, 4, 5));
        listener.handle.finish();
    }

    #[test]
    fn equal_ports_are_rejected() {
        let error = listen_otlp_requests("127.0.0.1", 4317, 4317, Duration::ZERO)
            .err()
            .expect("equal ports are refused");
        assert!(error.to_string().contains("must differ"), "{error}");
    }

    #[test]
    fn health_answers_on_the_admin_port() {
        let listener = listen_ephemeral(Duration::ZERO);
        assert_ne!(listener.grpc_addr.port(), 0);
        assert_ne!(listener.admin_addr.port(), 0);
        assert_ne!(listener.grpc_addr, listener.admin_addr);

        let response = ureq::get(format!("http://{}/health", listener.admin_addr))
            .call()
            .expect("GET /health");
        assert_eq!(response.status(), 200);
        assert_eq!(body_of(response), r#"{"status":"ready"}"#);

        // The gRPC port serves the same router, so it answers too.
        let response = ureq::get(format!("http://{}/health", listener.grpc_addr))
            .call()
            .expect("GET /health on the gRPC port");
        assert_eq!(response.status(), 200);

        listener.handle.finish();
    }

    #[test]
    fn stop_returns_once_the_run_is_over() {
        let listener = listen_ephemeral(Duration::ZERO);
        let OtlpListener {
            mut requests,
            admin_addr,
            handle,
            ..
        } = listener;

        let stop = thread::spawn(move || {
            ureq::post(format!("http://{admin_addr}/stop"))
                .send_empty()
                .expect("POST /stop")
        });

        assert!(matches!(
            read_until_stop(&mut requests),
            StopSignal::AdminStop
        ));
        // /stop is still waiting: the outcome is not published yet.
        assert!(!stop.is_finished());
        handle.finish();

        let response = stop.join().expect("stop thread");
        assert_eq!(response.status(), 200);
        assert_eq!(body_of(response), r#"{"state":"stopped","report":false}"#);
    }

    #[test]
    fn the_report_is_served_until_shutdown() {
        // A failed assertion on the client thread must not hang the test, so
        // inactivity ends the server if the client never asks for shutdown.
        let listener = listen_ephemeral(Duration::from_secs(10));
        let OtlpListener {
            mut requests,
            admin_addr,
            handle,
            ..
        } = listener;
        let base = format!("http://{admin_addr}");

        let before_stop = ureq::get(format!("{base}/report"))
            .call()
            .expect_err("no report while receiving");
        assert!(matches!(before_stop, ureq::Error::StatusCode(409)));

        let client = thread::spawn(move || {
            let stop = ureq::post(format!("{base}/stop"))
                .send_empty()
                .expect("POST /stop");
            assert_eq!(body_of(stop), r#"{"state":"stopped","report":true}"#);

            let report = ureq::get(format!("{base}/report"))
                .call()
                .expect("GET /report");
            assert_eq!(
                report.headers().get("content-type").map(|v| v.as_bytes()),
                Some(b"text/plain".as_slice())
            );
            assert_eq!(body_of(report), "the report");

            // Reading it twice is fine; nothing is consumed.
            let again = ureq::get(format!("{base}/report"))
                .call()
                .expect("GET /report again");
            assert_eq!(body_of(again), "the report");

            let shutdown = ureq::post(format!("{base}/shutdown"))
                .send_empty()
                .expect("POST /shutdown");
            assert_eq!(body_of(shutdown), r#"{"state":"shutting_down"}"#);
        });

        assert!(matches!(
            read_until_stop(&mut requests),
            StopSignal::AdminStop
        ));
        handle.serve_report(Report {
            content_type: "text/plain".to_owned(),
            body: Bytes::from_static(b"the report"),
        });
        client.join().expect("client thread");
    }

    #[test]
    fn a_report_after_an_inactivity_stop_waits_for_shutdown() {
        let timeout = Duration::from_millis(300);
        let listener = listen_ephemeral(timeout);
        let OtlpListener {
            mut requests,
            admin_addr,
            handle,
            ..
        } = listener;

        assert!(matches!(
            read_until_stop(&mut requests),
            StopSignal::Inactivity
        ));

        // Well past another inactivity period, the report is still there:
        // inactivity never ends a stopped run. Only the client does.
        let client = thread::spawn(move || {
            thread::sleep(timeout * 3);
            let base = format!("http://{admin_addr}");
            let report = ureq::get(format!("{base}/report"))
                .call()
                .expect("GET /report");
            assert_eq!(body_of(report), "still here");
            let _ = ureq::post(format!("{base}/shutdown"))
                .send_empty()
                .expect("POST /shutdown");
        });

        handle.serve_report(Report {
            content_type: "text/plain".to_owned(),
            body: Bytes::from_static(b"still here"),
        });
        client.join().expect("client thread");
    }

    #[test]
    fn an_export_after_the_stop_is_refused() {
        let listener = listen_ephemeral(Duration::from_millis(200));
        let OtlpListener {
            mut requests,
            grpc_addr,
            handle,
            ..
        } = listener;

        // Reading the stop closes the channel.
        assert!(matches!(
            read_until_stop(&mut requests),
            StopSignal::Inactivity
        ));

        let status = client_runtime().block_on(async {
            let mut client = LogsServiceClient::connect(format!("http://{grpc_addr}"))
                .await
                .expect("connect logs");
            client
                .export(ExportLogsServiceRequest::default())
                .await
                .expect_err("the export is refused")
        });
        assert_eq!(status.code(), tonic::Code::Unavailable);

        // The refused export holds nothing open, so this returns at once.
        let started = Instant::now();
        handle.finish();
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn an_idle_grpc_client_does_not_delay_the_exit() {
        let listener = listen_ephemeral(Duration::ZERO);
        let OtlpListener {
            mut requests,
            grpc_addr,
            handle,
            ..
        } = listener;

        // An SDK keeps its HTTP/2 connection open after exporting. Hold one
        // across the shutdown and make sure the exit does not wait for it.
        let runtime = client_runtime();
        let mut client = runtime.block_on(async {
            let mut client = LogsServiceClient::connect(format!("http://{grpc_addr}"))
                .await
                .expect("connect logs");
            let _ = client
                .export(ExportLogsServiceRequest::default())
                .await
                .expect("export logs");
            client
        });
        assert!(matches!(requests.next(), Some(OtlpRequest::Logs(_))));

        let started = Instant::now();
        handle.finish();
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "finish took {:?} with an idle gRPC client connected",
            started.elapsed()
        );

        // The connection is gone from the client's point of view too.
        let refused = runtime.block_on(client.export(ExportLogsServiceRequest::default()));
        assert!(refused.is_err());
    }

    #[test]
    fn shutdown_while_receiving_stops_the_run_first() {
        let listener = listen_ephemeral(Duration::ZERO);
        let OtlpListener {
            mut requests,
            admin_addr,
            handle,
            ..
        } = listener;

        let response = ureq::post(format!("http://{admin_addr}/shutdown"))
            .send_empty()
            .expect("POST /shutdown");
        assert_eq!(response.status(), 200);

        assert!(matches!(
            read_until_stop(&mut requests),
            StopSignal::AdminStop
        ));
        // The report is published to a listener that is already ending. That is
        // the client's choice; nothing else is done with it.
        handle.serve_report(Report {
            content_type: "text/plain".to_owned(),
            body: Bytes::from_static(b"too late"),
        });
    }
}
