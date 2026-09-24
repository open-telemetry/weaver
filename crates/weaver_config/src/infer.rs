// SPDX-License-Identifier: Apache-2.0

//! Configuration structs for the `registry infer` subcommand.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

/// Infer a semantic convention registry by observing live OTLP telemetry.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(inline)]
pub struct InferConfig {
    /// Output folder for generated YAML files.
    pub output: PathBuf,
    /// OTLP listener settings.
    pub otlp: InferOtlpConfig,
}

impl Default for InferConfig {
    fn default() -> Self {
        Self {
            output: PathBuf::from("./inferred-registry/"),
            otlp: InferOtlpConfig::default(),
        }
    }
}

/// OTLP listener settings for infer.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct InferOtlpConfig {
    /// Address used by the gRPC OTLP listener.
    pub grpc_address: String,
    /// Port used by the gRPC OTLP listener.
    pub grpc_port: u16,
    /// Port used by the HTTP admin server (endpoints: `/stop`, `/health`).
    pub admin_port: u16,
    /// Seconds of inactivity before auto-stop (0 = never).
    pub inactivity_timeout: u64,
}

impl Default for InferOtlpConfig {
    fn default() -> Self {
        Self {
            grpc_address: "127.0.0.1".to_owned(),
            grpc_port: 4317,
            admin_port: 8080,
            inactivity_timeout: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeaverConfig;
    use std::path::Path;

    fn infer(config: &WeaverConfig) -> InferConfig {
        config
            .command_config("infer")
            .expect("the fixture section deserializes")
    }

    #[test]
    fn test_parse_empty_config() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert_eq!(infer(&config), InferConfig::default());
    }

    #[test]
    fn test_parse_infer_settings() {
        let toml = r#"
[infer]
output = "out"

[infer.otlp]
grpc_address = "0.0.0.0"
grpc_port = 5317
admin_port = 5320
inactivity_timeout = 0
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let ic = infer(&config);
        assert_eq!(ic.output, Path::new("out"));
        assert_eq!(ic.otlp.grpc_address, "0.0.0.0");
        assert_eq!(ic.otlp.grpc_port, 5317);
        assert_eq!(ic.otlp.admin_port, 5320);
        assert_eq!(ic.otlp.inactivity_timeout, 0);
    }

    #[test]
    fn test_partial_otlp_keeps_defaults() {
        let toml = r#"
[infer.otlp]
grpc_port = 9999
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let ic = infer(&config);
        assert_eq!(ic.otlp.grpc_port, 9999);
        assert_eq!(ic.otlp.grpc_address, "127.0.0.1");
        assert_eq!(ic.otlp.admin_port, 8080);
    }

    #[test]
    fn test_flat_listener_key_is_rejected() {
        let toml = r#"
[infer]
grpc_port = 4317
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let result: Result<InferConfig, _> = config.command_config("infer");
        assert!(result.is_err());
    }
}
