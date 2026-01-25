// SPDX-License-Identifier: Apache-2.0

//! Perform checks on sample telemetry by:
//! - Comparing it to a semantic convention registry.
//! - Running built-in and custom policies to provide advice on how to improve the telemetry.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Args;
use include_dir::{include_dir, Dir};
use serde::Serialize;

use log::info;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::log_success;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_live_check::advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::json_file_ingester::JsonFileIngester;
use weaver_live_check::json_stdin_ingester::JsonStdinIngester;
use weaver_live_check::live_checker::LiveChecker;
use weaver_live_check::text_file_ingester::TextFileIngester;
use weaver_live_check::text_stdin_ingester::TextStdinIngester;
use weaver_live_check::{
    CumulativeStatistics, DisabledStatistics, Error, Ingester, LiveCheckReport, LiveCheckRunner,
    LiveCheckStatistics, Sample, VersionedRegistry,
};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};

use super::otlp::otlp_ingester::OtlpIngester;

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

/// Output processor - handles all output generation with embedded config
#[allow(clippy::large_enum_variant)]
enum OutputProcessor {
    /// JSON format - pretty-printed using serde_json
    Json {
        path: PathBuf,
        directive: OutputDirective,
    },
    /// YAML format - using serde_yaml
    Yaml {
        path: PathBuf,
        directive: OutputDirective,
    },
    /// JSONL format - one compact JSON object per line
    Jsonl {
        path: PathBuf,
        directive: OutputDirective,
    },
    /// Template-based format with embedded engine
    Template {
        engine: TemplateEngine,
        path: PathBuf,
        directive: OutputDirective,
    },
    /// No output (--output=none)
    Mute,
}

impl OutputProcessor {
    /// Create an OutputProcessor from format string, templates path, and output option
    fn new(
        format: &str,
        templates: PathBuf,
        output: Option<&PathBuf>,
    ) -> Result<Self, DiagnosticMessages> {
        // Check if output is disabled (--output=none)
        if output.is_some_and(|p| p.to_str() == Some("none")) {
            return Ok(OutputProcessor::Mute);
        }

        // Determine output path and directive
        let (path, directive) = match output {
            Some(p) => (p.clone(), OutputDirective::File),
            None => (PathBuf::from("output"), OutputDirective::Stdout),
        };

        match format.to_lowercase().as_str() {
            "mute" => Ok(OutputProcessor::Mute),
            "json" => Ok(OutputProcessor::Json { path, directive }),
            "yaml" => Ok(OutputProcessor::Yaml { path, directive }),
            "jsonl" => Ok(OutputProcessor::Jsonl { path, directive }),
            template_name => {
                let loader = EmbeddedFileLoader::try_new(
                    &DEFAULT_LIVE_CHECK_TEMPLATES,
                    templates,
                    template_name,
                )
                .map_err(|e| {
                    DiagnosticMessages::from(Error::OutputError {
                        error: format!(
                            "Failed to create the embedded file loader for the live check templates: {e}"
                        ),
                    })
                })?;
                let config = WeaverConfig::try_from_loader(&loader).map_err(|e| {
                    DiagnosticMessages::from(Error::OutputError {
                        error: format!(
                            "Failed to load `defaults/live_check_templates/weaver.yaml`: {e}"
                        ),
                    })
                })?;
                let engine = TemplateEngine::try_new(config, loader, Params::default())?;
                Ok(OutputProcessor::Template {
                    engine,
                    path,
                    directive,
                })
            }
        }
    }

