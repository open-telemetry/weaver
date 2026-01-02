// SPDX-License-Identifier: Apache-2.0

//! Get event tool for retrieving specific events from the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific event by name.
pub struct GetEventTool {
    registry: Arc<ForgeResolvedRegistry>,
}

impl GetEventTool {
    /// Create a new get event tool with the given registry.
    pub fn new(registry: Arc<ForgeResolvedRegistry>) -> Self {
        Self { registry }
    }
}

/// Parameters for the get event tool.
#[derive(Debug, Deserialize)]
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
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Event name (e.g., 'exception', 'session.start')"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetEventParams = serde_json::from_value(arguments)?;

        // Find the event by name
        let event = self
            .registry
            .signals
            .events
            .iter()
            .find(|e| *e.name == params.name);

        match event {
            Some(e) => {
                let result_json = serde_json::to_value(e)?;
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
