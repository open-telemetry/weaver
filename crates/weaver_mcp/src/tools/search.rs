// SPDX-License-Identifier: Apache-2.0

//! Search tool for querying the semantic convention registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_search::{SearchContext, SearchType};
use weaver_semconv::stability::Stability;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Search tool for finding semantic conventions.
pub struct SearchTool {
    search_context: Arc<SearchContext>,
}

impl SearchTool {
    /// Create a new search tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the search tool.
#[derive(Debug, Deserialize)]
struct SearchParams {
    /// Search query (keywords, attribute names, etc.). Omit for browse mode.
    query: Option<String>,
    /// Filter results by type.
    #[serde(rename = "type", default)]
    search_type: SearchTypeParam,
    /// Filter by stability level.
    stability: Option<StabilityParam>,
    /// Maximum results to return.
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum SearchTypeParam {
    #[default]
    All,
    Attribute,
    Metric,
    Span,
    Event,
    Entity,
}

impl From<SearchTypeParam> for SearchType {
    fn from(param: SearchTypeParam) -> Self {
        match param {
            SearchTypeParam::All => SearchType::All,
            SearchTypeParam::Attribute => SearchType::Attribute,
            SearchTypeParam::Metric => SearchType::Metric,
            SearchTypeParam::Span => SearchType::Span,
            SearchTypeParam::Event => SearchType::Event,
            SearchTypeParam::Entity => SearchType::Entity,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StabilityParam {
    Stable,
    #[serde(alias = "experimental")]
    Development,
}

impl From<StabilityParam> for Stability {
    fn from(param: StabilityParam) -> Self {
        match param {
            StabilityParam::Stable => Stability::Stable,
            StabilityParam::Development => Stability::Development,
        }
    }
}

impl Tool for SearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search".to_owned(),
            description: "Search OpenTelemetry semantic conventions. Supports searching by \
                          keywords across attributes, metrics, spans, events, and entities. \
                          Returns matching definitions with relevance scores. Use this to find \
                          conventions when instrumenting code (e.g., 'search for HTTP server \
                          attributes').".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (keywords, attribute names, etc.). Omit for browse mode."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["all", "attribute", "metric", "span", "event", "entity"],
                        "default": "all",
                        "description": "Filter results by type"
                    },
                    "stability": {
                        "type": "string",
                        "enum": ["stable", "development"],
                        "description": "Filter by stability level (development = experimental)"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "minimum": 1,
                        "maximum": 100,
                        "description": "Maximum results to return"
                    }
                }
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: SearchParams = serde_json::from_value(arguments)?;

        let search_type: SearchType = params.search_type.into();
        let stability = params.stability.map(Stability::from);
        let limit = params.limit.min(100);

        let (results, total) = self.search_context.search(
            params.query.as_deref(),
            search_type,
            stability,
            limit,
            0, // offset
        );

        let result_json = json!({
            "results": results,
            "count": results.len(),
            "total": total,
        });

        Ok(ToolCallResult::text(serde_json::to_string_pretty(
            &result_json,
        )?))
    }
}
