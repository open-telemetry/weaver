// SPDX-License-Identifier: Apache-2.0

//! Error types for the MCP server.

use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur in the MCP server.
#[derive(Error, Debug, Diagnostic, Serialize)]
pub enum McpError {
    /// JSON serialization/deserialization error.
    #[error("JSON error: {message}")]
    Json {
        /// The error message.
        message: String,
    },

    /// IO error during stdio communication.
    #[error("IO error: {message}")]
    Io {
        /// The error message.
        message: String,
    },

    /// Protocol error in MCP communication.
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    /// Tool execution error.
    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    /// Item not found in registry.
    #[error("{item_type} '{key}' not found in registry")]
    NotFound {
        /// The type of item that was not found.
        item_type: String,
        /// The key/name that was searched for.
        key: String,
    },
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError::Json {
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(err: std::io::Error) -> Self {
        McpError::Io {
            message: err.to_string(),
        }
    }
}
