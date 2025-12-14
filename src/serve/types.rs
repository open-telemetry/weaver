// SPDX-License-Identifier: Apache-2.0

//! API request and response types for the serve command.

use serde::{Deserialize, Serialize};
use weaver_forge::v2::attribute::Attribute;
use weaver_forge::v2::entity::Entity;
use weaver_forge::v2::event::Event;
use weaver_forge::v2::metric::Metric;
use weaver_forge::v2::span::Span;
use weaver_semconv::stability::Stability;

/// Generic wrapper that adds a relevance score to any searchable object.
#[derive(Debug, Serialize)]
pub struct ScoredResult<T> {
    /// The relevance score (higher = more relevant).
    pub score: u32,
    /// The full object (Attribute, Metric, Span, Event, or Entity).
    #[serde(flatten)]
    pub item: T,
}

/// Registry overview response.
#[derive(Debug, Serialize)]
pub struct RegistryOverview {
    /// The registry URL.
    pub registry_url: String,
    /// Counts of different entity types.
    pub counts: RegistryCounts,
}

/// Counts of different entity types in the registry.
#[derive(Debug, Serialize)]
pub struct RegistryCounts {
    /// Number of attributes.
    pub attributes: usize,
    /// Number of metrics.
    pub metrics: usize,
    /// Number of spans.
    pub spans: usize,
    /// Number of events.
    pub events: usize,
    /// Number of entities.
    pub entities: usize,
    /// Number of attribute groups.
    pub attribute_groups: usize,
}

/// Query parameters for list endpoints.
#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// Filter by stability level.
    pub stability: Option<StabilityFilter>,
    /// Maximum number of results (default: 100).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination (default: 0).
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

/// Stability filter options.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum StabilityFilter {
    /// Only stable items.
    Stable,
    /// Only development/experimental items.
    #[serde(alias = "experimental")]
    Development,
    /// Only alpha items.
    Alpha,
    /// Only beta items.
    Beta,
}

impl StabilityFilter {
    /// Check if a stability level matches this filter.
    #[allow(deprecated)]
    pub fn matches(&self, stability: &Stability) -> bool {
        matches!(
            (self, stability),
            (StabilityFilter::Stable, Stability::Stable)
                | (StabilityFilter::Development, Stability::Development)
                | (StabilityFilter::Alpha, Stability::Alpha)
                | (StabilityFilter::Beta, Stability::Beta)
        )
    }
}

/// Query parameters for search endpoint.
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Search query string.
    pub q: String,
    /// Filter by type.
    #[serde(rename = "type", default)]
    pub search_type: SearchType,
    /// Maximum number of results (default: 50).
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    50
}

/// Search type filter.
#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
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

/// Search response.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// The original query.
    pub query: String,
    /// Total number of matches.
    pub total: usize,
    /// Number of results returned.
    pub count: usize,
    /// The search results.
    pub results: Vec<SearchResult>,
}

/// A single search result containing a full object with its relevance score.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
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

/// Paginated list response.
#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    /// Total number of items.
    pub total: usize,
    /// Number of items returned.
    pub count: usize,
    /// Offset used.
    pub offset: usize,
    /// The items.
    pub items: Vec<T>,
}
