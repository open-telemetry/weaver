// SPDX-License-Identifier: Apache-2.0

//! Get metric tool for retrieving specific metrics from the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_search::SearchContext;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific metric by name.
pub struct GetMetricTool {
    search_context: Arc<SearchContext>,
}

impl GetMetricTool {
    /// Create a new get metric tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the get metric tool.
#[derive(Debug, Deserialize)]
struct GetMetricParams {
    /// Metric name (e.g., 'http.server.request.duration').
    name: String,
}

impl Tool for GetMetricTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_metric".to_owned(),
            description: "Get detailed information about a specific semantic convention metric \
                          by its name. Returns instrument type, unit, attributes, stability, \
                          and full documentation."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Metric name (e.g., 'http.server.request.duration')"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetMetricParams = serde_json::from_value(arguments)?;

        // O(1) lookup by name
        match self.search_context.get_metric(&params.name) {
            Some(m) => {
                let result_json = serde_json::to_value(m.as_ref())?;
                Ok(ToolCallResult::text(serde_json::to_string_pretty(
                    &result_json,
                )?))
            }
            None => Err(McpError::NotFound {
                item_type: "Metric".to_owned(),
                key: params.name,
            }),
        }
    }
}
