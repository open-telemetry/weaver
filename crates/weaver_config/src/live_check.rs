// SPDX-License-Identifier: Apache-2.0

//! Configuration structs for the `registry live-check` subcommand.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_checker::FindingLevel;

/// Severity gate controlling when `registry live-check` exits non-zero.
///
/// Supported thresholds (highest → lowest severity): `Violation`,
/// `Improvement`, `Information`, `None`. A finding whose level is at or above
/// the chosen threshold causes a non-zero exit code. `None` disables the gate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FailOnLevel {
    /// Fail only on violations (default).
    #[default]
    Violation,
    /// Fail on improvements or violations.
    Improvement,
    /// Fail on any finding (information, improvement, or violation).
    Information,
    /// Never fail based on findings — always exit 0 unless an internal error
    /// occurs.
    None,
}

impl FailOnLevel {
    /// Map this gate to the underlying `FindingLevel` threshold, if any.
    /// Returns `None` for [`FailOnLevel::None`] (meaning "never fail").
    #[must_use]
    pub fn as_finding_threshold(self) -> Option<FindingLevel> {
        match self {
            Self::Violation => Some(FindingLevel::Violation),
            Self::Improvement => Some(FindingLevel::Improvement),
            Self::Information => Some(FindingLevel::Information),
            Self::None => None,
        }
    }
}

impl fmt::Display for FailOnLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Violation => "violation",
            Self::Improvement => "improvement",
            Self::Information => "information",
            Self::None => "none",
        };
        f.write_str(s)
    }
}

impl FromStr for FailOnLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "violation" => Ok(Self::Violation),
            "improvement" => Ok(Self::Improvement),
            "information" => Ok(Self::Information),
            "none" => Ok(Self::None),
            other => Err(format!(
                "invalid fail-on level '{other}' (expected one of: violation, improvement, information, none)"
            )),
        }
    }
}

/// Validate live telemetry against a semantic convention registry.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct LiveCheckConfig {
    /// Filters control which findings are dropped. A filter without `signal_type`
    /// applies globally; a filter with `signal_type` applies only to that signal type.
    #[serde(default)]
    pub finding_filters: Vec<FindingFilter>,

    /// Rules that override the level of matching findings instead of dropping
    /// them (e.g. promote `undefined_enum_variant` from `information` to
    /// `violation`). Scoped the same way as `finding_filters` (optional
    /// `signal_type` and `sample_names`). Applied before `finding_filters`,
    /// so a subsequent `min_level` filter sees the overridden level.
    #[serde(default)]
    pub finding_level_overrides: Vec<FindingLevelOverride>,

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

    /// Severity threshold that causes a non-zero exit code. Findings at this
    /// level or higher fail the run. Use `none` to never fail.
    pub fail_on: FailOnLevel,

    /// Path to the directory where the generated artifacts will be saved.
    /// `none` disables all template output rendering.
    /// `http` sends the report as the response to the `/stop` request on the admin port.
    pub output: Option<PathBuf>,

    /// Advice policies directory. Overrides the built-in default policies.
    pub advice_policies: Option<PathBuf>,

    /// Glob pattern pointing to additional JSON/YAML files to load into OPA rego data.
    /// Files are nested in OPA data using their relative path inside the glob base directory (e.g. schemas/user.json is loaded at data.user).
    pub advice_data: Option<String>,

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
            finding_level_overrides: Vec::new(),
            input_source: "otlp".to_owned(),
            input_format: "json".to_owned(),
            format: "ansi".to_owned(),
            templates: PathBuf::from("live_check_templates"),
            no_stream: false,
            no_stats: false,
            fail_on: FailOnLevel::default(),
            output: None,
            advice_policies: None,
            advice_data: None,
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
/// Optional `signal_type` and `sample_names` scope the filter to a specific
/// signal type and/or set of sample names.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
pub struct FindingFilter {
    /// Drop findings with these IDs.
    pub exclude: Option<Vec<String>>,
    /// Drop all findings below this level.
    pub min_level: Option<FindingLevel>,
    /// Optional signal type scope. When set, this filter only applies to
    /// findings with a matching signal_type.
    pub signal_type: Option<String>,
    /// Drop all findings for samples with these names (supports glob
    /// wildcards, e.g. `"trace.*"`).
    /// For attribute samples, this matches the attribute key — e.g.
    /// `["trace.parent_id", "trace.span_id"]` suppresses all findings
    /// (e.g. `missing_attribute`) for those attribute keys.
    #[serde(default)]
    pub exclude_samples: Vec<String>,
    /// Optional sample name scope (supports glob wildcards, e.g. `"http.*"`).
    /// When set, this filter only applies to samples whose name matches one
    /// of these patterns, in addition to any `signal_type` scope.
    #[serde(default)]
    pub sample_names: Vec<String>,
}