    /// Generate output for data
    fn generate<T: Serialize>(&self, data: &T) -> Result<(), Error> {
        match self {
            OutputProcessor::Json { path, directive } => {
                let content =
                    serde_json::to_string_pretty(data).map_err(|e| Error::OutputError {
                        error: e.to_string(),
                    })?;
                Self::write_output(&content, path, directive, "live_check.json")
            }
            OutputProcessor::Yaml { path, directive } => {
                let content = serde_yaml::to_string(data).map_err(|e| Error::OutputError {
                    error: e.to_string(),
                })?;
                Self::write_output(&content, path, directive, "live_check.yaml")
            }
            OutputProcessor::Jsonl { path, directive } => {
                let content = serde_json::to_string(data).map_err(|e| Error::OutputError {
                    error: e.to_string(),
                })?;
                Self::write_output(&content, path, directive, "live_check.jsonl")
            }
            OutputProcessor::Template {
                engine,
                path,
                directive,
            } => engine
                .generate(data, path, directive)
                .map_err(|e| Error::OutputError {
                    error: e.to_string(),
                }),
            OutputProcessor::Mute => Ok(()),
        }
    }

    /// Generate output for a complete report - handles JSONL special case
    fn generate_report(
        &self,
        samples: Vec<Sample>,
        stats: LiveCheckStatistics,
    ) -> Result<(), Error> {
        match self {
            OutputProcessor::Jsonl { .. } => {
                // JSONL: one line per sample, stats at end
                for sample in &samples {
                    self.generate(sample)?;
                }
                self.generate(&stats)
            }
            OutputProcessor::Mute => Ok(()),
            _ => {
                // All other formats: output as a single report structure
                let report = LiveCheckReport {
                    statistics: stats,
                    samples,
                };
                self.generate(&report)
            }
        }
    }

    /// Write content to output destination
    #[allow(clippy::print_stderr)]
    fn write_output(
        content: &str,
        path: &Path,
        directive: &OutputDirective,
        filename: &str,
    ) -> Result<(), Error> {
        match directive {
            OutputDirective::Stdout => {
                println!("{content}");
                Ok(())
            }
            OutputDirective::File => {
                let file_path = path.join(filename);
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| Error::OutputError {
                        error: format!("Failed to create directory {}: {e}", parent.display()),
                    })?;
                }
                std::fs::write(&file_path, content).map_err(|e| Error::OutputError {
                    error: format!("Failed to write to {}: {e}", file_path.display()),
                })
            }
            OutputDirective::Stderr => {
                eprintln!("{content}");
                Ok(())
            }
        }
    }

    /// Returns true if file output is being used
    fn is_file_output(&self) -> bool {
        match self {
            OutputProcessor::Json { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Yaml { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Jsonl { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Template { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Mute => false,
        }
    }
}

/// Parameters for the `registry live-check` sub-command
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
    #[arg(long, default_value = "otlp")]
    input_source: InputSource,

    /// The format of the input telemetry. (Not required for OTLP). text | json
    #[arg(long, default_value = "json")]
    input_format: InputFormat,

    /// Format used to render the report.
    /// Builtin formats: json, yaml, jsonl (uses serde directly).
    /// Other values are treated as template names (e.g., "ansi" uses ansi templates).
    #[arg(long, default_value = "ansi")]
    format: String,

    /// Path to the directory where the templates are located.
    #[arg(long, default_value = "live_check_templates")]
    templates: PathBuf,

    /// Disable stream mode. Use this flag to disable streaming output.
    ///
    /// When the output is STDOUT, Ingesters that support streaming (STDIN and OTLP),
    /// by default output the live check results for each entity as they are ingested.
    #[arg(long, default_value = "false")]
    no_stream: bool,

    /// Disable statistics accumulation. This is useful for long-running live check
    /// sessions. Typically combined with --emit-otlp-logs and --output=none.
    #[arg(long, default_value = "false")]
    no_stats: bool,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the report is printed to stdout.
    /// Use "none" to disable all template output rendering (useful when emitting OTLP logs).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317")]
    otlp_grpc_port: u16,

    /// Enable OTLP log emission for live check policy findings
    #[arg(long, default_value = "false")]
    emit_otlp_logs: bool,

    /// OTLP endpoint for log emission
    #[arg(long, default_value = "http://localhost:4317")]
    otlp_logs_endpoint: String,

    /// Use stdout for OTLP log emission (debug mode)
    #[arg(long, default_value = "false")]
    otlp_logs_stdout: bool,

    /// Port used by the HTTP admin port (endpoints: /stop).
    #[clap(long, default_value = "4320")]
    admin_port: u16,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long, default_value = "10")]
    inactivity_timeout: u64,

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
}

