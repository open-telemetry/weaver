// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for instrumentation scope metadata.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::sample_attribute::SampleAttribute;

/// Identifies the instrumentation scope that produced a telemetry signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleInstrumentationScope {
    /// Instrumentation scope name.
    #[serde(default)]
    pub name: String,
    /// Instrumentation scope version.
    #[serde(default)]
    pub version: String,
    /// Schema URL declared by the OTLP scope container.
    #[serde(default)]
    pub schema_url: String,
    /// Instrumentation scope attributes.
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Number of scope attributes dropped before export.
    #[serde(default)]
    pub dropped_attributes_count: u32,
}
