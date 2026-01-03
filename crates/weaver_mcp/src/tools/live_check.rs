// SPDX-License-Identifier: Apache-2.0

//! Live check tool for validating telemetry samples against the registry.

use std::path::PathBuf;
use std::sync::Arc;

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use weaver_live_check::advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::live_checker::LiveChecker;
use weaver_live_check::{
    DisabledStatistics, LiveCheckRunner, LiveCheckStatistics, Sample, VersionedRegistry,
};

use super::{Tool, ToolCallResult, ToolDefinition};
use crate::error::McpError;

/// Tool for running live-check on telemetry samples.
///
/// This tool holds a pre-initialized `LiveChecker` that is reused across calls
/// for efficiency. The LiveChecker includes all configured advisors (built-in
/// and optionally Rego-based).
pub struct LiveCheckTool {
    live_checker: LiveChecker,
}

impl LiveCheckTool {
    /// Create a new live check tool with pre-initialized LiveChecker.
    ///
    /// # Arguments
    ///
    /// * `versioned_registry` - The semantic convention registry.
    /// * `advice_policies` - Optional path to custom Rego policies directory.
    /// * `advice_preprocessor` - Optional path to jq preprocessor script.
    ///
    /// # Errors
    ///
    /// Returns an error if RegoAdvisor initialization fails.
    pub fn new(
        versioned_registry: Arc<VersionedRegistry>,
        advice_policies: Option<PathBuf>,
        advice_preprocessor: Option<PathBuf>,
    ) -> Result<Self, McpError> {
        // Create LiveChecker with default advisors
        let mut live_checker = LiveChecker::new(versioned_registry, default_advisors());

        // Add RegoAdvisor for policy-based advice
        let rego_advisor = RegoAdvisor::new(&live_checker, &advice_policies, &advice_preprocessor)
            .map_err(|e| {
                McpError::ToolExecution(format!("Failed to initialize RegoAdvisor: {e}"))
            })?;
        live_checker.add_advisor(Box::new(rego_advisor));

        Ok(Self { live_checker })
    }
}

/// Parameters for the live check tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct LiveCheckParams {
    /// Array of telemetry samples to check (attributes, spans, metrics, logs, or resources).
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
            input_schema: serde_json::to_value(schema_for!(LiveCheckParams))
                .expect("LiveCheckParams schema should serialize"),
        }
    }

    fn execute(&mut self, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params: LiveCheckParams = serde_json::from_value(arguments)?;
        let mut samples = params.samples;
        let mut stats = LiveCheckStatistics::Disabled(DisabledStatistics);

        // Run live check on each sample (mutates samples in place)
        for sample in &mut samples {
            let sample_clone = sample.clone();
            sample
                .run_live_check(&mut self.live_checker, &mut stats, None, &sample_clone)
                .map_err(|e| McpError::ToolExecution(format!("Live check failed: {e}")))?;
        }

        // Return the modified samples as JSON array
        Ok(ToolCallResult::text(serde_json::to_string_pretty(
            &samples,
        )?))
    }
}
