// SPDX-License-Identifier: Apache-2.0

//! Perform checks on sample telemetry by:
//! - Comparing it to a semantic convention registry.
//! - Running built-in and custom policies to provide advice on how to improve the telemetry.

use std::path::PathBuf;

use clap::Args;
use include_dir::{include_dir, Dir};

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
    Error, Ingester, LiveCheckReport, LiveCheckRunner, LiveCheckStatistics, VersionedRegistry,
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

    /// Format used to render the report. Predefined formats are: ansi, json
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
    let mut output = PathBuf::from("output");

    // Check if output is disabled (--output=none)
    let no_output = args
        .output
        .as_ref()
        .map(|p| p.to_str() == Some("none"))
        .unwrap_or(false);

    let output_directive = if let Some(path_buf) = &args.output {
        if no_output {
            OutputDirective::Stdout // Will be ignored when no_output is true
        } else {
            output = path_buf.clone();
            OutputDirective::File
        }
    } else {
        OutputDirective::Stdout
    };

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
    let mut live_checker = LiveChecker::new(registry, default_advisors());

    let rego_advisor = RegoAdvisor::new(
        &live_checker,
        &args.advice_policies,
        &args.advice_preprocessor,
    )?;
    live_checker.add_advisor(Box::new(rego_advisor));

    // Prepare the template engine (only if output is enabled)
    let engine = if !no_output {
        let loader = EmbeddedFileLoader::try_new(
            &DEFAULT_LIVE_CHECK_TEMPLATES,
            args.templates.clone(),
            &args.format,
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
                error: format!("Failed to load `defaults/live_check_templates/weaver.yaml`: {e}"),
            })
        })?;
        Some(TemplateEngine::try_new(config, loader, Params::default())?)
    } else {
        None
    };

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

    let report_mode = if let OutputDirective::File = output_directive {
        // File output forces report mode
        true
    } else {
        // This flag is not set by default. The user can set it to disable streaming output
        // and force report mode.
        args.no_stream
    };

    let mut stats = if args.no_stats {
        LiveCheckStatistics::new_disabled(&live_checker.registry)
    } else {
        LiveCheckStatistics::new(&live_checker.registry)
    };
    let mut samples = Vec::new();
    for mut sample in ingester {
        sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone())?;

        if report_mode && !args.no_stats {
            samples.push(sample);
        } else if !no_output {
            // Only generate output if template rendering is enabled
            if let Some(ref engine) = engine {
                engine
                    .generate(&sample, output.as_path(), &output_directive)
                    .map_err(|e| {
                        DiagnosticMessages::from(Error::OutputError {
                            error: e.to_string(),
                        })
                    })?;
            }
        }
    }

    // Only finalize and output stats if stats are enabled and output is enabled
    if !args.no_stats && !no_output {
        stats.finalize();
        // Set the exit_code to a non-zero code if there are any violations
        if stats.has_violations() {
            exit_code = 1;
        }

        if report_mode {
            // Package into a report
            let report = LiveCheckReport {
                statistics: stats,
                samples,
            };
            if let Some(ref engine) = engine {
                engine
                    .generate(&report, output.as_path(), &output_directive)
                    .map_err(|e| {
                        DiagnosticMessages::from(Error::OutputError {
                            error: e.to_string(),
                        })
                    })?;
            }
        } else {
            // Output the stats
            if let Some(ref engine) = engine {
                engine
                    .generate(&stats, output.as_path(), &output_directive)
                    .map_err(|e| {
                        DiagnosticMessages::from(Error::OutputError {
                            error: e.to_string(),
                        })
                    })?;
            }
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
