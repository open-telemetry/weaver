// SPDX-License-Identifier: Apache-2.0

//! Configuration structs for the `registry live-check` subcommand.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;
use weaver_checker::FindingLevel;

/// Configuration for the live-check subcommand.
///
/// All fields carry their defaults via the `Default` impl. TOML deserialization
/// with `#[serde(default)]` populates only the fields present in the file;
/// the rest keep their defaults. CLI args are applied on top via direct
/// mutation — no intermediate merge structs needed.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct LiveCheckConfig {
    /// Filters control which findings are dropped. A filter without `signal_type`
    /// applies globally; a filter with `signal_type` applies only to that signal type.
    #[serde(default)]
    pub finding_filters: Vec<FindingFilter>,

    /// Where to read the input telemetry from. `{file path}` | `stdin` | `otlp`.
    pub input_source: String,

    /// The format of the input telemetry. `text` | `json`. (Not used for OTLP.)
    pub input_format: String,

    /// Format used to render the report.
    /// Builtin formats: `json`, `yaml`, `jsonl`. Other values are template names
    /// (e.g. `ansi` selects the ansi templates).
    pub format: String,

    /// Path to the directory where the templates are located.
    pub templates: PathBuf,

    /// Disable stream mode. When set, the report is built up before being rendered.
    pub no_stream: bool,

    /// Disable statistics accumulation. Useful for long-running live-check sessions.
    pub no_stats: bool,

    /// Path to the directory where the generated artifacts will be saved.
    /// `none` disables all template output rendering.
    /// `http` sends the report as the response to the `/stop` request on the admin port.
    pub output: Option<PathBuf>,

    /// Advice policies directory. Overrides the built-in default policies.
    pub advice_policies: Option<PathBuf>,

    /// Advice preprocessor — a jq script run once over the registry data before
    /// being passed to rego policies.
    pub advice_preprocessor: Option<PathBuf>,

    /// OTLP listener settings (used when `input_source = "otlp"`).
    pub otlp: LiveCheckOtlpConfig,

    /// OTLP log emission settings.
    pub emit: LiveCheckEmitConfig,
}

impl Default for LiveCheckConfig {
    fn default() -> Self {
        Self {
            finding_filters: Vec::new(),
            input_source: "otlp".to_owned(),
            input_format: "json".to_owned(),
            format: "ansi".to_owned(),
            templates: PathBuf::from("live_check_templates"),
            no_stream: false,
            no_stats: false,
            output: None,
            advice_policies: None,
            advice_preprocessor: None,
            otlp: LiveCheckOtlpConfig::default(),
            emit: LiveCheckEmitConfig::default(),
        }
    }
}

/// OTLP listener settings for live-check.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct LiveCheckOtlpConfig {
    /// Address used by the gRPC OTLP listener.
    pub grpc_address: String,
    /// Port used by the gRPC OTLP listener.
    pub grpc_port: u16,
    /// Port used by the HTTP admin port (endpoints: `/stop`, `/health`).
    pub admin_port: u16,
    /// Max inactivity time in seconds before stopping the listener.
    pub inactivity_timeout: u64,
}

impl Default for LiveCheckOtlpConfig {
    fn default() -> Self {
        Self {
            grpc_address: "0.0.0.0".to_owned(),
            grpc_port: 4317,
            admin_port: 4320,
            inactivity_timeout: 10,
        }
    }
}

/// OTLP log emission settings for live-check.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
pub struct LiveCheckEmitConfig {
    /// Enable OTLP log emission for live-check policy findings.
    pub otlp_logs: bool,
    /// OTLP endpoint for log emission.
    pub otlp_logs_endpoint: String,
    /// Use stdout for OTLP log emission (debug mode).
    pub otlp_logs_stdout: bool,
}

impl Default for LiveCheckEmitConfig {
    fn default() -> Self {
        Self {
            otlp_logs: false,
            otlp_logs_endpoint: "http://localhost:4317".to_owned(),
            otlp_logs_stdout: false,
        }
    }
}

/// A filter that drops findings by ID exclusion or minimum level.
/// Optional `signal_type` scopes the filter to a specific signal type.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
pub struct FindingFilter {
    /// Drop findings with these IDs.
    pub exclude: Option<Vec<String>>,
    /// Drop all findings below this level.
    pub min_level: Option<FindingLevel>,
    /// Optional signal type scope. When set, this filter only applies to
    /// findings with a matching signal_type.
    pub signal_type: Option<String>,
    /// Drop all findings for samples with these names.
    /// For attribute samples, this matches the attribute key — e.g.
    /// `["trace.parent_id", "trace.span_id"]` suppresses all findings
    /// (e.g. `missing_attribute`) for those attribute keys.
    #[serde(default)]
    pub exclude_samples: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeaverConfig;
    use std::path::Path;

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
# Global filter (no signal_type)
[[live_check.finding_filters]]
exclude = ["deprecated", "missing_namespace"]
min_level = "improvement"

# Scoped filter (with signal_type)
[[live_check.finding_filters]]
signal_type = "span"
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = &config.live_check;

