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
use super::types::{
    ListParams, ListResponse, RegistryCounts, RegistryOverview, SearchParams,
    SearchResponse,
};

/// Health check endpoint.
pub async fn health() -> StatusCode {
    StatusCode::OK
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

/// List all attributes.
pub async fn list_attributes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let registry = &state.registry;

    let filtered: Vec<_> = registry
        .attributes
        .iter()
        .filter(|attr| {
            params
                .stability
                .map(|f| f.matches(&attr.common.stability))
                .unwrap_or(true)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<_> = filtered
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .cloned()
        .collect();

    Json(ListResponse {
        total,
        count: items.len(),
        offset: params.offset,
        items,
    })
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

/// List all metrics.
pub async fn list_metrics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let registry = &state.registry;

    let filtered: Vec<_> = registry
        .signals
        .metrics
        .iter()
        .filter(|m| {
            params
                .stability
                .map(|f| f.matches(&m.common.stability))
                .unwrap_or(true)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<_> = filtered
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .cloned()
        .collect();

    Json(ListResponse {
        total,
        count: items.len(),
        offset: params.offset,
        items,
    })
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

/// List all spans.
pub async fn list_spans(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let registry = &state.registry;

    let filtered: Vec<_> = registry
        .signals
        .spans
        .iter()
        .filter(|s| {
            params
                .stability
                .map(|f| f.matches(&s.common.stability))
                .unwrap_or(true)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<_> = filtered
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .cloned()
        .collect();

    Json(ListResponse {
        total,
        count: items.len(),
        offset: params.offset,
        items,
    })
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

/// List all events.
pub async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let registry = &state.registry;

    let filtered: Vec<_> = registry
        .signals
        .events
        .iter()
        .filter(|e| {
            params
                .stability
                .map(|f| f.matches(&e.common.stability))
                .unwrap_or(true)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<_> = filtered
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .cloned()
        .collect();

    Json(ListResponse {
        total,
        count: items.len(),
        offset: params.offset,
        items,
    })
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

/// List all entities.
pub async fn list_entities(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let registry = &state.registry;

    let filtered: Vec<_> = registry
        .signals
        .entities
        .iter()
        .filter(|e| {
            params
                .stability
                .map(|f| f.matches(&e.common.stability))
                .unwrap_or(true)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<_> = filtered
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .cloned()
        .collect();

    Json(ListResponse {
        total,
        count: items.len(),
        offset: params.offset,
        items,
    })
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

/// Search across all types.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    if params.q.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Query parameter 'q' is required"})),
        )
            .into_response();
    }

    let results = state
        .search_ctx
        .search(&params.q, params.search_type, params.limit);

    let response = SearchResponse {
        query: params.q,
        total: results.len(),
        count: results.len(),
        results,
    };

    Json(response).into_response()
}
