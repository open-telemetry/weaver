// SPDX-License-Identifier: Apache-2.0

//! Perform checks on sample telemetry by:
//! - Comparing it to a semantic convention registry.
//! - Running built-in and custom policies to provide advice on how to improve the telemetry.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use include_dir::{include_dir, Dir};

use log::info;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::log_success;
use weaver_config::{override_if_set, CliOverrides, FieldMapping, LiveCheckConfig, WeaverConfig};
use weaver_forge::{OutputProcessor, OutputTarget};
use weaver_live_check::advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::finding_modifier::FindingModifier;
use weaver_live_check::json_file_ingester::JsonFileIngester;
use weaver_live_check::json_stdin_ingester::JsonStdinIngester;
use weaver_live_check::live_checker::LiveChecker;
use weaver_live_check::text_file_ingester::TextFileIngester;
use weaver_live_check::text_stdin_ingester::TextStdinIngester;
use weaver_live_check::{
    CumulativeStatistics, DisabledStatistics, Error, Ingester, LiveCheckReport, LiveCheckRunner,
    LiveCheckStatistics, Sample, VersionedRegistry,
};

use crate::registry::{load_config, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};

use super::otlp::otlp_ingester::OtlpIngester;
use super::otlp::AdminReportSender;

/// Embedded default live check templates
pub(crate) static DEFAULT_LIVE_CHECK_TEMPLATES: Dir<'_> =
    include_dir!("defaults/live_check_templates");

/// The input source
#[derive(Debug, Clone)]
enum InputSource {
    File(PathBuf),
    Stdin,
    Otlp,
}

impl From<String> for InputSource {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "stdin" | "s" => InputSource::Stdin,
            "otlp" | "o" => InputSource::Otlp,
            _ => InputSource::File(PathBuf::from(s)),
        }
    }
}

/// The input format
#[derive(Debug, Clone)]
enum InputFormat {
    Text,
    Json,
}

impl From<String> for InputFormat {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "js" => InputFormat::Json,
            _ => InputFormat::Text,
        }
    }
}

/// Parameters for the `registry live-check` sub-command.
///
/// Each setting may also be provided in `.weaver.toml`. CLI flags always take
/// precedence over config values, which take precedence over hardcoded defaults.
#[derive(Debug, Args)]
pub struct RegistryLiveCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Where to read the input telemetry from. {file path} | stdin | otlp
    /// (default: otlp)
    #[arg(long)]
    input_source: Option<String>,

    /// The format of the input telemetry. (Not required for OTLP). text | json
    /// (default: json)
    #[arg(long)]
    input_format: Option<String>,

    /// Format used to render the report.
    /// Builtin formats: json, yaml, jsonl (uses serde directly).
    /// Other values are treated as template names (e.g., "ansi" uses ansi templates).
    /// (default: ansi)
    #[arg(long)]
    format: Option<String>,

    /// Path to the directory where the templates are located.
    /// (default: live_check_templates)
    #[arg(long)]
    templates: Option<PathBuf>,

    /// Disable stream mode. Use this flag to disable streaming output.
    ///
    /// When the output is STDOUT, Ingesters that support streaming (STDIN and OTLP),
    /// by default output the live check results for each entity as they are ingested.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    no_stream: Option<bool>,

    /// Disable statistics accumulation. This is useful for long-running live check
    /// sessions. Typically combined with --emit-otlp-logs and --output=none.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    no_stats: Option<bool>,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the report is printed to stdout.
    /// Use "none" to disable all template output rendering (useful when emitting OTLP logs).
    /// Use "http" to send the report as the response to the /stop request on the admin port.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener. (default: 0.0.0.0)
    #[clap(long)]
    otlp_grpc_address: Option<String>,

    /// Port used by the gRPC OTLP listener. (default: 4317)
    #[clap(long)]
    otlp_grpc_port: Option<u16>,

    /// Enable OTLP log emission for live check policy findings
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    emit_otlp_logs: Option<bool>,

    /// OTLP endpoint for log emission (default: http://localhost:4317)
    #[arg(long)]
    otlp_logs_endpoint: Option<String>,

    /// Use stdout for OTLP log emission (debug mode)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    otlp_logs_stdout: Option<bool>,

    /// Port used by the HTTP admin port (endpoints: /stop). (default: 4320)
    #[clap(long)]
    admin_port: Option<u16>,

    /// Max inactivity time in seconds before stopping the listener. (default: 10)
    #[clap(long)]
    inactivity_timeout: Option<u64>,

    /// Advice policies directory. Set this to override the default policies.
    #[arg(long)]
    advice_policies: Option<PathBuf>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    ///
    /// Rego policies are run for each sample as it arrives in a stream. The preprocessor
    /// can be used to create a new data structure that is more efficient for the rego policies
    /// versus processing the data for every sample.
    #[arg(long)]
    advice_preprocessor: Option<PathBuf>,

    /// Path to a `.weaver.toml` config file. Skips automatic discovery when set.
    #[arg(long = "config")]
    config_path: Option<PathBuf>,
}

