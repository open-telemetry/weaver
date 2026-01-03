// SPDX-License-Identifier: Apache-2.0

//! Get attribute tool for retrieving specific attributes from the registry.

use std::sync::Arc;

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use weaver_search::SearchContext;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific attribute by key.
pub struct GetAttributeTool {
    search_context: Arc<SearchContext>,
}

impl GetAttributeTool {
    /// Create a new get attribute tool with the given search context.
    pub fn new(search_context: Arc<SearchContext>) -> Self {
        Self { search_context }
    }
}

/// Parameters for the get attribute tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GetAttributeParams {
    /// Attribute key (e.g., 'http.request.method', 'db.system').
    key: String,
}

impl Tool for GetAttributeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_attribute".to_owned(),
            description: "Get detailed information about a specific semantic convention attribute \
                          by its key. Returns type, examples, stability, deprecation info, and \
                          full documentation."
                .to_owned(),
            input_schema: serde_json::to_value(schema_for!(GetAttributeParams))
                .expect("GetAttributeParams schema should serialize"),
        }
    }

    fn execute(&mut self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetAttributeParams = serde_json::from_value(arguments)?;

        // O(1) lookup by key
        match self.search_context.get_attribute(&params.key) {
            Some(attr) => {
                let result_json = serde_json::to_value(attr.as_ref())?;
                Ok(ToolCallResult::text(serde_json::to_string_pretty(
                    &result_json,
                )?))
            }
            None => Err(McpError::NotFound {
                item_type: "Attribute".to_owned(),
                key: params.key,
            }),
        }
    }
}
