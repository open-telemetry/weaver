// SPDX-License-Identifier: Apache-2.0

//! The live-check admin API and the state it shares with the OTLP receiver.
//!
//! A run has three phases. While it is *receiving*, exports flow to the checker.
//! Once it is *stopped*, the report is final. With `--output http` it is served
//! on `/report` until the run is *shutting down*, when the process exits.
//! Reading the report is separate from exiting, so a large body is never cut
//! off (#1657).
//!
//! The phase is the whole state of the run, and the one thing everyone waits on.
//!
//! Weaver acts on what it is told, a flag or a request, and never invents an
//! exit of its own. There are no timers here. Every wait ends when the client
//! acts, and `/shutdown` finishes the responses in flight and exits.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::info;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

use super::{OtlpRequest, StopSignal};

/// The rendered report, ready to serve.
#[derive(Clone, Debug)]
pub struct Report {
    /// The `Content-Type` of the body, from the output format.
    pub content_type: String,
    /// The whole report. `Bytes`, so serving it again is a refcount, not a copy.
    pub body: Bytes,
}

/// The state of a run. Each phase carries the data that only exists in it.
#[derive(Clone, Debug)]
pub enum Phase {
    /// Exports are checked as they arrive.
    Receiving {
        /// When the last export arrived, for the inactivity timeout.
        last_export: Instant,
    },
    /// The report is final and served on `/report`.
    Stopped { report: Report },
    /// The listeners are closing and the process is about to exit.
    ShuttingDown,
}

/// Everything the receiver and the admin handlers share.
pub struct AppState {
    /// Exports, and the `Stop` that ends them, in order.
    pub(super) exports: mpsc::Sender<OtlpRequest>,
    /// The phase of the run, and the only state anyone waits on. `/stop` waits
    /// for it to leave `Receiving`; the listeners wait for `ShuttingDown`.
    pub(super) phase: watch::Sender<Phase>,
}

impl AppState {
    pub(super) fn new(exports: mpsc::Sender<OtlpRequest>) -> Self {
        Self {
            exports,
            phase: watch::Sender::new(Phase::Receiving {
                last_export: Instant::now(),
            }),
        }
    }

    pub(super) fn is_receiving(&self) -> bool {
        matches!(*self.phase.borrow(), Phase::Receiving { .. })
    }

    /// Time since the last export, while receiving.
    pub(super) fn since_last_export(&self) -> Option<Duration> {
        match &*self.phase.borrow() {
            Phase::Receiving { last_export } => Some(last_export.elapsed()),
            _ => None,
        }
    }

    /// Restarts the inactivity clock.
    pub(super) fn touch(&self) {
        self.phase.send_modify(|phase| {
            if let Phase::Receiving { last_export } = phase {
                *last_export = Instant::now();
            }
        });
    }

    /// Asks the checker to stop. A no-op once the channel is closed.
    pub(super) async fn request_stop(&self) {
        if self.is_receiving() {
            let _ = self
                .exports
                .send(OtlpRequest::Stop(StopSignal::AdminStop))
                .await;
        }
    }

    /// Publishes the report. Wakes a waiting `/stop`. Does nothing if shutdown
    /// was already requested: nobody can fetch the report then.
    pub(super) fn stopped(&self, report: Report) {
        self.phase.send_modify(|phase| {
            if !matches!(phase, Phase::ShuttingDown) {
                *phase = Phase::Stopped { report };
            }
        });
    }

    /// Ends the run, whatever phase it is in. The listeners stop and the
    /// process exits.
    pub(super) fn request_shutdown(&self) {
        let _ = self.phase.send_replace(Phase::ShuttingDown);
    }

    /// Resolves once shutdown has been requested.
    pub(super) async fn shutting_down(&self) {
        let mut phase = self.phase.subscribe();
        let _ = phase
            .wait_for(|phase| matches!(phase, Phase::ShuttingDown))
            .await;
    }
}

/// The body of `/health`, `/stop` and `/shutdown`.
#[derive(Serialize)]
struct StateResponse {
    state: &'static str,
    /// Whether `GET /report` has something to return. Only on `/stop`.
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<bool>,
}

impl StateResponse {
    fn new(state: &'static str) -> Self {
        Self {
            state,
            report: None,
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

/// `/health`, `/stop`, `/report` and `/shutdown`.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/stop", post(stop))
        .route("/report", get(report))
        .route("/shutdown", post(shutdown))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    // Scripts poll for this exact body. Keep it.
    Json(serde_json::json!({"status": "ready"}))
}

/// Stops receiving and waits until the report is ready. Idempotent once stopped.
///
/// There is no timeout. If the client disconnects, hyper drops this handler.
async fn stop(State(state): State<Arc<AppState>>) -> Response {
    info!("POST /stop: stopping the run and waiting for the report");
    state.request_stop().await;

    let mut phase = state.phase.subscribe();
    let outcome = phase
        .wait_for(|phase| !matches!(phase, Phase::Receiving { .. }))
        .await;
    let has_report = match outcome {
        Ok(phase) => matches!(&*phase, Phase::Stopped { .. }),
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the run ended without a report",
            )
        }
    };
    if has_report {
        info!("POST /stop: the run has stopped; the report is ready at GET /report");
    } else {
        info!("POST /stop: the run has stopped; the report was written, not served");
    }
    Json(StateResponse {
        state: "stopped",
        report: Some(has_report),
    })
    .into_response()
}

/// The report of a stopped run. Available for as long as the process runs.
async fn report(State(state): State<Arc<AppState>>) -> Response {
    let phase = state.phase.borrow().clone();
    match phase {
        Phase::Receiving { .. } => {
            info!("GET /report: refused, the run is still receiving");
            error(StatusCode::CONFLICT, "still receiving; POST /stop first")
        }
        Phase::ShuttingDown => {
            info!("GET /report: refused, the process is shutting down");
            error(
                StatusCode::CONFLICT,
                "shutting down; a report is only served with --output http",
            )
        }
        Phase::Stopped { report } => {
            info!(
                "GET /report: serving the report ({} bytes, {})",
                report.body.len(),
                report.content_type
            );
            ([(header::CONTENT_TYPE, report.content_type)], report.body).into_response()
        }
    }
}

/// Ends the process. Stops the run first if it is still receiving.
async fn shutdown(State(state): State<Arc<AppState>>) -> Response {
    info!("POST /shutdown: exiting");
    state.request_stop().await;
    state.request_shutdown();
    Json(StateResponse::new("shutting_down")).into_response()
}

fn error(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}
