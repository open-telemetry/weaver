// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) server for the semantic convention registry.
//!
//! This crate provides an MCP server that exposes the semantic conventions
//! registry to LLMs like Claude. It supports 7 tools:
//!
//! - `search` - Search across all registry items
//! - `get_attribute` - Get a specific attribute by key
//! - `get_metric` - Get a specific metric by name
//! - `get_span` - Get a specific span by type
//! - `get_event` - Get a specific event by name
//! - `get_entity` - Get a specific entity by type
//! - `live_check` - Validate telemetry samples against the registry
//!
//! The server uses the rmcp SDK with JSON-RPC 2.0 over stdio for communication.

mod service;

pub use service::WeaverMcpService;

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::transport::stdio;
use rmcp::ServiceExt;
use weaver_forge::v2::registry::ForgeResolvedRegistry;

/// Configuration for the MCP server.
#[derive(Debug, Default, Clone)]
pub struct McpConfig {
    /// Path to custom Rego advice policies directory.
    /// If None, default built-in policies are used.
    pub advice_policies: Option<PathBuf>,

    /// Path to a jq preprocessor script for Rego policies.
    /// The script transforms registry data before passing to Rego.
    pub advice_preprocessor: Option<PathBuf>,
}

/// Error type for MCP operations.
#[derive(Debug, serde::Serialize, miette::Diagnostic)]
#[diagnostic(code(weaver::mcp::error))]
pub struct McpError(#[help] String);

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MCP error: {}", self.0)
    }
}

impl std::error::Error for McpError {}

/// Run the MCP server with the given registry and default configuration.
///
/// This function blocks until the server is shut down (e.g., when stdin is closed).
///
/// # Arguments
///
/// * `registry` - The resolved semantic convention registry to serve.
///
/// # Errors
///
/// Returns an error if there's an IO error during communication.
pub fn run(registry: ForgeResolvedRegistry) -> Result<(), McpError> {
    run_with_config(registry, McpConfig::default())
}

/// Run the MCP server with the given registry and configuration.
///
/// This function blocks until the server is shut down (e.g., when stdin is closed).
///
/// # Arguments
///
/// * `registry` - The resolved semantic convention registry to serve.
/// * `config` - Configuration options for the server.
///
/// # Errors
///
/// Returns an error if there's an IO error during communication.
pub fn run_with_config(registry: ForgeResolvedRegistry, config: McpConfig) -> Result<(), McpError> {
    // Create a tokio runtime for the async rmcp server
    let rt = tokio::runtime::Runtime::new().map_err(|e| McpError(e.to_string()))?;

    rt.block_on(async { run_async(registry, config).await })
}

/// Run the MCP server asynchronously.
///
/// This is the async implementation that uses rmcp's stdio transport.
async fn run_async(registry: ForgeResolvedRegistry, config: McpConfig) -> Result<(), McpError> {
    let registry = Arc::new(registry);
    let service = WeaverMcpService::new(registry, config);

    let server = service
        .serve(stdio())
        .await
        .map_err(|e| McpError(e.to_string()))?;

    let _quit_reason = server
        .waiting()
        .await
        .map_err(|e| McpError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_display() {
        let error = McpError("test error message".to_owned());
        let display = format!("{error}");
        assert_eq!(display, "MCP error: test error message");
    }

    #[test]
    fn test_mcp_error_debug() {
        let error = McpError("test".to_owned());
        let debug = format!("{error:?}");
        assert!(debug.contains("McpError"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_mcp_config_default() {
        let config = McpConfig::default();
        assert!(config.advice_policies.is_none());
        assert!(config.advice_preprocessor.is_none());
    }

    #[test]
    fn test_mcp_config_with_paths() {
        let config = McpConfig {
            advice_policies: Some(PathBuf::from("/path/to/policies")),
            advice_preprocessor: Some(PathBuf::from("/path/to/preprocessor.jq")),
        };
        assert_eq!(
            config.advice_policies,
            Some(PathBuf::from("/path/to/policies"))
        );
        assert_eq!(
            config.advice_preprocessor,
            Some(PathBuf::from("/path/to/preprocessor.jq"))
        );
    }
}
