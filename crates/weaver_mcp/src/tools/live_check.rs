// SPDX-License-Identifier: Apache-2.0

//! Live check tool for validating telemetry samples against the registry.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use weaver_live_check::advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::live_checker::LiveChecker;
use weaver_live_check::{
    DisabledStatistics, LiveCheckRunner, LiveCheckStatistics, Sample, VersionedRegistry,
};

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for running live-check on telemetry samples.
pub struct LiveCheckTool {
    versioned_registry: Arc<VersionedRegistry>,
}

impl LiveCheckTool {
    /// Create a new live check tool with the given registry.
    pub fn new(versioned_registry: Arc<VersionedRegistry>) -> Self {
        Self { versioned_registry }
    }
}

/// Parameters for the live check tool.
#[derive(Debug, Deserialize)]
struct LiveCheckParams {
    /// Array of telemetry samples to check.
    samples: Vec<Sample>,
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

impl Tool for LiveCheckTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "live_check".to_owned(),
            description: "Run live-check on telemetry samples against the semantic conventions \
                          registry. Returns the samples with live_check_result fields populated \
                          containing advice and findings."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "samples": {
                        "type": "array",
                        "description": "Array of telemetry samples to check (attributes, spans, metrics, logs, or resources)",
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "attribute": {
                                            "type": "object",
                                            "properties": {
                                                "name": { "type": "string" },
                                                "value": {}
                                            },
                                            "required": ["name"]
                                        }
                                    },
                                    "required": ["attribute"]
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "span": {
                                            "type": "object",
                                            "properties": {
                                                "name": { "type": "string" },
                                                "kind": { "type": "string" },
                                                "attributes": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "name": { "type": "string" },
                                                            "value": {}
                                                        }
                                                    }
                                                }
                                            },
                                            "required": ["name"]
                                        }
                                    },
                                    "required": ["span"]
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "metric": {
                                            "type": "object",
                                            "properties": {
                                                "name": { "type": "string" },
                                                "attributes": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "name": { "type": "string" },
                                                            "value": {}
                                                        }
                                                    }
                                                }
                                            },
                                            "required": ["name"]
                                        }
                                    },
                                    "required": ["metric"]
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "log": {
                                            "type": "object",
                                            "properties": {
                                                "body": {},
                                                "attributes": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "name": { "type": "string" },
                                                            "value": {}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    "required": ["log"]
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "resource": {
                                            "type": "object",
                                            "properties": {
                                                "attributes": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "name": { "type": "string" },
                                                            "value": {}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    "required": ["resource"]
                                }
                            ]
                        }
                    }
                },
                "required": ["samples"]
            }),
        }
    }

    fn execute(&self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: LiveCheckParams = serde_json::from_value(arguments)?;
        let mut samples = params.samples;

        // Create LiveChecker with shared registry (Arc::clone is cheap)
        let mut live_checker =
            LiveChecker::new(Arc::clone(&self.versioned_registry), default_advisors());
        let mut stats = LiveCheckStatistics::Disabled(DisabledStatistics);

        // Run live check on each sample (mutates samples in place)
        for sample in &mut samples {
            let sample_clone = sample.clone();
            sample
                .run_live_check(&mut live_checker, &mut stats, None, &sample_clone)
                .map_err(|e| McpError::ToolExecution(format!("Live check failed: {e}")))?;
        }

        // Return the modified samples as JSON array
        Ok(ToolCallResult::text(serde_json::to_string_pretty(
            &samples,
        )?))
    }
}