/// A rule that overrides the level of matching findings instead of dropping
/// them. Optional `signal_type` and `sample_names` scope the rule the same
/// way as `FindingFilter`.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
pub struct FindingLevelOverride {
    /// Override the level of findings with these IDs. When unset, applies to
    /// any finding ID within scope.
    pub ids: Option<Vec<String>>,
    /// The level to set on matching findings.
    pub level: FindingLevel,
    /// Optional signal type scope. When set, this rule only applies to
    /// findings with a matching signal_type.
    pub signal_type: Option<String>,
    /// Optional sample name scope (supports glob wildcards, e.g. `"http.*"`).
    /// When set, this rule only applies to samples whose name matches one of
    /// these patterns, in addition to any `signal_type` scope.
    #[serde(default)]
    pub sample_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeaverConfig;
    use std::path::Path;

    fn live_check(config: &WeaverConfig) -> LiveCheckConfig {
        config.command_config("live-check")
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
# Global filter (no signal_type)
[["live-check".finding_filters]]
exclude = ["deprecated", "missing_namespace"]
min_level = "improvement"

# Scoped filter (with signal_type)
[["live-check".finding_filters]]
signal_type = "span"
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);

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
        let lc = live_check(&config);
        assert_eq!(lc.input_source, "otlp");
        assert_eq!(lc.format, "ansi");
        assert!(lc.finding_filters.is_empty());
    }

    #[test]
    fn test_parse_partial_config() {
        let toml = r#"
[["live-check".finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
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
["live-check"]
input_source = "otlp"
input_format = "json"
format = "ansi"
templates = "live_check_templates"
no_stream = false
no_stats = true
fail_on = "improvement"
output = "reports"
advice_policies = "policies"
advice_data = "data"
advice_preprocessor = "pre.jq"

["live-check".otlp]
grpc_address = "127.0.0.1"
grpc_port = 4317
admin_port = 4320
inactivity_timeout = 30

["live-check".emit]
otlp_logs = true
otlp_logs_endpoint = "http://localhost:4317"
otlp_logs_stdout = false
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert_eq!(lc.input_source, "otlp");
        assert_eq!(lc.input_format, "json");
        assert_eq!(lc.format, "ansi");
        assert_eq!(lc.templates, Path::new("live_check_templates"));
        assert!(!lc.no_stream);
        assert!(lc.no_stats);
        assert_eq!(lc.fail_on, FailOnLevel::Improvement);
        assert_eq!(lc.output.as_deref(), Some(Path::new("reports")));
        assert_eq!(lc.advice_policies.as_deref(), Some(Path::new("policies")));
        assert_eq!(lc.advice_data.as_deref(), Some("data"));
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
["live-check".otlp]
grpc_port = 9999
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert_eq!(lc.otlp.grpc_port, 9999);
        assert_eq!(lc.otlp.grpc_address, "0.0.0.0");
        assert_eq!(lc.otlp.admin_port, 4320);
        assert_eq!(lc.format, "ansi");
        assert!(!lc.emit.otlp_logs);
    }

    #[test]
    fn test_parse_exclude_samples() {
        let toml = r#"
[["live-check".finding_filters]]
exclude_samples = ["trace.parent_id", "trace.span_id", "trace.trace_id"]

[["live-check".finding_filters]]
signal_type = "span"
exclude_samples = ["custom.internal_id"]
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
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
[["live-check".finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert!(lc.finding_filters[0].exclude_samples.is_empty());
    }

    #[test]
    fn test_parse_sample_names() {
        let toml = r#"
[["live-check".finding_filters]]
exclude = ["illegal_namespace"]
sample_names = ["server.address", "server.port", "http.*"]

[["live-check".finding_filters]]
signal_type = "span"
sample_names = ["custom.internal_id"]
exclude = ["not_stable"]
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert_eq!(lc.finding_filters.len(), 2);

        let f0 = &lc.finding_filters[0];
        assert!(f0.signal_type.is_none());
        assert_eq!(
            f0.exclude.as_deref(),
            Some(&["illegal_namespace".to_owned()][..])
        );
        assert_eq!(
            f0.sample_names,
            vec!["server.address", "server.port", "http.*"]
        );

        let f1 = &lc.finding_filters[1];
        assert_eq!(f1.signal_type.as_deref(), Some("span"));
        assert_eq!(f1.exclude.as_deref(), Some(&["not_stable".to_owned()][..]));
        assert_eq!(f1.sample_names, vec!["custom.internal_id"]);
    }

    #[test]
    fn test_parse_sample_names_defaults_to_empty() {
        let toml = r#"
[["live-check".finding_filters]]
min_level = "violation"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert!(lc.finding_filters[0].sample_names.is_empty());
    }

    #[test]
    fn test_parse_finding_level_overrides() {
        let toml = r#"
[["live-check".finding_level_overrides]]
ids = ["undefined_enum_variant"]
level = "violation"

[["live-check".finding_level_overrides]]
signal_type = "span"
sample_names = ["custom.*"]
ids = ["undefined_enum_variant"]
level = "improvement"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("Failed to parse TOML");
        let lc = live_check(&config);
        assert_eq!(lc.finding_level_overrides.len(), 2);

        let o0 = &lc.finding_level_overrides[0];
        assert!(o0.signal_type.is_none());
        assert!(o0.sample_names.is_empty());
        assert_eq!(
            o0.ids.as_deref(),
            Some(&["undefined_enum_variant".to_owned()][..])
        );
        assert_eq!(o0.level, FindingLevel::Violation);

        let o1 = &lc.finding_level_overrides[1];
        assert_eq!(o1.signal_type.as_deref(), Some("span"));
        assert_eq!(o1.sample_names, vec!["custom.*"]);
        assert_eq!(o1.level, FindingLevel::Improvement);
    }

    #[test]
    fn test_parse_finding_level_overrides_defaults_to_empty() {
        let config: WeaverConfig = toml::from_str("").expect("Failed to parse empty TOML");
        let lc = live_check(&config);
        assert!(lc.finding_level_overrides.is_empty());
    }

    #[test]
    fn test_fail_on_default_is_violation() {
        let lc = LiveCheckConfig::default();
        assert_eq!(lc.fail_on, FailOnLevel::Violation);
    }

    #[test]
    fn test_fail_on_round_trip_all_values() {
        for (text, expected) in [
            ("violation", FailOnLevel::Violation),
            ("improvement", FailOnLevel::Improvement),
            ("information", FailOnLevel::Information),
            ("none", FailOnLevel::None),
        ] {
            let toml = format!("fail_on = \"{text}\"\ninput_source = \"x\"\ninput_format = \"y\"\nformat = \"z\"\ntemplates = \"t\"\n");
            let lc: LiveCheckConfig = toml::from_str(&toml).expect("Failed to parse fail_on TOML");
            assert_eq!(lc.fail_on, expected, "round-trip for {text}");
            assert_eq!(expected.to_string(), text);
        }
    }

    #[test]
    fn test_fail_on_invalid_value_errors() {
        // Deserialize the typed config directly so we observe the error
        // instead of `command_config`'s silent `unwrap_or_default()`.
        let toml = "fail_on = \"bogus\"\ninput_source = \"x\"\ninput_format = \"y\"\nformat = \"z\"\ntemplates = \"t\"\n";
        let err = toml::from_str::<LiveCheckConfig>(toml).expect_err("expected parse error");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown variant") || msg.contains("bogus"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn test_fail_on_threshold_mapping() {
        assert_eq!(
            FailOnLevel::Violation.as_finding_threshold(),
            Some(FindingLevel::Violation)
        );
        assert_eq!(
            FailOnLevel::Improvement.as_finding_threshold(),
            Some(FindingLevel::Improvement)
        );
        assert_eq!(
            FailOnLevel::Information.as_finding_threshold(),
            Some(FindingLevel::Information)
        );
        assert_eq!(FailOnLevel::None.as_finding_threshold(), None);
    }

    #[test]
    fn test_fail_on_from_str() {
        use std::str::FromStr;
        assert_eq!(
            FailOnLevel::from_str("violation").expect("violation should parse"),
            FailOnLevel::Violation
        );
        assert_eq!(
            FailOnLevel::from_str("improvement").expect("improvement should parse"),
            FailOnLevel::Improvement
        );
        assert_eq!(
            FailOnLevel::from_str("information").expect("information should parse"),
            FailOnLevel::Information
        );
        assert_eq!(
            FailOnLevel::from_str("none").expect("none should parse"),
            FailOnLevel::None
        );
        let err = FailOnLevel::from_str("bogus").expect_err("bogus should fail to parse");
        assert!(err.contains("bogus"));
        assert!(err.contains("violation"));
    }
}
