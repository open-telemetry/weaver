// SPDX-License-Identifier: Apache-2.0

//! Get span tool for retrieving specific spans from the registry.

use std::sync::Arc;

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use weaver_search::SearchContext;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific span by type.
pub struct GetSpanTool {
    search_context: Arc<SearchContext>,
}

impl GetSpanTool {
    /// Create a new get span tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the get span tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GetSpanParams {
    /// Span type (e.g., 'http.client', 'db.query').
    #[serde(rename = "type")]
    #[schemars(rename = "type")]
    span_type: String,
}

impl Tool for GetSpanTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_span".to_owned(),
            description: "Get detailed information about a specific semantic convention span \
                          by its type. Returns span kind, attributes, events, stability, \
                          and full documentation."
                .to_owned(),
            input_schema: serde_json::to_value(schema_for!(GetSpanParams))
                .expect("GetSpanParams schema should serialize"),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetSpanParams = serde_json::from_value(arguments)?;

        // O(1) lookup by type
        match self.search_context.get_span(&params.span_type) {
            Some(s) => {
                let result_json = serde_json::to_value(s.as_ref())?;
                Ok(ToolCallResult::text(serde_json::to_string_pretty(
                    &result_json,
                )?))
            }
            None => Err(McpError::NotFound {
                item_type: "Span".to_owned(),
                key: params.span_type,
            }),
        }
    }
}
