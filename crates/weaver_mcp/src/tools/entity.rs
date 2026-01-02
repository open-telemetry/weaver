// SPDX-License-Identifier: Apache-2.0

//! Get entity tool for retrieving specific entities from the registry.

use std::sync::Arc;

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use weaver_search::SearchContext;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific entity by type.
pub struct GetEntityTool {
    search_context: Arc<SearchContext>,
}

impl GetEntityTool {
    /// Create a new get entity tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the get entity tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GetEntityParams {
    /// Entity type (e.g., 'service', 'host', 'container').
    #[serde(rename = "type")]
    #[schemars(rename = "type")]
    entity_type: String,
}

impl Tool for GetEntityTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_entity".to_owned(),
            description: "Get detailed information about a specific semantic convention entity \
                          by its type. Returns attributes, stability, and full documentation."
                .to_owned(),
            input_schema: serde_json::to_value(schema_for!(GetEntityParams))
                .expect("GetEntityParams schema should serialize"),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetEntityParams = serde_json::from_value(arguments)?;

        // O(1) lookup by type
        match self.search_context.get_entity(&params.entity_type) {
            Some(e) => {
                let result_json = serde_json::to_value(e.as_ref())?;
                Ok(ToolCallResult::text(serde_json::to_string_pretty(
                    &result_json,
                )?))
            }
            None => Err(McpError::NotFound {
                item_type: "Entity".to_owned(),
                key: params.entity_type,
            }),
        }
    }
}
