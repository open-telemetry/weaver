// SPDX-License-Identifier: Apache-2.0

//! MCP server implementation using JSON-RPC over stdio.
//!
//! This server implements the Model Context Protocol (MCP) for exposing
//! semantic convention registry data to LLMs like Claude.

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;

use log::{debug, error, info};
use serde_json::Value;
use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_search::SearchContext;

use crate::error::McpError;
use crate::protocol::{
    InitializeResult, JsonRpcMessage, JsonRpcResponse, ServerCapabilities, ServerInfo,
    ToolCallParams, ToolCallResult, ToolDefinition, ToolsCapability, ToolsListResult,
    INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
};
use crate::tools::{
    GetAttributeTool, GetEntityTool, GetEventTool, GetMetricTool, SearchTool, GetSpanTool, Tool,
};

/// MCP server for the semantic convention registry.
pub struct McpServer {
    /// List of available tools.
    tools: Vec<Box<dyn Tool>>,
}

impl McpServer {
    /// Create a new MCP server with the given registry.
    #[must_use]
    pub fn new(registry: Arc<ForgeResolvedRegistry>) -> Self {
        let search_context = Arc::new(SearchContext::from_registry(&registry));

        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(SearchTool::new(Arc::clone(&search_context))),
            Box::new(GetAttributeTool::new(Arc::clone(&registry))),
            Box::new(GetMetricTool::new(Arc::clone(&registry))),
            Box::new(GetSpanTool::new(Arc::clone(&registry))),
            Box::new(GetEventTool::new(Arc::clone(&registry))),
            Box::new(GetEntityTool::new(Arc::clone(&registry))),
        ];

        Self { tools }
    }

    /// Run the MCP server, reading from stdin and writing to stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if there's an IO error during communication.
    pub fn run(&self) -> Result<(), McpError> {
        info!("Starting MCP server");

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = line?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            // Parse the JSON-RPC message (request or notification)
            let message = match serde_json::from_str::<JsonRpcMessage>(&line) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Failed to parse message: {}", e);
                    // For parse errors, we don't know the id, so we can't send a proper response
                    // Just log and continue
                    continue;
                }
            };

            // Handle notifications (no id) - don't send a response
            if message.is_notification() {
                debug!("Received notification: {}", message.method);
                self.handle_notification(&message);
                continue;
            }

            // Handle requests (have id) - send a response
            let id = message.id.clone().unwrap_or(Value::Null);
            let response = self.handle_request(id, &message.method, message.params);

            // Serialize and write the response
            let response_json = serde_json::to_string(&response)?;
            debug!("Sending: {}", response_json);
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }

        info!("MCP server shutting down");
        Ok(())
    }

    /// Handle a notification (no response expected).
    fn handle_notification(&self, message: &JsonRpcMessage) {
        match message.method.as_str() {
            "notifications/initialized" => {
                debug!("Client initialized");
            }
            "notifications/cancelled" => {
                debug!("Request cancelled");
            }
            _ => {
                debug!("Unknown notification: {}", message.method);
            }
        }
    }

    /// Handle a single JSON-RPC request.
    fn handle_request(&self, id: Value, method: &str, params: Value) -> JsonRpcResponse {
        debug!("Handling method: {}", method);

        match method {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, params),
            "ping" => JsonRpcResponse::success(id, serde_json::json!({})),
            _ => {
                error!("Unknown method: {}", method);
                JsonRpcResponse::error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Method not found: {}", method),
                )
            }
        }
    }

    /// Handle the initialize request.
    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        info!("Handling initialize request");

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_owned(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "weaver-mcp".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
        };

        JsonRpcResponse::success(
            id,
            serde_json::to_value(result).expect("InitializeResult should serialize"),
        )
    }

    /// Handle the tools/list request.
    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        debug!("Handling tools/list request");

        let tool_definitions: Vec<ToolDefinition> =
            self.tools.iter().map(|t| t.definition()).collect();

        let result = ToolsListResult {
            tools: tool_definitions,
        };

        JsonRpcResponse::success(
            id,
            serde_json::to_value(result).expect("ToolsListResult should serialize"),
        )
    }

    /// Handle the tools/call request.
    fn handle_tools_call(&self, id: Value, params: Value) -> JsonRpcResponse {
        debug!("Handling tools/call request");

        // Parse the tool call parameters
        let call_params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    INVALID_PARAMS,
                    format!("Invalid tool call params: {}", e),
                );
            }
        };

        debug!("Calling tool: {}", call_params.name);

        // Find the tool
        let tool = self.tools.iter().find(|t| t.definition().name == call_params.name);

        match tool {
            Some(t) => {
                // Execute the tool
                match t.execute(call_params.arguments) {
                    Ok(result) => JsonRpcResponse::success(
                        id,
                        serde_json::to_value(result).expect("ToolCallResult should serialize"),
                    ),
                    Err(e) => {
                        let error_result = ToolCallResult::error(e.to_string());
                        JsonRpcResponse::success(
                            id,
                            serde_json::to_value(error_result)
                                .expect("ToolCallResult should serialize"),
                        )
                    }
                }
            }
            None => JsonRpcResponse::error(
                id,
                INTERNAL_ERROR,
                format!("Tool not found: {}", call_params.name),
            ),
        }
    }
}
