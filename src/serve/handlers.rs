// SPDX-License-Identifier: Apache-2.0

//! HTTP request handlers for the serve command.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use schemars::schema_for;
use serde_json::json;

use super::server::AppState;
use super::types::{RegistryCounts, RegistryOverview, SearchParams, SearchResponse};

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy")
    ),
    tag = "health"
)]
pub async fn health() -> StatusCode {
    StatusCode::OK
}

/// Serve a specific schema by name.
#[utoipa::path(
    get,
    path = "/api/v1/schema/{name}",
    params(
        ("name" = String, Path, description = "Schema name (forge, semconv, or sample)")
    ),
    responses(
        (status = 200, description = "Requested schema", content_type = "application/json"),
        (status = 404, description = "Schema not found")
    ),
    tag = "schemas"
)]
pub async fn get_schema(Path(name): Path<String>) -> impl IntoResponse {
    let name = name.trim_start_matches('/');

    let schema = match name {
        "forge" => schema_for!(weaver_forge::v2::registry::ForgeResolvedRegistry),
        "semconv" => schema_for!(weaver_semconv::v2::SemConvSpecV2),
        "sample" => schema_for!(weaver_live_check::Sample),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                json!({"error": format!("Schema '{}' not found. Available schemas: forge, semconv, sample", name)}).to_string(),
            ).into_response();
        }
    };

    match serde_json::to_string_pretty(&schema) {
        Ok(schema_json) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            schema_json,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json!({"error": format!("Failed to serialize schema: {}", e)}).to_string(),
        )
            .into_response(),
    }
}

/// Registry overview endpoint.
#[utoipa::path(
    get,
    path = "/api/v1/registry",
    responses(
        (status = 200, description = "Registry overview", body = RegistryOverview)
    ),
    tag = "registry"
)]
pub async fn registry_overview(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let registry = &state.registry;

    let overview = RegistryOverview {
        registry_url: registry.registry_url.clone(),
        counts: RegistryCounts {
            attributes: registry.attributes.len(),
            metrics: registry.signals.metrics.len(),
            spans: registry.signals.spans.len(),
            events: registry.signals.events.len(),
            entities: registry.signals.entities.len(),
            attribute_groups: registry.attribute_groups.len(),
        },
    };

    Json(overview)
}

/// Get a specific attribute by key.
#[utoipa::path(
    get,
    path = "/api/v1/attribute/{key}",
    params(
        ("key" = String, Path, description = "Attribute key")
    ),
    responses(
        (status = 200, description = "Attribute details", body = weaver_forge::v2::attribute::Attribute),
        (status = 404, description = "Attribute not found")
    ),
    tag = "attributes"
)]
pub async fn get_attribute(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    // Remove leading slash if present (from wildcard match)
    let key = key.trim_start_matches('/');

    let attr = state.registry.attributes.iter().find(|a| a.key == key);

    match attr {
        Some(attr) => Json(json!(attr)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Attribute not found", "key": key})),
        )
            .into_response(),
    }
}

/// Get a specific metric by name.
#[utoipa::path(
    get,
    path = "/api/v1/metric/{name}",
    params(
        ("name" = String, Path, description = "Metric name")
    ),
    responses(
        (status = 200, description = "Metric details", body = weaver_forge::v2::metric::Metric),
        (status = 404, description = "Metric not found")
    ),
    tag = "metrics"
)]
pub async fn get_metric(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let name = name.trim_start_matches('/');

    let metric = state
        .registry
        .signals
        .metrics
        .iter()
        .find(|m| &*m.name == name);

    match metric {
        Some(metric) => Json(json!(metric)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Metric not found", "name": name})),
        )
            .into_response(),
    }
}

/// Get a specific span by type.
#[utoipa::path(
    get,
    path = "/api/v1/span/{type}",
    params(
        ("type" = String, Path, description = "Span type")
    ),
    responses(
        (status = 200, description = "Span details", body = weaver_forge::v2::span::Span),
        (status = 404, description = "Span not found")
    ),
    tag = "spans"
)]
pub async fn get_span(
    State(state): State<Arc<AppState>>,
    Path(span_type): Path<String>,
) -> impl IntoResponse {
    let span_type = span_type.trim_start_matches('/');

    let span = state
        .registry
        .signals
        .spans
        .iter()
        .find(|s| &*s.r#type == span_type);

    match span {
        Some(span) => Json(json!(span)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Span not found", "type": span_type})),
        )
            .into_response(),
    }
}

/// Get a specific event by name.
#[utoipa::path(
    get,
    path = "/api/v1/event/{name}",
    params(
        ("name" = String, Path, description = "Event name")
    ),
    responses(
        (status = 200, description = "Event details", body = weaver_forge::v2::event::Event),
        (status = 404, description = "Event not found")
    ),
    tag = "events"
)]
pub async fn get_event(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let name = name.trim_start_matches('/');

    let event = state
        .registry
        .signals
        .events
        .iter()
        .find(|e| &*e.name == name);

    match event {
        Some(event) => Json(json!(event)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Event not found", "name": name})),
        )
            .into_response(),
    }
}

/// Get a specific entity by type.
#[utoipa::path(
    get,
    path = "/api/v1/entity/{type}",
    params(
        ("type" = String, Path, description = "Entity type")
    ),
    responses(
        (status = 200, description = "Entity details", body = weaver_forge::v2::entity::Entity),
        (status = 404, description = "Entity not found")
    ),
    tag = "entities"
)]
pub async fn get_entity(
    State(state): State<Arc<AppState>>,
    Path(entity_type): Path<String>,
) -> impl IntoResponse {
    let entity_type = entity_type.trim_start_matches('/');

    let entity = state
        .registry
        .signals
        .entities
        .iter()
        .find(|e| &*e.r#type == entity_type);

    match entity {
        Some(entity) => Json(json!(entity)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Entity not found", "type": entity_type})),
        )
            .into_response(),
    }
}

/// Search across all types or list all items (browse mode).
#[utoipa::path(
    get,
    path = "/api/v1/search",
    params(
        SearchParams
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResponse)
    ),
    tag = "search"
)]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    // Convert Option<String> to Option<&str> for search
    let query = params.q.as_deref();

    let (results, total) = state.search_ctx.search(
        query,
        params.search_type,
        params.stability,
        params.limit,
        params.offset,
    );

    let response = SearchResponse {
        query: params.q,
        total,
        count: results.len(),
        offset: params.offset,
        results,
    };

    Json(response).into_response()
}