fn default_advisors() -> Vec<Box<dyn Advisor>> {
    vec![
        Box::new(DeprecatedAdvisor),
        Box::new(StabilityAdvisor),
        Box::new(TypeAdvisor),
        Box::new(EnumAdvisor),
    ]
}

/// Perform a live check on sample data by comparing it to a semantic convention registry.
pub(crate) fn command(args: &RegistryLiveCheckArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut exit_code = 0;

    // Create output processor (handles format, path, directive, and mute)
    let output = OutputProcessor::new(&args.format, args.templates.clone(), args.output.as_ref())?;

    info!("Weaver Registry Live Check");

    // Prepare the registry
    info!("Resolving registry `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&args.registry, &args.policy);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;
    let registry = if args.registry.v2 {
        let resolved_v2 = resolved.try_into_v2()?;
        resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;
        VersionedRegistry::V2(resolved_v2.into_template_schema())
    } else {
        resolved.check_after_resolution_policy(&mut diag_msgs)?;
        VersionedRegistry::V1(resolved.into_template_schema())
    };
    // Create the live checker with advisors
    let mut live_checker = LiveChecker::new(Arc::new(registry), default_advisors());

    let rego_advisor = RegoAdvisor::new(
        &live_checker,
        &args.advice_policies,
        &args.advice_preprocessor,
    )?;
    live_checker.add_advisor(Box::new(rego_advisor));

    // Prepare the ingester
    let ingester = match (&args.input_source, &args.input_format) {
        (InputSource::File(path), InputFormat::Text) => TextFileIngester::new(path).ingest()?,

        (InputSource::Stdin, InputFormat::Text) => TextStdinIngester::new().ingest()?,

        (InputSource::File(path), InputFormat::Json) => JsonFileIngester::new(path).ingest()?,

        (InputSource::Stdin, InputFormat::Json) => JsonStdinIngester::new().ingest()?,

        (InputSource::Otlp, _) => (OtlpIngester {
            otlp_grpc_address: args.otlp_grpc_address.clone(),
            otlp_grpc_port: args.otlp_grpc_port,
            admin_port: args.admin_port,
            inactivity_timeout: args.inactivity_timeout,
        })
        .ingest()?,
    };

    // Create Tokio runtime if OTLP log emission is enabled
    // Use a multi-threaded runtime so we can use spawn_blocking
    let rt = if args.emit_otlp_logs {
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
    if args.emit_otlp_logs {
        let emitter = if args.otlp_logs_stdout {
            weaver_live_check::otlp_logger::OtlpEmitter::new_stdout()
        } else {
            weaver_live_check::otlp_logger::OtlpEmitter::new_grpc(&args.otlp_logs_endpoint)?
        };
        live_checker.otlp_emitter = Some(std::rc::Rc::new(emitter));
    }

    let report_mode = if output.is_file_output() {
        // File output forces report mode
        true
    } else {
        // This flag is not set by default. The user can set it to disable streaming output
        // and force report mode.
        args.no_stream
    };

    let mut stats = if args.no_stats {
        LiveCheckStatistics::Disabled(DisabledStatistics)
    } else {
        LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry))
    };
    let mut samples = Vec::new();
    for mut sample in ingester {
        sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone())?;

        if report_mode && !args.no_stats {
            samples.push(sample);
        } else {
            // Output this sample immediately (streaming mode)
            output.generate(&sample).map_err(DiagnosticMessages::from)?;
        }
    }

    // Finalize and output stats if stats are enabled
    if !args.no_stats {
        stats.finalize();
        // Set the exit_code to a non-zero code if there are any violations
        if stats.has_violations() {
            exit_code = 1;
        }

        if report_mode {
            output
                .generate_report(samples, stats)
                .map_err(DiagnosticMessages::from)?;
        } else {
            // Stats only (streaming mode finished)
            output.generate(&stats).map_err(DiagnosticMessages::from)?;
        }
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
