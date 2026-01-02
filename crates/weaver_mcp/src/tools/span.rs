// SPDX-License-Identifier: Apache-2.0

//! Get span tool for retrieving specific spans from the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific span by type.
pub struct GetSpanTool {
    registry: Arc<ForgeResolvedRegistry>,
}

impl GetSpanTool {
    /// Create a new get span tool with the given registry.
    pub fn new(registry: Arc<ForgeResolvedRegistry>) -> Self {
        Self { registry }
    }
}

/// Parameters for the get span tool.
#[derive(Debug, Deserialize)]
struct GetSpanParams {
    /// Span type (e.g., 'http.client', 'db.query').
    #[serde(rename = "type")]
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
            input_schema: json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "description": "Span type (e.g., 'http.client', 'db.query')"
                    }
                },
                "required": ["type"]
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: GetSpanParams = serde_json::from_value(arguments)?;

        // Find the span by type
        let span = self
            .registry
            .signals
            .spans
            .iter()
            .find(|s| *s.r#type == params.span_type);

        match span {
            Some(s) => {
                let result_json = serde_json::to_value(s)?;
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
