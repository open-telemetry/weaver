// SPDX-License-Identifier: Apache-2.0

//! Get event tool for retrieving specific events from the registry.

use std::sync::Arc;

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use weaver_search::SearchContext;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific event by name.
pub struct GetEventTool {
    search_context: Arc<SearchContext>,
}

impl GetEventTool {
    /// Create a new get event tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the get event tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GetEventParams {
    /// Event name (e.g., 'exception', 'session.start').
    name: String,
}

impl Tool for GetEventTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_event".to_owned(),
            description: "Get detailed information about a specific semantic convention event \
                          by its name. Returns attributes, stability, and full documentation."
                .to_owned(),
            input_schema: serde_json::to_value(schema_for!(GetEventParams))
                .expect("GetEventParams schema should serialize"),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetEventParams = serde_json::from_value(arguments)?;

        // O(1) lookup by name
        match self.search_context.get_event(&params.name) {
            Some(e) => {
                let result_json = serde_json::to_value(e.as_ref())?;
                Ok(ToolCallResult::text(serde_json::to_string_pretty(
                    &result_json,
                )?))
            }
            None => Err(McpError::NotFound {
                item_type: "Event".to_owned(),
                key: params.name,
            }),
        }
    }
}
