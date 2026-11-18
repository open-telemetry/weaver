// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) server for the semantic convention registry.
//!
//! This crate provides an MCP server that exposes the semantic conventions
//! registry to LLMs. It supports 10 tools:
//!
//! - `search` - Search across all registry items
//! - `get_attribute` - Get a specific attribute by key
//! - `get_metric` - Get a specific metric by name
//! - `get_span` - Get a specific span by type
//! - `get_event` - Get a specific event by name
//! - `get_entity` - Get a specific entity by type
//! - `live_check` - Validate telemetry samples (with full/findings_only output modes)
//! - `browse_namespace` - Browse attribute namespace hierarchy
//! - `load_registry` - Replace the served registry with another one
//! - `get_registry_info` - Describe the served registry
//!
//! The server uses the rmcp SDK with JSON-RPC 2.0 over stdio for communication.

mod service;

pub use service::WeaverMcpService;

use std::path::PathBuf;

use rmcp::transport::stdio;
use rmcp::ServiceExt;
use weaver_forge::v2::registry::ForgeResolvedRegistry;

/// Configuration for the MCP server.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    /// Path to custom Rego advice policies directory.
    /// If None, default built-in policies are used.
    pub advice_policies: Option<PathBuf>,

    /// Glob pattern pointing to additional JSON/YAML files to load into OPA rego data.
    /// Files are nested in OPA data using their relative path inside the glob base directory (e.g. schemas/user.json is loaded at data.user).
    pub advice_data: Option<String>,

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

/// Loads registries for the `load_registry` tool.
pub trait RegistryLoader: Send + Sync {
    /// Loads and resolves the registry at `source`, given in the `--registry` CLI flag format.
    ///
    /// # Errors
    ///
    /// Errors if the registry cannot be fetched, loaded, or resolved.
    fn load(&self, source: &str) -> Result<ForgeResolvedRegistry, McpError>;
}

impl McpError {
    /// Creates an error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Run the MCP server over stdio.
///
/// This function blocks until the server is shut down (e.g., when stdin is closed).
///
/// # Errors
///
/// Errors if there's an IO error during communication.
pub fn run(service: WeaverMcpService) -> Result<(), McpError> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| McpError(e.to_string()))?;

    rt.block_on(async {
        let server = service
            .serve(stdio())
            .await
            .map_err(|e| McpError(e.to_string()))?;

        let _quit_reason = server
            .waiting()
            .await
            .map_err(|e| McpError(e.to_string()))?;

        Ok(())
    })
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
    fn test_mcp_error_new() {
        assert_eq!(McpError::new("boom").to_string(), "MCP error: boom");
    }

    #[test]
    fn test_mcp_config_default() {
        let config = McpConfig::default();
        assert!(config.advice_policies.is_none());
        assert!(config.advice_data.is_none());
        assert!(config.advice_preprocessor.is_none());
    }

    #[test]
    fn test_mcp_config_with_paths() {
        let config = McpConfig {
            advice_policies: Some(PathBuf::from("/path/to/policies")),
            advice_data: Some("/path/to/data".to_owned()),
            advice_preprocessor: Some(PathBuf::from("/path/to/preprocessor.jq")),
        };
        assert_eq!(
            config.advice_policies,
            Some(PathBuf::from("/path/to/policies"))
        );
        assert_eq!(config.advice_data, Some("/path/to/data".to_owned()));
        assert_eq!(
            config.advice_preprocessor,
            Some(PathBuf::from("/path/to/preprocessor.jq"))
        );
    }
}
