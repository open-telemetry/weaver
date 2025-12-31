// SPDX-License-Identifier: Apache-2.0

//! Core search types for the weaver search engine.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use weaver_forge::v2::attribute::Attribute;
use weaver_forge::v2::entity::Entity;
use weaver_forge::v2::event::Event;
use weaver_forge::v2::metric::Metric;
use weaver_forge::v2::span::Span;

/// Generic wrapper that adds a relevance score to any searchable object.
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct ScoredResult<T> {
    /// The relevance score (higher = more relevant).
    pub score: u32,
    /// The full object (Attribute, Metric, Span, Event, or Entity).
    #[serde(flatten)]
    #[schema(value_type = T)]
    pub item: Arc<T>,
}

/// Search type filter.
#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    /// Search all types.
    #[default]
    All,
    /// Search only attributes.
    Attribute,
    /// Search only metrics.
    Metric,
    /// Search only spans.
    Span,
    /// Search only events.
    Event,
    /// Search only entities.
    Entity,
}

/// A single search result containing a full object with its relevance score.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "result_type", rename_all = "lowercase")]
pub enum SearchResult {
    /// An attribute result.
    Attribute(ScoredResult<Attribute>),
    /// A metric result.
    Metric(ScoredResult<Metric>),
    /// A span result.
    Span(ScoredResult<Span>),
    /// An event result.
    Event(ScoredResult<Event>),
    /// An entity result.
    Entity(ScoredResult<Entity>),
}
