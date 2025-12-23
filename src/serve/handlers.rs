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
use super::types::{RegistryCounts, RegistryStats, SearchParams, SearchResponse};

/// Health check.
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

/// Get a schema by name.
#[utoipa::path(
    get,
    path = "/api/v1/schema/{name}",
    params(
        ("name" = String, Path, description = "Schema name (ForgeRegistryV2, SemconvDefinitionV2, or LiveCheckSample)")
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
        "ForgeRegistryV2" => schema_for!(weaver_forge::v2::registry::ForgeResolvedRegistry),
        "SemconvDefinitionV2" => schema_for!(weaver_semconv::v2::SemConvSpecV2),
        "LiveCheckSample" => schema_for!(weaver_live_check::Sample),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                json!({"error": format!("Schema '{}' not found. Available schemas: ForgeRegistryV2, SemconvDefinitionV2, LiveCheckSample", name)}).to_string(),
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

/// Get statistics for the registry.
#[utoipa::path(
    get,
    path = "/api/v1/registry/stats",
    responses(
        (status = 200, description = "Registry stats", body = RegistryStats)
    ),
    tag = "registry"
)]
pub async fn get_registry_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let registry = &state.registry;

    let stats = RegistryStats {
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

    Json(stats)
}

/// Get an attribute by key.
#[utoipa::path(
    get,
    path = "/api/v1/registry/attribute/{key}",
    params(
        ("key" = String, Path, description = "Attribute key")
    ),
    responses(
        (status = 200, description = "Attribute details", body = weaver_forge::v2::attribute::Attribute),
        (status = 404, description = "Attribute not found")
    ),
    tag = "registry"
)]
pub async fn get_registry_attribute(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    // Remove leading slash if present (from wildcard match)
    let key = key.trim_start_matches('/');

    let attr = state.registry.attributes.iter().find(|a| a.key == key);

    match attr {
        Some(attr) => Json(attr).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Attribute not found", "key": key})),
        )
            .into_response(),
    }
}

/// Get a metric by name.
#[utoipa::path(
    get,
    path = "/api/v1/registry/metric/{name}",
    params(
        ("name" = String, Path, description = "Metric name")
    ),
    responses(
        (status = 200, description = "Metric details", body = weaver_forge::v2::metric::Metric),
        (status = 404, description = "Metric not found")
    ),
    tag = "registry"
)]
pub async fn get_registry_metric(
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
        Some(metric) => Json(metric).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Metric not found", "name": name})),
        )
            .into_response(),
    }
}

/// Get a span by type.
#[utoipa::path(
    get,
    path = "/api/v1/registry/span/{type}",
    params(
        ("type" = String, Path, description = "Span type")
    ),
    responses(
        (status = 200, description = "Span details", body = weaver_forge::v2::span::Span),
        (status = 404, description = "Span not found")
    ),
    tag = "registry"
)]
pub async fn get_registry_span(
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
        Some(span) => Json(span).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Span not found", "type": span_type})),
        )
            .into_response(),
    }
}

/// Get an event by name.
#[utoipa::path(
    get,
    path = "/api/v1/registry/event/{name}",
    params(
        ("name" = String, Path, description = "Event name")
    ),
    responses(
        (status = 200, description = "Event details", body = weaver_forge::v2::event::Event),
        (status = 404, description = "Event not found")
    ),
    tag = "registry"
)]
pub async fn get_registry_event(
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
        Some(event) => Json(event).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Event not found", "name": name})),
        )
            .into_response(),
    }
}

/// Get an entity by type.
#[utoipa::path(
    get,
    path = "/api/v1/registry/entity/{type}",
    params(
        ("type" = String, Path, description = "Entity type")
    ),
    responses(
        (status = 200, description = "Entity details", body = weaver_forge::v2::entity::Entity),
        (status = 404, description = "Entity not found")
    ),
    tag = "registry"
)]
pub async fn get_registry_entity(
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
        Some(entity) => Json(entity).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Entity not found", "type": entity_type})),
        )
            .into_response(),
    }
}

/// Search registry across all attributes and signals.
#[utoipa::path(
    get,
    path = "/api/v1/registry/search",
    params(
        SearchParams
    ),
    responses(
        (status = 200, description = "Registry search results", body = SearchResponse)
    ),
    tag = "registry"
)]
pub async fn search_registry(
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
