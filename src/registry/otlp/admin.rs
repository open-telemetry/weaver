// SPDX-License-Identifier: Apache-2.0

//! Live-check admin HTTP API and the run state it shares with the OTLP receiver.
//!
//! A run moves through `Receiving`, `Stopped` and `ShuttingDown`. With
//! `--output http` the report is served on `/report` until `/shutdown`, so
//! fetching the report is separate from exiting and large bodies are not
//! truncated (#1657).

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

/// Rendered report served on `/report`.
#[derive(Clone, Debug)]
pub struct Report {
    /// `Content-Type` of the body, set by the output format.
    pub content_type: String,
    /// Report body. `Bytes` so repeated responses share one buffer.
    pub body: Bytes,
}

/// Run state. Each variant holds the data that is valid in that phase.
#[derive(Clone, Debug)]
pub enum Phase {
    /// Accepting exports and passing them to the checker.
    Receiving {
        /// Time of the most recent export, used by the inactivity timeout.
        last_export: Instant,
    },
    /// Report is final and served on `/report`.
    Stopped { report: Report },
    /// Listeners are closing before the process exits.
    ShuttingDown,
}

/// State shared by the OTLP receiver and the admin handlers.
pub struct AppState {
    /// Channel to the checker for exports and the final `Stop`.
    pub(super) exports: mpsc::Sender<OtlpRequest>,
    /// Current phase. `/stop` waits for it to leave `Receiving`; the listeners
    /// wait for `ShuttingDown`.
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

    /// Time since the last export, or `None` when not receiving.
    pub(super) fn since_last_export(&self) -> Option<Duration> {
        match &*self.phase.borrow() {
            Phase::Receiving { last_export } => Some(last_export.elapsed()),
            _ => None,
        }
    }

    /// Sets the last export time to now while receiving.
    pub(super) fn touch(&self) {
        self.phase.send_modify(|phase| {
            if let Phase::Receiving { last_export } = phase {
                *last_export = Instant::now();
            }
        });
    }

    /// Sends a stop signal to the checker if still receiving.
    pub(super) async fn request_stop(&self) {
        if self.is_receiving() {
            let _ = self
                .exports
                .send(OtlpRequest::Stop(StopSignal::AdminStop))
                .await;
        }
    }

    /// Stores the report and moves to `Stopped`, waking any pending `/stop`.
    /// Ignored after shutdown is requested, since the report can no longer be fetched.
    pub(super) fn stopped(&self, report: Report) {
        self.phase.send_modify(|phase| {
            if !matches!(phase, Phase::ShuttingDown) {
                *phase = Phase::Stopped { report };
            }
        });
    }

    /// Moves to `ShuttingDown` from any phase, which stops the listeners.
    pub(super) fn request_shutdown(&self) {
        let _ = self.phase.send_replace(Phase::ShuttingDown);
    }

    /// Resolves when the phase becomes `ShuttingDown`.
    pub(super) async fn shutting_down(&self) {
        let mut phase = self.phase.subscribe();
        let _ = phase
            .wait_for(|phase| matches!(phase, Phase::ShuttingDown))
            .await;
    }
}

/// Response body for `/stop` and `/shutdown`.
#[derive(Serialize)]
struct StateResponse {
    state: &'static str,
    /// Whether `GET /report` has a report to return. Set only by `/stop`.
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

/// Builds the admin router: `/health`, `/stop`, `/report` and `/shutdown`.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/stop", post(stop))
        .route("/report", get(report))
        .route("/shutdown", post(shutdown))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    // Clients poll for this exact body; do not change it.
    Json(serde_json::json!({"status": "ready"}))
}

/// Stops receiving and waits for the report. Idempotent after the first call.
///
/// Has no timeout; hyper drops the handler if the client disconnects.
async fn stop(State(state): State<Arc<AppState>>) -> Response {
    info!("POST /stop: stopping, waiting for the report");
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
        info!("POST /stop: stopped, report available at GET /report");
    } else {
        info!("POST /stop: stopped, report written to output, not served over HTTP");
    }
    Json(StateResponse {
        state: "stopped",
        report: Some(has_report),
    })
    .into_response()
}

/// Returns the report while the run is `Stopped`, otherwise `409 Conflict`.
async fn report(State(state): State<Arc<AppState>>) -> Response {
    let phase = state.phase.borrow().clone();
    match phase {
        Phase::Receiving { .. } => {
            info!("GET /report: rejected, still receiving");
            error(StatusCode::CONFLICT, "still receiving; POST /stop first")
        }
        Phase::ShuttingDown => {
            info!("GET /report: rejected, shutting down");
            error(
                StatusCode::CONFLICT,
                "shutting down; a report is only served with --output http",
            )
        }
        Phase::Stopped { report } => {
            info!(
                "GET /report: serving {} bytes as {}",
                report.body.len(),
                report.content_type
            );
            ([(header::CONTENT_TYPE, report.content_type)], report.body).into_response()
        }
    }
}

/// Stops the run if still receiving, then shuts down the process.
async fn shutdown(State(state): State<Arc<AppState>>) -> Response {
    info!("POST /shutdown: shutting down");
    state.request_stop().await;
    state.request_shutdown();
    Json(StateResponse::new("shutting_down")).into_response()
}

fn error(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}
