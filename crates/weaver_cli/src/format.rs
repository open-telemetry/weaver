// SPDX-License-Identifier: Apache-2.0

//! Supported output formats for the resolved registry and telemetry schema

use clap::ValueEnum;

/// Supported output formats for the resolved schema
#[derive(Debug, Clone, ValueEnum)]
pub enum Format {
    /// YAML format
    Yaml,
    /// JSON format
    Json,
}
