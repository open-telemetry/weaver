// SPDX-License-Identifier: Apache-2.0

//! MCP tool implementations for the semantic convention registry.
//!
//! This module provides 7 tools for querying and validating against the registry:
//! - `search` - Search across all registry items
//! - `get_attribute` - Get a specific attribute by key
//! - `get_metric` - Get a specific metric by name
//! - `get_span` - Get a specific span by type
//! - `get_event` - Get a specific event by name
//! - `get_entity` - Get a specific entity by type
//! - `live_check` - Validate telemetry samples against the registry

mod attribute;
mod entity;
mod event;
mod live_check;
mod metric;
mod search;
mod span;

pub use attribute::GetAttributeTool;
pub use entity::GetEntityTool;
pub use event::GetEventTool;
pub use live_check::LiveCheckTool;
pub use metric::GetMetricTool;
pub use search::SearchTool;
pub use span::GetSpanTool;

use serde_json::Value;

use crate::error::McpError;
use crate::protocol::{ToolCallResult, ToolDefinition};

/// Trait for MCP tools.
pub trait Tool {
    /// Get the tool definition for MCP registration.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments.
    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError>;
}
