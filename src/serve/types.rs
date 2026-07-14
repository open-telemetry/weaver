// SPDX-License-Identifier: Apache-2.0

//! API request and response types for the serve command.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use weaver_resolved_schema::v2::stats::Stats;
use weaver_search::{SearchResult, SearchType};
use weaver_semconv::stability::Stability;

/// Registry stats response.
///
/// Serves the full output of `weaver registry stats` (the same `Stats` struct
/// computed by the CLI) so the UI can render detailed breakdowns and charts.
#[derive(Debug, Serialize)]
pub struct RegistryStatsResponse {
    /// The schema URL.
    pub schema_url: String,
    /// The resolved schema version these stats were computed from.
    pub version: &'static str,
    /// The full registry and refinement statistics.
    #[serde(flatten)]
    pub stats: Stats,
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
    // The maximum must match weaver_search::MAX_SEARCH_LIMIT (enforced there).
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
