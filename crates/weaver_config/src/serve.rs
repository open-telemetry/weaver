// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `serve` command.

use std::net::SocketAddr;

use schemars::JsonSchema;
use serde::Deserialize;

/// Serve-specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct ServeConfig {
    /// Address to bind the server to (e.g. `127.0.0.1:8080`).
    #[schemars(with = "String")]
    pub bind: SocketAddr,
    /// Allowed CORS origins (comma-separated). Use `*` for any origin.
    pub cors_origins: Option<String>,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".parse().expect("valid default bind"),
            cors_origins: None,
        }
    }
}
