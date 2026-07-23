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
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::{log_success, log_warn};
use weaver_config::{FailOnLevel, WeaverConfig};
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
    sample_resource::SampleResource, CumulativeStatistics, DisabledStatistics, Error, Ingester,
    LiveCheckReport, LiveCheckRunner, LiveCheckStatistics, Sample, VersionedRegistry,
};
use weaver_macros::weaver_command;

use crate::registry::{load_config, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_config::WeaverCommand;

use super::otlp::otlp_ingester::OtlpIngester;
use super::otlp::{AdminController, AdminDrainGuard};

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

/// Validate live telemetry against a semantic convention registry.
#[weaver_command(
    section = "live-check",
    config_type = "::weaver_config::LiveCheckConfig",
    extra_config_only = "finding_filters,finding_level_overrides"
)]
#[derive(Debug, Args, WeaverCommand)]
pub struct RegistryLiveCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    #[shared(registry)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    #[shared(policy)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,

    /// Where to read the input telemetry from. {file path} | stdin | otlp
    #[arg(long)]
    #[config(default = "otlp")]
    input_source: Option<String>,

    /// The format of the input telemetry. text | json (not required for OTLP)
    #[arg(long)]
    #[config(default = "json")]
    input_format: Option<String>,

    /// Format used to render the report.
    /// Builtin formats: json, yaml, jsonl. Other values are template names (e.g. "ansi").
    #[arg(long)]
    #[config(default = "ansi")]
    format: Option<String>,

    /// Path to the directory where the templates are located.
    #[arg(long)]
    #[config(default = "live_check_templates")]
    templates: Option<PathBuf>,

    /// Disable stream mode (build report before rendering).
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    #[config(default = "false")]
    no_stream: Option<bool>,

    /// Disable statistics accumulation. Useful for long-running sessions.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    #[config(default = "false")]
    no_stats: Option<bool>,

    /// Findings at this level or higher cause a non-zero exit code.
    /// Levels (highest→lowest): violation, improvement, information.
    /// Use `none` to never fail.
    #[arg(long)]
    #[config(default = "violation")]
    fail_on: Option<FailOnLevel>,

    /// Path to save generated artifacts. Use "none" to suppress output,
    /// "http" to send as the /stop response.
    #[arg(short, long)]
    #[config]
    output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener.
    #[clap(long)]
    #[config(path = "otlp.grpc_address")]
    otlp_grpc_address: Option<String>,

    /// Port used by the gRPC OTLP listener.
    #[clap(long)]
    #[config(path = "otlp.grpc_port")]
    otlp_grpc_port: Option<u16>,

    /// Enable OTLP log emission for live check policy findings.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    #[config(path = "emit.otlp_logs")]
    emit_otlp_logs: Option<bool>,

    /// OTLP endpoint for log emission.
    #[arg(long)]
    #[config(path = "emit.otlp_logs_endpoint")]
    otlp_logs_endpoint: Option<String>,

    /// Use stdout for OTLP log emission (debug mode).
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    #[config(path = "emit.otlp_logs_stdout")]
    otlp_logs_stdout: Option<bool>,

    /// Port used by the HTTP admin port (endpoints: /stop).
    #[clap(long)]
    #[config(path = "otlp.admin_port")]
    admin_port: Option<u16>,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long)]
    #[config(path = "otlp.inactivity_timeout")]
    inactivity_timeout: Option<u64>,

    /// Advice policies directory. Set this to override the default policies.
    #[arg(long)]
    #[config]
    advice_policies: Option<PathBuf>,

    /// Glob pattern pointing to additional JSON/YAML files to load into OPA rego data (other extensions are ignored). Files are nested in OPA data using their relative path inside the glob base directory (e.g. schemas/user.json is loaded at data.user).
    #[arg(long)]
    #[config]
    advice_data: Option<String>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    #[arg(long)]
    #[config]
    advice_preprocessor: Option<PathBuf>,
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
pub(crate) fn command(
    args: &RegistryLiveCheckArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut exit_code = 0;

    let cmd_config = load_config(args, cfg);
    let config = cmd_config.config;
    let registry_args = cmd_config.registry;
    let policy_args = cmd_config.policy;

    // --no-stats disables the statistics accumulator, so --fail-on cannot be
    // evaluated. Warn the user so the silent no-op is visible.
    if config.no_stats && config.fail_on != FailOnLevel::None {
        log_warn(format!(
            "--no-stats disables statistics; --fail-on={} cannot be enforced. \
             The command will exit 0 regardless of findings. \
             Pass --fail-on=none to suppress this warning.",
            config.fail_on
        ));
    }

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
    info!("Resolving registry `{}`", registry_args.registry);

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&registry_args, &policy_args, auth);
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

    live_checker.finding_modifier =
        FindingModifier::from_rules(&config.finding_filters, &config.finding_level_overrides)?;

    let rego_advisor = RegoAdvisor::new(
        &live_checker,
        &config.advice_policies,
        &config.advice_preprocessor,
        &config.advice_data,
    )?;
    live_checker.add_advisor(Box::new(rego_advisor));

    // Prepare the ingester
    let mut admin_controller: Option<AdminController> = None;
    // Write-only: kept alive only so its `Drop` impl (signal + drain the
    // OTLP receiver's background thread) fires no matter how this function
    // returns, including through the several `?` early-returns below.
    let mut _admin_thread: Option<AdminDrainGuard> = None;
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
            let (iter, controller, handle) = otlp.ingest_otlp()?;
            if is_http_output {
                controller.enable_http_report();
            }
            _admin_thread = Some(AdminDrainGuard::new(controller.clone(), handle));
            admin_controller = Some(controller);
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
    let mut current_resource: Option<std::rc::Rc<SampleResource>> = None;
    for mut sample in ingester {
        // Track the most-recently-seen resource and attach it to signals that carry one.
        // This mirrors what the OTLP ingester does when building metrics/logs/spans from
        // OTLP protocol buffers, and allows JSON file inputs to associate a resource.
        match &mut sample {
            Sample::Resource(r) => current_resource = Some(std::rc::Rc::new(r.clone())),
            Sample::Metric(m) => m.resource = current_resource.clone(),
            Sample::Log(l) => l.resource = current_resource.clone(),
            Sample::Span(s) => s.resource = current_resource.clone(),
            _ => {}
        }
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
    // Set exit_code based on the configured --fail-on threshold. `None`
    // threshold means "never fail". `should_fail` returns false for disabled
    // stats; the startup check above warns about --no-stats + non-`none` gates.
    if let Some(threshold) = config.fail_on.as_finding_threshold() {
        if stats.should_fail(threshold) {
            exit_code = 1;
        }
    }

    if is_http_output {
        let admin_waiting = admin_controller
            .as_ref()
            .is_some_and(AdminController::has_report_waiter);

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
            if let Some(controller) = admin_controller.take() {
                let _ = controller.deliver_report(content_type, body);
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

    // `_admin_thread`'s Drop impl waits for the OTLP receiver's background
    // thread to fully drain (gRPC + HTTP admin servers) once it goes out of
    // scope at the end of this function — on every return path, not just
    // this one — so process::exit in main() can't cut off an in-flight
    // request (e.g. the /stop response above). Bounded so a stuck drain can
    // never hang the process.

    // Shutdown OTLP emitter to flush any pending log records
    if let Some(emitter) = live_checker.otlp_emitter {
        emitter.shutdown()?;
    }

    log_success(format!(
        "Performed live check for registry `{}`",
        registry_args.registry
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
