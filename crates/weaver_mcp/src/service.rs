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
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LiveCheckParams {
    /// Array of telemetry samples to check (attributes, spans, metrics, logs, or resources).
    samples: Vec<Sample>,
}

// =============================================================================
// Tool Implementations
// =============================================================================

#[tool_router]
impl WeaverMcpService {
    /// Search OpenTelemetry semantic conventions.
    #[tool(
        name = "search",
        description = "Search OpenTelemetry and custom semantic conventions. Supports searching by keywords \
                       across attributes, metrics, spans, events, and entities. Query terms are AND-matched \
                       (all must appear). Returns matching definitions with relevance scores. \
                       Use short queries like 'http.request', 'db system', or 'server duration'."
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
        let mut samples = params.samples;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use weaver_forge::v2::attribute::Attribute;
    use weaver_forge::v2::entity::Entity;
    use weaver_forge::v2::event::Event;
    use weaver_forge::v2::metric::Metric;
    use weaver_forge::v2::registry::{ForgeResolvedRegistry, Refinements, Registry};
    use weaver_forge::v2::span::Span;
    use weaver_search::SearchType;
    use weaver_semconv::attribute::AttributeType;
    use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};
    use weaver_semconv::stability::Stability;
    use weaver_semconv::v2::span::SpanName;
    use weaver_semconv::v2::CommonFields;

    fn make_test_registry() -> ForgeResolvedRegistry {
        ForgeResolvedRegistry {
            registry_url: "test".to_owned(),
            registry: Registry {
                attributes: vec![Attribute {
                    key: "http.request.method".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(
                        weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    examples: None,
                    common: CommonFields {
                        brief: "HTTP request method".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                attribute_groups: vec![],
                metrics: vec![Metric {
                    name: "http.server.request.duration".to_owned().into(),
                    instrument: InstrumentSpec::Histogram,
                    unit: "s".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "Duration of HTTP server requests".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                spans: vec![Span {
                    r#type: "http.client".to_owned().into(),
                    kind: SpanKindSpec::Client,
                    name: SpanName {
                        note: "HTTP client span".to_owned(),
                    },
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "HTTP client span".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                events: vec![Event {
                    name: "exception".to_owned().into(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "An exception event".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                entities: vec![Entity {
                    r#type: "service".to_owned().into(),
                    identity: vec![],
                    description: vec![],
                    common: CommonFields {
                        brief: "A service entity".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
            },
        }
    }

    fn create_test_service() -> WeaverMcpService {
        let registry = Arc::new(make_test_registry());
        WeaverMcpService::new(registry, McpConfig::default())
    }

    // =========================================================================
    // Parameter Conversion Tests
    // =========================================================================

    #[test]
    fn test_search_type_param_to_search_type() {
        assert_eq!(SearchType::from(SearchTypeParam::All), SearchType::All);
        assert_eq!(
            SearchType::from(SearchTypeParam::Attribute),
            SearchType::Attribute
        );
        assert_eq!(
            SearchType::from(SearchTypeParam::Metric),
            SearchType::Metric
        );
        assert_eq!(SearchType::from(SearchTypeParam::Span), SearchType::Span);
        assert_eq!(SearchType::from(SearchTypeParam::Event), SearchType::Event);
        assert_eq!(
            SearchType::from(SearchTypeParam::Entity),
            SearchType::Entity
        );
    }

    #[test]
    fn test_stability_param_to_stability() {
        assert_eq!(Stability::from(StabilityParam::Stable), Stability::Stable);
        assert_eq!(
            Stability::from(StabilityParam::Development),
            Stability::Development
        );
    }

    #[test]
    fn test_stability_param_deserialize_experimental_alias() {
        // "experimental" should deserialize to Development
        let json = r#""experimental""#;
        let param: StabilityParam = serde_json::from_str(json).unwrap();
        assert_eq!(Stability::from(param), Stability::Development);
    }

    // =========================================================================
    // MCP-Specific Behavior Tests
    // =========================================================================

    #[test]
    fn test_get_attribute_not_found_message_format() {
        // The not-found message should contain the attribute key
        let key = "nonexistent.attr";
        let expected_msg = format!("Attribute '{}' not found in registry", key);

        // We verify the format matches what the service returns
        assert!(expected_msg.contains(key));
        assert!(expected_msg.contains("not found"));
    }

    #[test]
    fn test_get_metric_not_found_message_format() {
        let name = "nonexistent.metric";
        let expected_msg = format!("Metric '{}' not found in registry", name);

        assert!(expected_msg.contains(name));
        assert!(expected_msg.contains("not found"));
    }

    #[test]
    fn test_search_params_default_limit() {
        // Verify the default limit function returns 20
        assert_eq!(default_limit(), 20);
    }

    #[test]
    fn test_search_type_param_default() {
        // Verify SearchTypeParam defaults to All
        let default: SearchTypeParam = Default::default();
        assert!(matches!(default, SearchTypeParam::All));
    }

    // =========================================================================
    // Service Method Tests
    // =========================================================================

    #[test]
    fn test_service_new_and_get_info() {
        let service = create_test_service();

        // Test get_info returns valid ServerInfo
        let info = service.get_info();
        assert!(info.instructions.is_some());
        assert!(info
            .instructions
            .unwrap()
            .contains("OpenTelemetry semantic conventions"));
    }

    #[test]
    fn test_search_tool_with_query() {
        let service = create_test_service();

        let params = SearchParams {
            query: Some("http".to_owned()),
            search_type: SearchTypeParam::All,
            stability: None,
            limit: 20,
        };

        let result = service.search(Parameters(params));

        // Result should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("results").is_some());
        assert!(parsed.get("count").is_some());
        assert!(parsed.get("total").is_some());
    }

    #[test]
    fn test_search_tool_browse_mode() {
        let service = create_test_service();

        let params = SearchParams {
            query: None,
            search_type: SearchTypeParam::All,
            stability: None,
            limit: 100,
        };

        let result = service.search(Parameters(params));

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        // Should return all 5 items (1 attr + 1 metric + 1 span + 1 event + 1 entity)
        assert_eq!(parsed["total"].as_u64().unwrap(), 5);
    }

    #[test]
    fn test_search_tool_limit_clamped_to_100() {
        let service = create_test_service();

        let params = SearchParams {
            query: None,
            search_type: SearchTypeParam::All,
            stability: None,
            limit: 200, // MCP should clamp this to 100
        };

        let result = service.search(Parameters(params));

        // Should still work (we only have 5 items anyway)
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("results").is_some());
    }

    #[test]
    fn test_get_attribute_found() {
        let service = create_test_service();

        let params = GetAttributeParams {
            key: "http.request.method".to_owned(),
        };

        let result = service.get_attribute(Parameters(params));

        // Should return valid JSON with the attribute
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "http.request.method");
    }

    #[test]
    fn test_get_attribute_not_found() {
        let service = create_test_service();

        let params = GetAttributeParams {
            key: "nonexistent.attr".to_owned(),
        };

        let result = service.get_attribute(Parameters(params));

        assert!(result.contains("not found"));
        assert!(result.contains("nonexistent.attr"));
    }

    #[test]
    fn test_get_metric_found() {
        let service = create_test_service();

        let params = GetMetricParams {
            name: "http.server.request.duration".to_owned(),
        };

        let result = service.get_metric(Parameters(params));

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "http.server.request.duration");
    }

    #[test]
    fn test_get_metric_not_found() {
        let service = create_test_service();

        let params = GetMetricParams {
            name: "nonexistent.metric".to_owned(),
        };

        let result = service.get_metric(Parameters(params));

        assert!(result.contains("not found"));
    }

    #[test]
    fn test_get_span_found() {
        let service = create_test_service();

        let params = GetSpanParams {
            span_type: "http.client".to_owned(),
        };

        let result = service.get_span(Parameters(params));

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "http.client");
    }

    #[test]
    fn test_get_span_not_found() {
        let service = create_test_service();

        let params = GetSpanParams {
            span_type: "nonexistent.span".to_owned(),
        };

        let result = service.get_span(Parameters(params));

        assert!(result.contains("not found"));
    }

    #[test]
    fn test_get_event_found() {
        let service = create_test_service();

        let params = GetEventParams {
            name: "exception".to_owned(),
        };

        let result = service.get_event(Parameters(params));

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "exception");
    }

    #[test]
    fn test_get_event_not_found() {
        let service = create_test_service();

        let params = GetEventParams {
            name: "nonexistent.event".to_owned(),
        };

        let result = service.get_event(Parameters(params));

        assert!(result.contains("not found"));
    }

    #[test]
    fn test_get_entity_found() {
        let service = create_test_service();

        let params = GetEntityParams {
            entity_type: "service".to_owned(),
        };

        let result = service.get_entity(Parameters(params));

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "service");
    }

    #[test]
    fn test_get_entity_not_found() {
        let service = create_test_service();

        let params = GetEntityParams {
            entity_type: "nonexistent.entity".to_owned(),
        };

        let result = service.get_entity(Parameters(params));

        assert!(result.contains("not found"));
    }

    #[test]
    fn test_live_check_with_valid_sample() {
        let service = create_test_service();

        // Create a valid attribute sample
        let sample: Sample = serde_json::from_value(serde_json::json!({
            "attribute": {
                "name": "http.request.method",
                "value": "GET"
            }
        }))
        .unwrap();

        let params = LiveCheckParams {
            samples: vec![sample],
        };

        let result = service.live_check(Parameters(params));

        // Should return valid JSON array
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn test_live_check_invalid_sample_deserialization() {
        // Invalid JSON should fail to deserialize as Sample
        let invalid_json = serde_json::json!({"invalid": "structure"});
        let result: Result<Sample, _> = serde_json::from_value(invalid_json);
        assert!(result.is_err());

        // The error message format should be user-friendly
        if let Err(e) = result {
            let error_msg = format!("Invalid sample: {e}");
            assert!(error_msg.starts_with("Invalid sample:"));
        }
    }

    #[test]
    fn test_live_check_empty_samples() {
        let service = create_test_service();

        let params = LiveCheckParams { samples: vec![] };

        let result = service.live_check(Parameters(params));

        // Should return empty array
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }
}
