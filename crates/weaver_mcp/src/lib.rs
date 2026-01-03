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
//! The server uses JSON-RPC 2.0 over stdio for communication.

mod error;
mod protocol;
mod server;
mod tools;

pub use error::McpError;
pub use server::McpServer;

use std::path::PathBuf;
use std::sync::Arc;

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
    let registry = Arc::new(registry);
    let mut server = McpServer::new(registry, config)?;
    server.run()
}
