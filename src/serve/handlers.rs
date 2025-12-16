// SPDX-License-Identifier: Apache-2.0

//! HTTP request handlers for the serve command.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use super::server::AppState;
use super::types::{RegistryCounts, RegistryOverview, SearchParams, SearchResponse};

/// Health check endpoint.
pub async fn health() -> StatusCode {
    StatusCode::OK
}

/// Serve JSON schemas.
pub async fn get_schema(path: Option<Path<String>>) -> impl IntoResponse {
    let name = path
        .map(|Path(s)| s.trim_start_matches('/').to_owned())
        .unwrap_or_default();

    // Map schema names to their file contents
    let schema = match name.as_str() {
        "" | "forge" => include_str!("../../schemas/forge.schema.v2.json"),
        "semconv" => include_str!("../../schemas/semconv.schema.v2.json"),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                json!({"error": format!("Schema '{}' not found. Available schemas: forge, semconv", name)}).to_string(),
            ).into_response();
        }
    };

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        schema,
    )
        .into_response()
}

/// Registry overview endpoint.
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
