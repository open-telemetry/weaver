// SPDX-License-Identifier: Apache-2.0

//! Axum server setup for the weaver serve command.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use miette::Diagnostic;
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use super::handlers;
use super::search::SearchContext;
use super::ui::UI_DIST;

/// Shared application state for all request handlers.
pub struct AppState {
    /// The resolved registry loaded at startup.
    pub registry: ForgeResolvedRegistry,
    /// Pre-built search context for fast lookups.
    pub search_ctx: SearchContext,
}

/// Error type for server operations.
#[derive(Debug, thiserror::Error, Serialize, Diagnostic)]
pub enum Error {
    /// IO error
    #[error("IO error: {error}")]
    Io {
        /// The error message.
        error: String,
    },
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io {
            error: err.to_string(),
        }
    }
}

/// Run the API server.
///
/// # Arguments
///
/// * `bind_addr` - The address to bind the server to.
/// * `registry` - The resolved V2 registry to serve.
pub async fn run_server(
    bind_addr: SocketAddr,
    registry: ForgeResolvedRegistry,
) -> Result<(), Error> {
    // Build search context once at startup
    let search_ctx = SearchContext::from_registry(&registry);

    let state = Arc::new(AppState {
        registry,
        search_ctx,
    });

    // Configure CORS to allow any origin (for development)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health check
        .route("/health", get(handlers::health))
        // Schema
        .route("/api/v1/schema", get(handlers::get_schema))
        // Registry overview
        .route("/api/v1/registry", get(handlers::registry_overview))
        // Attributes
        .route("/api/v1/attributes", get(handlers::list_attributes))
        .route("/api/v1/attributes/*key", get(handlers::get_attribute))
        // Metrics
        .route("/api/v1/metrics", get(handlers::list_metrics))
        .route("/api/v1/metrics/*name", get(handlers::get_metric))
        // Spans
        .route("/api/v1/spans", get(handlers::list_spans))
        .route("/api/v1/spans/*type", get(handlers::get_span))
        // Events
        .route("/api/v1/events", get(handlers::list_events))
        .route("/api/v1/events/*name", get(handlers::get_event))
        // Entities
        .route("/api/v1/entities", get(handlers::list_entities))
        .route("/api/v1/entities/*type", get(handlers::get_entity))
        // Search
        .route("/api/v1/search", get(handlers::search))
        // UI fallback - serves embedded static files
        .fallback(serve_ui)
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve embedded UI files with SPA fallback.
async fn serve_ui(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try to find the exact file in embedded assets
    if let Some(file) = UI_DIST.get_file(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.contents(),
        )
            .into_response();
    }

    // For SPA routing: serve index.html for any non-file paths
    if let Some(index) = UI_DIST.get_file("index.html") {
        return Html(index.contents_utf8().unwrap_or_default()).into_response();
    }

    // No UI available
    (StatusCode::NOT_FOUND, "UI not available").into_response()
}
