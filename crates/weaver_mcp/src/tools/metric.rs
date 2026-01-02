// SPDX-License-Identifier: Apache-2.0

//! Get metric tool for retrieving specific metrics from the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for getting a specific metric by name.
pub struct GetMetricTool {
    registry: Arc<ForgeResolvedRegistry>,
}

impl GetMetricTool {
    /// Create a new get metric tool with the given registry.
    pub fn new(registry: Arc<ForgeResolvedRegistry>) -> Self {
        Self { registry }
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

        // Find the metric by name
        let metric = self
            .registry
            .signals
            .metrics
            .iter()
            .find(|m| *m.name == params.name);

        match metric {
            Some(m) => {
                let result_json = serde_json::to_value(m)?;
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
