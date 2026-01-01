// SPDX-License-Identifier: Apache-2.0

//! Get entity tool for retrieving specific entities from the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific entity by type.
pub struct GetEntityTool {
    registry: Arc<ForgeResolvedRegistry>,
}

impl GetEntityTool {
    /// Create a new get entity tool with the given registry.
    pub fn new(registry: Arc<ForgeResolvedRegistry>) -> Self {
        Self { registry }
    }
}

/// Parameters for the get entity tool.
#[derive(Debug, Deserialize)]
struct GetEntityParams {
    /// Entity type (e.g., 'service', 'host', 'container').
    #[serde(rename = "type")]
    entity_type: String,
}

impl Tool for GetEntityTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_entity".to_owned(),
            description: "Get detailed information about a specific semantic convention entity \
                          by its type. Returns attributes, stability, and full documentation.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "description": "Entity type (e.g., 'service', 'host', 'container')"
                    }
                },
                "required": ["type"]
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetEntityParams = serde_json::from_value(arguments)?;

        // Find the entity by type
        let entity = self
            .registry
            .signals
            .entities
            .iter()
            .find(|e| *e.r#type == params.entity_type);

        match entity {
            Some(e) => {
                let result_json = serde_json::to_value(e)?;
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
