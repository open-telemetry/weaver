// SPDX-License-Identifier: Apache-2.0

//! Supported output formats for the resolved registry and telemetry schema

use clap::ValueEnum;
use serde::Serialize;

/// Supported output formats for the resolved schema
#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum Format {
    /// YAML format
    Yaml,
    /// JSON format
    Json,
}

#[cfg(not(tarpaulin_include))]
pub(crate) fn apply_format<T: Serialize>(format: &Format, object: &T) -> Result<String, String> {
    match format {
        Format::Yaml => serde_yaml::to_string(object)
            .map_err(|e| format!("Failed to serialize in Yaml the resolved registry: {:?}", e)),
        Format::Json => serde_json::to_string_pretty(object)
            .map_err(|e| format!("Failed to serialize in Json the resolved registry: {:?}", e)),
    }
}