        assert_eq!(lc.finding_filters.len(), 2);

        // Global filter (no signal_type)
        assert!(lc.finding_filters[0].signal_type.is_none());
        assert_eq!(
            lc.finding_filters[0].exclude.as_deref(),
            Some(&["deprecated".to_owned(), "missing_namespace".to_owned()][..])
        );
        assert_eq!(
            lc.finding_filters[0].min_level,
            Some(FindingLevel::Improvement)
        );

        // Scoped filter (with signal_type)
        assert_eq!(lc.finding_filters[1].signal_type.as_deref(), Some("span"));
        assert_eq!(
            lc.finding_filters[1].exclude.as_deref(),
            Some(&["not_stable".to_owned()][..])
        );
    }

    #[test]
    fn test_parse_empty_config() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        assert_eq!(config.live_check.input_source, "otlp");
        assert_eq!(config.live_check.format, "ansi");
        assert!(config.live_check.finding_filters.is_empty());
    }

    #[test]
    fn test_parse_partial_config() {
        let toml = r#"
[[live_check.finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = &config.live_check;
        assert_eq!(lc.finding_filters.len(), 1);
        assert_eq!(
            lc.finding_filters[0].min_level,
            Some(FindingLevel::Violation)
        );
        assert_eq!(lc.format, "ansi");
        assert_eq!(lc.otlp.grpc_port, 4317);
    }

    #[test]
    fn test_parse_live_check_cli_settings() {
        let toml = r#"
[live_check]
input_source = "otlp"
input_format = "json"
format = "ansi"
templates = "live_check_templates"
no_stream = false
no_stats = true
output = "reports"
advice_policies = "policies"
advice_preprocessor = "pre.jq"

[live_check.otlp]
grpc_address = "127.0.0.1"
grpc_port = 4317
admin_port = 4320
inactivity_timeout = 30

[live_check.emit]
otlp_logs = true
otlp_logs_endpoint = "http://localhost:4317"
otlp_logs_stdout = false
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = &config.live_check;
        assert_eq!(lc.input_source, "otlp");
        assert_eq!(lc.input_format, "json");
        assert_eq!(lc.format, "ansi");
        assert_eq!(lc.templates, Path::new("live_check_templates"));
        assert!(!lc.no_stream);
        assert!(lc.no_stats);
        assert_eq!(lc.output.as_deref(), Some(Path::new("reports")));
        assert_eq!(lc.advice_policies.as_deref(), Some(Path::new("policies")));
        assert_eq!(lc.advice_preprocessor.as_deref(), Some(Path::new("pre.jq")));

        assert_eq!(lc.otlp.grpc_address, "127.0.0.1");
        assert_eq!(lc.otlp.grpc_port, 4317);
        assert_eq!(lc.otlp.admin_port, 4320);
        assert_eq!(lc.otlp.inactivity_timeout, 30);

        assert!(lc.emit.otlp_logs);
        assert_eq!(lc.emit.otlp_logs_endpoint, "http://localhost:4317");
        assert!(!lc.emit.otlp_logs_stdout);
    }

    #[test]
    fn test_defaults_applied_for_missing_sections() {
        let toml = r#"
[live_check.otlp]
grpc_port = 9999
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = &config.live_check;
        assert_eq!(lc.otlp.grpc_port, 9999);
        assert_eq!(lc.otlp.grpc_address, "0.0.0.0");
        assert_eq!(lc.otlp.admin_port, 4320);
        assert_eq!(lc.format, "ansi");
        assert!(!lc.emit.otlp_logs);
    }

    #[test]
    fn test_parse_exclude_samples() {
        let toml = r#"
[[live_check.finding_filters]]
exclude_samples = ["trace.parent_id", "trace.span_id", "trace.trace_id"]

[[live_check.finding_filters]]
signal_type = "span"
exclude_samples = ["custom.internal_id"]
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = &config.live_check;
        assert_eq!(lc.finding_filters.len(), 2);

        let f0 = &lc.finding_filters[0];
        assert!(f0.signal_type.is_none());
        assert!(f0.exclude.is_none());
        assert!(f0.min_level.is_none());
        assert_eq!(
            f0.exclude_samples,
            vec!["trace.parent_id", "trace.span_id", "trace.trace_id"]
        );

        let f1 = &lc.finding_filters[1];
        assert_eq!(f1.signal_type.as_deref(), Some("span"));
        assert_eq!(f1.exclude.as_deref(), Some(&["not_stable".to_owned()][..]));
        assert_eq!(f1.exclude_samples, vec!["custom.internal_id"]);
    }

    #[test]
    fn test_parse_exclude_samples_defaults_to_empty() {
        let toml = r#"
[[live_check.finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        assert!(config.live_check.finding_filters[0]
            .exclude_samples
            .is_empty());
    }
}
