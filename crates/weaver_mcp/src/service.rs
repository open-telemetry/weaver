// SPDX-License-Identifier: Apache-2.0

//! MCP service implementation using rmcp SDK.
//!
//! This module provides the `WeaverMcpService` which implements all 7 tools
//! for querying and validating against the semantic convention registry.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_live_check::advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::live_checker::LiveChecker;
use weaver_live_check::{
    DisabledStatistics, LiveCheckRunner, LiveCheckStatistics, Sample, VersionedRegistry,
};
use weaver_search::{SearchContext, SearchType};
use weaver_semconv::stability::Stability;

use crate::McpConfig;

/// MCP service for the semantic convention registry.
///
/// This service exposes 7 tools for querying and validating against the registry:
/// - `search` - Search across all registry items
/// - `get_attribute` - Get a specific attribute by key
/// - `get_metric` - Get a specific metric by name
/// - `get_span` - Get a specific span by type
/// - `get_event` - Get a specific event by name
/// - `get_entity` - Get a specific entity by type
/// - `live_check` - Validate telemetry samples against the registry
#[derive(Clone)]
pub struct WeaverMcpService {
    search_context: Arc<SearchContext>,
    /// Versioned registry for live check (LiveChecker created per call due to Rc internals)
    versioned_registry: Arc<VersionedRegistry>,
    /// Path to custom Rego advice policies directory.
    advice_policies: Option<PathBuf>,
    /// Path to jq preprocessor script for Rego policies.
    advice_preprocessor: Option<PathBuf>,
    /// Tool router for handling tool calls.
    tool_router: ToolRouter<Self>,
}

impl WeaverMcpService {
    /// Create a new MCP service with the given registry and configuration.
    #[must_use]
    pub fn new(registry: Arc<ForgeResolvedRegistry>, config: McpConfig) -> Self {
        let search_context = Arc::new(SearchContext::from_registry(&registry));

        // Create versioned registry wrapper once for live check
        let versioned_registry = Arc::new(VersionedRegistry::V2((*registry).clone()));

        Self {
            search_context,
            versioned_registry,
            advice_policies: config.advice_policies,
            advice_preprocessor: config.advice_preprocessor,
            tool_router: Self::tool_router(),
        }
    }

    /// Create a LiveChecker for a single live_check call.
    ///
    /// LiveChecker contains Rc internally and cannot be stored in the async service.
    /// We create it fresh for each call.
    fn create_live_checker(&self) -> Result<LiveChecker, String> {
        let mut live_checker =
            LiveChecker::new(Arc::clone(&self.versioned_registry), default_advisors());

        // Add RegoAdvisor for policy-based advice
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &self.advice_policies,
            &self.advice_preprocessor,
        )
        .map_err(|e| format!("Failed to initialize RegoAdvisor: {e}"))?;
        live_checker.add_advisor(Box::new(rego_advisor));

        Ok(live_checker)
    }
}

/// Create the default advisors for live check.
fn default_advisors() -> Vec<Box<dyn Advisor>> {
    vec![
        Box::new(DeprecatedAdvisor),
        Box::new(StabilityAdvisor),
        Box::new(TypeAdvisor),
        Box::new(EnumAdvisor),
    ]
}

#[tool_handler]
impl ServerHandler for WeaverMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "MCP server for OpenTelemetry semantic conventions. Use 'search' to find \
                 conventions, 'get_*' tools to get details, and 'live_check' to validate samples."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// =============================================================================
// Tool Parameter Types
// =============================================================================

/// Parameters for the search tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Search query (keywords, attribute names, etc.). Omit for browse mode.
    query: Option<String>,
    /// Filter results by type.
    #[serde(rename = "type", default)]
    #[schemars(rename = "type")]
    search_type: SearchTypeParam,
    /// Filter by stability level (development = experimental).
    stability: Option<StabilityParam>,
    /// Maximum results to return (1-100, default 20).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Filter results by type.
#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchTypeParam {
    #[default]
    All,
    Attribute,
    Metric,
    Span,
    Event,
    Entity,
}

impl From<SearchTypeParam> for SearchType {
    fn from(param: SearchTypeParam) -> Self {
        match param {
            SearchTypeParam::All => SearchType::All,
            SearchTypeParam::Attribute => SearchType::Attribute,
            SearchTypeParam::Metric => SearchType::Metric,
            SearchTypeParam::Span => SearchType::Span,
            SearchTypeParam::Event => SearchType::Event,
            SearchTypeParam::Entity => SearchType::Entity,
        }
    }
}

/// Filter by stability level.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StabilityParam {
    Stable,
    #[serde(alias = "experimental")]
    Development,
}

impl From<StabilityParam> for Stability {
    fn from(param: StabilityParam) -> Self {
        match param {
            StabilityParam::Stable => Stability::Stable,
            StabilityParam::Development => Stability::Development,
        }
    }
}

/// Parameters for the get attribute tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAttributeParams {
    /// Attribute key (e.g., 'http.request.method', 'db.system').
    key: String,
}

/// Parameters for the get metric tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMetricParams {
    /// Metric name (e.g., 'http.server.request.duration').
    name: String,
}

/// Parameters for the get span tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSpanParams {
    /// Span type (e.g., 'http.client', 'db.query').
    #[serde(rename = "type")]
    #[schemars(rename = "type")]
    span_type: String,
}

/// Parameters for the get event tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEventParams {
    /// Event name (e.g., 'exception', 'session.start').
    name: String,
}

/// Parameters for the get entity tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntityParams {
    /// Entity type (e.g., 'service', 'host', 'container').
    #[serde(rename = "type")]
    #[schemars(rename = "type")]
    entity_type: String,
}

