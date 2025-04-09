// SPDX-License-Identifier: Apache-2.0

//! Perform checks on sample telemetry by:
//! - Comparing it to a semantic convention registry.
//! - Running built-in and custom policies to provide advice on how to improve the telemetry.

use std::path::PathBuf;

use clap::Args;
use include_dir::{include_dir, Dir};

use weaver_checker::violation::AdviceLevel;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_live_check::attribute_advice::{
    Advisor, DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor,
};
use weaver_live_check::attribute_file_ingester::AttributeFileIngester;
use weaver_live_check::attribute_json_file_ingester::AttributeJsonFileIngester;
use weaver_live_check::attribute_json_stdin_ingester::AttributeJsonStdinIngester;
use weaver_live_check::attribute_live_check::AttributeLiveChecker;
use weaver_live_check::attribute_stdin_ingester::AttributeStdinIngester;
use weaver_live_check::{Error, Ingester, LiveCheckStatistics};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

use super::otlp::attribute_otlp_ingester::AttributeOtlpIngester;

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

/// The scope for the live check
#[derive(Debug, Clone)]
enum AdviceScope {
    Attributes,
    Signals,
}

impl From<String> for AdviceScope {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "signals" | "signal" | "s" => AdviceScope::Signals,
            _ => AdviceScope::Attributes,
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
    #[arg(long, default_value = "stdin")]
    input_source: InputSource,

    /// The format of the input telemetry. (Not required for OTLP). text | json
    #[arg(long, default_value = "text")]
    input_format: InputFormat,

    /// Whether to provide advice on attributes or whole signals. attributes | signals
    ///
    /// If set to `signals`, the advice is provided on the whole signal (e.g. span, event, etc.)
    /// If set to `attributes`, the advice is provided on each attribute.
    /// The scope must be set to match what is present in the input as well as the desired output.
    #[arg(long, default_value = "attributes")]
    advice_scope: AdviceScope,

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

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the report is printed to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317")]
    otlp_grpc_port: u16,

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
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryLiveCheckArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut exit_code = 0;
    let mut output = PathBuf::from("output");
    let output_directive = if let Some(path_buf) = &args.output {
        output = path_buf.clone();
        OutputDirective::File
    } else {
        logger.mute();
        OutputDirective::Stdout
    };

    logger.log("Weaver Registry Live Check");

    // Prepare the registry
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!(
        "Performing live check with registry `{}`",
        args.registry.registry
    ));

    // Create the live checker with advisors
    let mut live_checker = AttributeLiveChecker::new(registry, default_advisors());

    let rego_advisor = RegoAdvisor::new(
        &live_checker,
        &args.advice_policies,
        &args.advice_preprocessor,
    )?;
    live_checker.add_advisor(Box::new(rego_advisor));

    // Prepare the template engine
    let loader = EmbeddedFileLoader::try_new(
        &DEFAULT_LIVE_CHECK_TEMPLATES,
        args.templates.clone(),
        &args.format,
    )
    .map_err(|e| {
        DiagnosticMessages::from(Error::OutputError {
            error: format!(
                "Failed to create the embedded file loader for the live check templates: {}",
                e
            ),
        })
    })?;
    let config = WeaverConfig::try_from_loader(&loader).map_err(|e| {
        DiagnosticMessages::from(Error::OutputError {
            error: format!(
                "Failed to load `defaults/live_check_templates/weaver.yaml`: {}",
                e
            ),
        })
    })?;
    let engine = TemplateEngine::new(config, loader, Params::default());

    // Prepare the ingester
    let ingester = match (&args.input_source, &args.input_format) {
        (InputSource::File(path), InputFormat::Text) => {
            AttributeFileIngester::new(path).ingest(logger.clone())?
        }

        (InputSource::Stdin, InputFormat::Text) => {
            AttributeStdinIngester::new().ingest(logger.clone())?
        }

        (InputSource::File(path), InputFormat::Json) => {
            AttributeJsonFileIngester::new(path).ingest(logger.clone())?
        }

        (InputSource::Stdin, InputFormat::Json) => {
            AttributeJsonStdinIngester::new().ingest(logger.clone())?
        }

        (InputSource::Otlp, _) => (AttributeOtlpIngester {
            otlp_grpc_address: args.otlp_grpc_address.clone(),
            otlp_grpc_port: args.otlp_grpc_port,
            admin_port: args.admin_port,
            inactivity_timeout: args.inactivity_timeout,
        })
        .ingest(logger.clone())?,
    };

    // If this is a stream - process the attributes one by one
    // File output is not supported in stream mode
    let stream_mode = if let OutputDirective::File = output_directive {
        false
    } else {
        !args.no_stream
    };

    if stream_mode {
        let mut stats = LiveCheckStatistics::new(&live_checker.registry);
        for attribute in ingester {
            let live_check_attribute = live_checker.create_live_check_attribute(&attribute);
            stats.update(&live_check_attribute);
            // Set the exit_code to a non-zero code if there are any violations
            if let Some(AdviceLevel::Violation) = live_check_attribute.highest_advice_level {
                exit_code = 1;
            }
            engine
                .generate(
                    logger.clone(),
                    &live_check_attribute,
                    output.as_path(),
                    &output_directive,
                )
                .map_err(|e| {
                    DiagnosticMessages::from(Error::OutputError {
                        error: e.to_string(),
                    })
                })?;
        }
        stats.finalize();
        // Output the final statistics
        engine
            .generate(logger.clone(), &stats, output.as_path(), &output_directive)
            .map_err(|e| {
                DiagnosticMessages::from(Error::OutputError {
                    error: e.to_string(),
                })
            })?;
    } else {
        let attributes = ingester.collect::<Vec<_>>();
        let results = live_checker.check_attributes(attributes);
        // Set the exit_code to a non-zero code if there are any violations
        if results.has_violations() {
            exit_code = 1;
        }
        engine
            .generate(
                logger.clone(),
                &results,
                output.as_path(),
                &output_directive,
            )
            .map_err(|e| {
                DiagnosticMessages::from(Error::OutputError {
                    error: e.to_string(),
                })
            })?;
    }

    logger.success(&format!(
        "Performed live check for registry `{}`",
        args.registry.registry
    ));

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code,
        quiet_mode: args.output.is_none(),
    })
}
