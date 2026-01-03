// SPDX-License-Identifier: Apache-2.0

//! API request and response types for the serve command.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use weaver_search::{SearchResult, SearchType};
use weaver_semconv::stability::Stability;

/// Registry stats response.
#[derive(Debug, Serialize, ToSchema)]
pub struct RegistryStats {
    /// The registry URL.
    pub registry_url: String,
    /// Counts of different entity types.
    pub counts: RegistryCounts,
    // TODO: It would be better to serve the output of `weaver registry stats` here
    // then we could draw graphs in the UI.
}

/// Counts of different entity types in the registry.
#[derive(Debug, Serialize, ToSchema)]
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

/// Query parameters for search endpoint.
#[derive(Debug, Deserialize, IntoParams)]
pub struct SearchParams {
    /// Search query string (optional for browse mode).
    #[param(example = "http")]
    pub q: Option<String>,
    /// Filter by type.
    #[serde(rename = "type", default)]
    #[param(rename = "type")]
    pub search_type: SearchType,
    /// Filter by stability level.
    pub stability: Option<Stability>,
    /// Maximum number of results (default: 50).
    #[serde(default = "default_search_limit")]
    #[param(maximum = 1000)]
    pub limit: usize,
    /// Offset for pagination (default: 0).
    #[serde(default)]
    pub offset: usize,
}

fn default_search_limit() -> usize {
    50
}

/// Search response.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    /// The original query (None in browse mode).
    pub query: Option<String>,
    /// Total number of matches (for pagination).
    pub total: usize,
    /// Number of results returned.
    pub count: usize,
    /// Offset used for pagination.
    pub offset: usize,
    /// The search results.
    pub results: Vec<SearchResult>,
}

/// Query parameters for filter endpoint.
#[derive(Debug, Deserialize, IntoParams)]
pub struct FilterParams {
    /// JQ Filter string.
    #[param(example = ".")]
    pub filter: Option<String>,
}
