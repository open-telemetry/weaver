// SPDX-License-Identifier: Apache-2.0

//! Axum server setup for the weaver serve command.

#![allow(clippy::needless_for_each)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use miette::Diagnostic;
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use weaver_forge::v2::{
    attribute::Attribute, entity::Entity, event::Event, metric::Metric,
    registry::ForgeResolvedRegistry, span::Span,
};
use weaver_semconv::stability::Stability;

use super::handlers;
use super::search::SearchContext;
use super::types::{
    RegistryCounts, RegistryOverview, ScoredResult, SearchResponse, SearchResult, SearchType,
};
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

/// OpenAPI documentation for the Weaver API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Weaver API",
        version = "0.0.1",
        description = "REST API for OpenTelemetry Weaver registry, search, schemas and more.",
        contact(
            name = "OpenTelemetry",
            url = "https://github.com/open-telemetry/weaver"
        ),
        license(
            name = "Apache-2.0",
            url = "https://www.apache.org/licenses/LICENSE-2.0"
        )
    ),
    paths(
        handlers::health,
        handlers::get_schema,
        handlers::registry_overview,
        handlers::get_attribute,
        handlers::get_metric,
        handlers::get_span,
        handlers::get_event,
        handlers::get_entity,
        handlers::search,
    ),
    components(
        schemas(
            RegistryOverview,
            RegistryCounts,
            SearchType,
            SearchResponse,
            SearchResult,
            ScoredResult<Attribute>,
            ScoredResult<Metric>,
            ScoredResult<Span>,
            ScoredResult<Event>,
            ScoredResult<Entity>,
            Attribute,
            Metric,
            Span,
            Event,
            Entity,
            Stability,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "schemas", description = "JSON schema endpoints"),
        (name = "registry", description = "Registry overview"),
        (name = "attributes", description = "Attribute lookup"),
        (name = "metrics", description = "Metric lookup"),
        (name = "spans", description = "Span lookup"),
        (name = "events", description = "Event lookup"),
        (name = "entities", description = "Entity lookup"),
        (name = "search", description = "Search and browse"),
    )
)]
pub struct ApiDoc;

/// Handler for serving the OpenAPI specification.
async fn openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Run the API server.
///
/// # Arguments
///
/// * `bind_addr` - The address to bind the server to.
/// * `registry` - The resolved V2 registry to serve.
/// * `cors_origins` - Optional CORS origins. Use "*" for any origin, comma-separated for specific origins, or None for no CORS.
pub async fn run_server(
    bind_addr: SocketAddr,
    registry: ForgeResolvedRegistry,
    cors_origins: Option<&str>,
) -> Result<(), Error> {
    // Build search context once at startup
    let search_ctx = SearchContext::from_registry(&registry);

    let state = Arc::new(AppState {
        registry,
        search_ctx,
    });

    let mut app = Router::new()
        // Health check
        .route("/health", get(handlers::health))
        // Schemas
        .route("/api/v1/schema/*name", get(handlers::get_schema))
        // Registry overview
        .route("/api/v1/registry", get(handlers::registry_overview))
        // Individual resources
        .route("/api/v1/attribute/*key", get(handlers::get_attribute))
        .route("/api/v1/metric/*name", get(handlers::get_metric))
        .route("/api/v1/span/*type", get(handlers::get_span))
        .route("/api/v1/event/*name", get(handlers::get_event))
        .route("/api/v1/entity/*type", get(handlers::get_entity))
        // Search
        .route("/api/v1/search", get(handlers::search))
        // OpenAPI specification
        .route("/api/v1/openapi.json", get(openapi_spec))
        // UI fallback - serves embedded static files
        .fallback(serve_ui)
        .with_state(state);

    // Configure CORS if origins are specified
    if let Some(origins) = cors_origins {
        use tower_http::cors::AllowOrigin;

        let cors = if origins == "*" {
            // Allow any origin
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            // Parse comma-separated origins
            let origin_list: Vec<_> = origins
                .split(',')
                .map(|s| s.trim().parse().expect("Invalid origin URL"))
                .collect();

            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origin_list))
                .allow_methods(Any)
                .allow_headers(Any)
        };

        app = app.layer(cors);
    }

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
