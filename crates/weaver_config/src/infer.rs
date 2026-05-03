// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry infer` command.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Infer-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct InferConfig {
    /// Output folder for generated YAML files.
    pub output: PathBuf,
    /// Address used by the gRPC OTLP listener.
    pub grpc_address: String,
    /// Port used by the gRPC OTLP listener.
    pub grpc_port: u16,
    /// Port used by the HTTP admin server.
    pub admin_port: u16,
    /// Seconds of inactivity before auto-stop (0 = never).
    pub inactivity_timeout: u64,
}

impl Default for InferConfig {
    fn default() -> Self {
        Self {
            output: PathBuf::from("./inferred-registry/"),
            grpc_address: "0.0.0.0".to_owned(),
            grpc_port: 4317,
            admin_port: 8080,
            inactivity_timeout: 60,
        }
    }
}