/// Parameters for the live check tool.
///
/// Note: We use Value here because Sample is from weaver_live_check which uses
/// schemars 0.8.x, while rmcp uses schemars 1.x. We deserialize manually.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LiveCheckParams {
    /// Array of telemetry samples to check (attributes, spans, metrics, logs, or resources).
    samples: Vec<serde_json::Value>,
}

// =============================================================================
// Tool Implementations
// =============================================================================

#[tool_router]
impl WeaverMcpService {
    /// Search OpenTelemetry semantic conventions.
    #[tool(
        name = "search",
        description = "Search OpenTelemetry semantic conventions. Supports searching by keywords \
                       across attributes, metrics, spans, events, and entities. Returns matching \
                       definitions with relevance scores. Use this to find conventions when \
                       instrumenting code (e.g., 'search for HTTP server attributes')."
    )]
    fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let search_type: SearchType = params.search_type.into();
        let stability = params.stability.map(Stability::from);
        let limit = params.limit.min(100);

        let (results, total) = self.search_context.search(
            params.query.as_deref(),
            search_type,
            stability,
            limit,
            0, // offset
        );

        let result_json = json!({
            "results": results,
            "count": results.len(),
            "total": total,
        });

        serde_json::to_string_pretty(&result_json).unwrap_or_else(|e| format!("Error: {e}"))
    }

    /// Get detailed information about a specific attribute.
    #[tool(
        name = "get_attribute",
        description = "Get detailed information about a specific semantic convention attribute \
                       by its key. Returns type, examples, stability, deprecation info, and \
                       full documentation."
    )]
    fn get_attribute(&self, Parameters(params): Parameters<GetAttributeParams>) -> String {
        match self.search_context.get_attribute(&params.key) {
            Some(attr) => serde_json::to_string_pretty(attr.as_ref())
                .unwrap_or_else(|e| format!("Error: {e}")),
            None => format!("Attribute '{}' not found in registry", params.key),
        }
    }

    /// Get detailed information about a specific metric.
    #[tool(
        name = "get_metric",
        description = "Get detailed information about a specific semantic convention metric \
                       by its name. Returns instrument type, unit, attributes, stability, \
                       and full documentation."
    )]
    fn get_metric(&self, Parameters(params): Parameters<GetMetricParams>) -> String {
        match self.search_context.get_metric(&params.name) {
            Some(m) => {
                serde_json::to_string_pretty(m.as_ref()).unwrap_or_else(|e| format!("Error: {e}"))
            }
            None => format!("Metric '{}' not found in registry", params.name),
        }
    }

    /// Get detailed information about a specific span.
    #[tool(
        name = "get_span",
        description = "Get detailed information about a specific semantic convention span \
                       by its type. Returns span kind, attributes, events, stability, \
                       and full documentation."
    )]
    fn get_span(&self, Parameters(params): Parameters<GetSpanParams>) -> String {
        match self.search_context.get_span(&params.span_type) {
            Some(s) => {
                serde_json::to_string_pretty(s.as_ref()).unwrap_or_else(|e| format!("Error: {e}"))
            }
            None => format!("Span '{}' not found in registry", params.span_type),
        }
    }

    /// Get detailed information about a specific event.
    #[tool(
        name = "get_event",
        description = "Get detailed information about a specific semantic convention event \
                       by its name. Returns attributes, stability, and full documentation."
    )]
    fn get_event(&self, Parameters(params): Parameters<GetEventParams>) -> String {
        match self.search_context.get_event(&params.name) {
            Some(e) => {
                serde_json::to_string_pretty(e.as_ref()).unwrap_or_else(|e| format!("Error: {e}"))
            }
            None => format!("Event '{}' not found in registry", params.name),
        }
    }

    /// Get detailed information about a specific entity.
    #[tool(
        name = "get_entity",
        description = "Get detailed information about a specific semantic convention entity \
                       by its type. Returns attributes, stability, and full documentation."
    )]
    fn get_entity(&self, Parameters(params): Parameters<GetEntityParams>) -> String {
        match self.search_context.get_entity(&params.entity_type) {
            Some(e) => {
                serde_json::to_string_pretty(e.as_ref()).unwrap_or_else(|e| format!("Error: {e}"))
            }
            None => format!("Entity '{}' not found in registry", params.entity_type),
        }
    }

    /// Run live-check on telemetry samples.
    #[tool(
        name = "live_check",
        description = "Run live-check on telemetry samples against the semantic conventions \
                       registry. Returns the samples with live_check_result fields populated \
                       containing advice and findings."
    )]
    fn live_check(&self, Parameters(params): Parameters<LiveCheckParams>) -> String {
        // Deserialize samples from Value to Sample
        let samples_result: Result<Vec<Sample>, _> = params
            .samples
            .into_iter()
            .map(serde_json::from_value)
            .collect();

        let mut samples = match samples_result {
            Ok(s) => s,
            Err(e) => return format!("Invalid sample: {e}"),
        };

        let mut stats = LiveCheckStatistics::Disabled(DisabledStatistics);

        // Create a fresh LiveChecker for this call (contains Rc, not Send)
        let mut live_checker = match self.create_live_checker() {
            Ok(lc) => lc,
            Err(e) => return format!("Failed to create live checker: {e}"),
        };

        // Run live check on each sample (mutates samples in place)
        for sample in &mut samples {
            let sample_clone: Sample = sample.clone();
            if let Err(e) =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample_clone)
            {
                return format!("Live check failed: {e}");
            }
        }

        serde_json::to_string_pretty(&samples).unwrap_or_else(|e| format!("Error: {e}"))
    }
}