impl CliOverrides for RegistryLiveCheckArgs {
    type Config = LiveCheckConfig;
    const SUBCOMMAND: &'static str = "live-check";

    fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    fn extract_config(weaver_config: &WeaverConfig) -> LiveCheckConfig {
        weaver_config.live_check.clone()
    }

    fn config_only_fields() -> &'static [&'static str] {
        &[
            "finding_filters", // array of filter rules, too complex for CLI flags
        ]
    }

    fn cli_only_args() -> &'static [&'static str] {
        &[
            "config",                  // controls config loading, not a setting
            "registry",                // RegistryArgs (invocation-specific)
            "follow_symlinks",         // RegistryArgs
            "include_unreferenced",    // RegistryArgs
            "v2",                      // RegistryArgs
            "policy",                  // PolicyArgs
            "skip_policies",           // PolicyArgs
            "display_policy_coverage", // PolicyArgs
            "diagnostic_format",       // DiagnosticArgs
            "diagnostic_template",     // DiagnosticArgs
            "diagnostic_stdout",       // DiagnosticArgs
        ]
    }

    fn field_mappings() -> &'static [FieldMapping] {
        &[
            FieldMapping {
                config_name: "otlp_admin_port",
                cli_name: "admin_port",
            },
            FieldMapping {
                config_name: "otlp_inactivity_timeout",
                cli_name: "inactivity_timeout",
            },
            FieldMapping {
                config_name: "emit_otlp_logs_endpoint",
                cli_name: "otlp_logs_endpoint",
            },
            FieldMapping {
                config_name: "emit_otlp_logs_stdout",
                cli_name: "otlp_logs_stdout",
            },
        ]
    }

    fn apply_overrides(&self, config: &mut LiveCheckConfig) {
        override_if_set!(config.input_source, self.input_source);
        override_if_set!(config.input_format, self.input_format);
        override_if_set!(config.format, self.format);
        override_if_set!(config.templates, self.templates);
        override_if_set!(config.no_stream, self.no_stream);
        override_if_set!(config.no_stats, self.no_stats);
        override_if_set!(config.output, self.output, optional);
        override_if_set!(config.advice_policies, self.advice_policies, optional);
        override_if_set!(
            config.advice_preprocessor,
            self.advice_preprocessor,
            optional
        );
        override_if_set!(config.otlp.grpc_address, self.otlp_grpc_address);
        override_if_set!(config.otlp.grpc_port, self.otlp_grpc_port);
        override_if_set!(config.otlp.admin_port, self.admin_port);
        override_if_set!(config.otlp.inactivity_timeout, self.inactivity_timeout);
        override_if_set!(config.emit.otlp_logs, self.emit_otlp_logs);
        override_if_set!(config.emit.otlp_logs_endpoint, self.otlp_logs_endpoint);
        override_if_set!(config.emit.otlp_logs_stdout, self.otlp_logs_stdout);
    }
}

fn default_advisors() -> Vec<Box<dyn Advisor>> {
    vec![
        Box::new(DeprecatedAdvisor),
        Box::new(StabilityAdvisor),
        Box::new(TypeAdvisor),
        Box::new(EnumAdvisor),
    ]
}

