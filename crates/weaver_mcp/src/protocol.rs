// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) types for JSON-RPC communication.
//!
//! The MCP protocol uses JSON-RPC 2.0 over stdio for communication between
//! the client (e.g., Claude Code) and the server.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 message (can be request or notification).
///
/// Requests have an `id` field and expect a response.
/// Notifications have no `id` field and should not receive a response.
#[derive(Debug, Deserialize)]
pub struct JsonRpcMessage {
    /// JSON-RPC version (always "2.0").
    #[allow(dead_code)]
    pub jsonrpc: String,
    /// Request ID (can be number or string). None for notifications.
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Optional parameters.
    #[serde(default)]
    pub params: Value,
}

impl JsonRpcMessage {
    /// Returns true if this is a notification (no id field).
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: &'static str,
    /// Request ID (echoed from request).
    pub id: Value,
    /// Result (mutually exclusive with error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (mutually exclusive with result).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Optional additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC 2.0 error codes
// See: https://www.jsonrpc.org/specification#error_object
#[allow(dead_code)]
pub const PARSE_ERROR: i32 = -32700;
#[allow(dead_code)]
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// MCP server information returned in initialize response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Server name.
    pub name: String,
    /// Server version.
    pub version: String,
}

/// MCP server capabilities.
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    /// Tools capability.
    pub tools: ToolsCapability,
}

/// Tools capability configuration.
#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    /// Whether the server supports listing tools that have changed.
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

/// MCP initialize result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Protocol version.
    pub protocol_version: String,
    /// Server capabilities.
    pub capabilities: ServerCapabilities,
    /// Server information.
    pub server_info: ServerInfo,
}

/// MCP tool definition.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema for input parameters.
    pub input_schema: Value,
}

/// MCP tools list result.
#[derive(Debug, Serialize)]
pub struct ToolsListResult {
    /// List of available tools.
    pub tools: Vec<ToolDefinition>,
}

/// MCP tool call parameters.
#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    /// Name of the tool to call.
    pub name: String,
    /// Arguments to pass to the tool.
    #[serde(default)]
    pub arguments: Value,
}

/// Content type for tool results.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
}

/// MCP tool call result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    /// Content returned by the tool.
    pub content: Vec<ToolContent>,
    /// Whether the tool call resulted in an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolCallResult {
    /// Create a successful text result.
    pub fn text(content: String) -> Self {
        Self {
            content: vec![ToolContent::Text { text: content }],
            is_error: None,
        }
    }

    /// Create an error result.
    pub fn error(message: String) -> Self {
        Self {
            content: vec![ToolContent::Text { text: message }],
            is_error: Some(true),
        }
    }
}