/// Generate output for a complete report - handles line-oriented special case
fn generate_report(
    output: &mut OutputProcessor,
    samples: Vec<Sample>,
    stats: LiveCheckStatistics,
) -> Result<(), weaver_forge::error::Error> {
    // Special handling: one line per sample, stats at end
    if output.is_line_oriented() {
        for sample in &samples {
            output.generate(sample)?;
        }
        match stats {
            LiveCheckStatistics::Cumulative(_) => output.generate(&stats),
            LiveCheckStatistics::Disabled(_) => Ok(()),
        }
    } else {
        let report = LiveCheckReport {
            statistics: stats,
            samples,
        };
        output.generate(&report)
    }
}

/// Perform a live check on sample data by comparing it to a semantic convention registry.
pub(crate) fn command(args: &RegistryLiveCheckArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut exit_code = 0;

    // Load config: defaults -> .weaver.toml -> CLI overrides.
    let config = load_config(args)?;

    let input_source = InputSource::from(config.input_source.clone());
    let input_format = InputFormat::from(config.input_format.clone());

    // Detect --output http mode
    let is_http_output = config
        .output
        .as_ref()
        .is_some_and(|p| p.to_str() == Some("http"));

    if is_http_output && !matches!(&input_source, InputSource::Otlp) {
        return Err(DiagnosticMessages::from(Error::OutputError {
            error: "--output http is only valid with --input otlp".to_owned(),
        }));
    }

    // For http output, create the processor in stdout mode (used for format/template config)
    let target = if is_http_output {
        OutputTarget::Stdout
    } else {
        OutputTarget::from_optional_dir(config.output.as_ref())
    };
    let mut output = OutputProcessor::new(
        &config.format,
        "live_check",
        Some(&DEFAULT_LIVE_CHECK_TEMPLATES),
        Some(config.templates.clone()),
        target,
    )?;

    info!("Weaver Registry Live Check");

    // Prepare the registry
    info!("Resolving registry `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&args.registry, &args.policy);
    let resolved_registry = weaver.load_and_resolve_main(&mut diag_msgs)?;
    let registry = match resolved_registry {
        crate::weaver::Resolved::V2(resolved_v2) => {
            resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;
            VersionedRegistry::V2(Box::new(resolved_v2.into_template_schema()))
        }
        crate::weaver::Resolved::V1(resolved_v1) => {
            resolved_v1.check_after_resolution_policy(&mut diag_msgs)?;
            VersionedRegistry::V1(Box::new(resolved_v1.into_template_schema()))
        }
    };
    // Create the live checker with advisors
    let mut live_checker = LiveChecker::new(Arc::new(registry), default_advisors());

    live_checker.finding_modifier = FindingModifier::from_filters(&config.finding_filters);

    let rego_advisor = RegoAdvisor::new(
        &live_checker,
        &config.advice_policies,
        &config.advice_preprocessor,
    )?;
    live_checker.add_advisor(Box::new(rego_advisor));

    // Prepare the ingester
    let mut admin_report_sender: Option<AdminReportSender> = None;
    let ingester = match (&input_source, &input_format) {
        (InputSource::File(path), InputFormat::Text) => TextFileIngester::new(path).ingest()?,

        (InputSource::Stdin, InputFormat::Text) => TextStdinIngester::new().ingest()?,

        (InputSource::File(path), InputFormat::Json) => JsonFileIngester::new(path).ingest()?,

        (InputSource::Stdin, InputFormat::Json) => JsonStdinIngester::new().ingest()?,

        (InputSource::Otlp, _) => {
            let otlp = OtlpIngester {
                otlp_grpc_address: config.otlp.grpc_address.clone(),
                otlp_grpc_port: config.otlp.grpc_port,
                admin_port: config.otlp.admin_port,
                inactivity_timeout: config.otlp.inactivity_timeout,
            };
            let (iter, sender) = otlp.ingest_otlp()?;
            if is_http_output {
                sender
                    .expect_report
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            admin_report_sender = Some(sender);
            iter
        }
    };

    // Create Tokio runtime if OTLP log emission is enabled
    // Use a multi-threaded runtime so we can use spawn_blocking
    let rt = if config.emit.otlp_logs {
        Some(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    DiagnosticMessages::from(Error::OutputError {
                        error: format!("Failed to create Tokio runtime: {}", e),
                    })
                })?,
        )
    } else {
        None
    };

    // Enter the runtime context if OTLP emission is enabled
    // This is required for the batch exporter to spawn its background tasks
    let _guard = rt.as_ref().map(|rt| rt.enter());

    // Initialize OTLP emitter if requested
    // Must be done after entering the runtime
    if config.emit.otlp_logs {
        let emitter = if config.emit.otlp_logs_stdout {
            weaver_live_check::otlp_logger::OtlpEmitter::new_stdout()
        } else {
            weaver_live_check::otlp_logger::OtlpEmitter::new_grpc(&config.emit.otlp_logs_endpoint)?
        };
        live_checker.otlp_emitter = Some(std::rc::Rc::new(emitter));
    }

    let report_mode = if is_http_output || output.is_file_output() {
        // HTTP output and file output force report mode
        true
    } else {
        // This flag is not set by default. The user can set it to disable streaming output
        // and force report mode.
        config.no_stream
    };

    let mut stats = if config.no_stats {
        LiveCheckStatistics::Disabled(DisabledStatistics)
    } else {
        LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry))
    };

    let mut samples = Vec::new();
    for mut sample in ingester {
        sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone())?;
        //TODO: Check for violations and set exit_code here for no-stats mode
        if report_mode {
            samples.push(sample);
        } else {
            // Output this sample immediately (streaming mode)
            output.generate(&sample).map_err(DiagnosticMessages::from)?;
        }
    }

    stats.finalize();
    // Set the exit_code to a non-zero code if there are any violations
    if stats.has_violations() {
        exit_code = 1;
    }

    if is_http_output {
        let admin_waiting = admin_report_sender.as_ref().is_some_and(|s| {
            s.sender
                .lock()
                .expect("Failed to acquire lock on admin report sender")
                .is_some()
        });

        if admin_waiting {
            // Format report and send through admin channel
            let content_type = output.content_type().to_owned();
            let body = if output.is_line_oriented() {
                // For line-oriented formats (jsonl), build the body line by line
                let mut lines = Vec::new();
                for sample in &samples {
                    lines.push(
                        output
                            .generate_to_string(sample)
                            .map_err(DiagnosticMessages::from)?,
                    );
                }
                match &stats {
                    LiveCheckStatistics::Cumulative(_) => {
                        lines.push(
                            output
                                .generate_to_string(&stats)
                                .map_err(DiagnosticMessages::from)?,
                        );
                    }
                    LiveCheckStatistics::Disabled(_) => {}
                }
                lines.join("\n")
            } else {
                let report = LiveCheckReport {
                    statistics: stats,
                    samples,
                };
                output
                    .generate_to_string(&report)
                    .map_err(DiagnosticMessages::from)?
            };
            if let Some(report) = admin_report_sender.take() {
                if let Some(sender) = report
                    .sender
                    .lock()
                    .expect("Failed to acquire lock on admin report sender")
                    .take()
                {
                    let _ = sender.send((content_type, body));
                }
            }
        } else {
            // No HTTP client waiting (SIGINT/inactivity stop), fall back to stdout
            generate_report(&mut output, samples, stats).map_err(DiagnosticMessages::from)?;
        }
    } else if report_mode {
        generate_report(&mut output, samples, stats).map_err(DiagnosticMessages::from)?;
    } else {
        // Stats only (streaming mode finished)
        output.generate(&stats).map_err(DiagnosticMessages::from)?;
    }

    // Shutdown OTLP emitter to flush any pending log records
    if let Some(emitter) = live_checker.otlp_emitter {
        emitter.shutdown()?;
    }

    log_success(format!(
        "Performed live check for registry `{}`",
        args.registry.registry
    ));

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code,
        warnings: Some(diag_msgs),
    })
}

#[cfg(test)]
mod tests {
    use super::RegistryLiveCheckArgs;
    use crate::registry::tests::assert_config_cli_consistency;

    #[test]
    fn config_fields_match_cli_args() {
        assert_config_cli_consistency::<RegistryLiveCheckArgs>();
    }
}
