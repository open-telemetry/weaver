// SPDX-License-Identifier: Apache-2.0

//! Perform a health check on sample telemetry by comparing it to a semantic convention registry.

use std::path::PathBuf;

use clap::Args;
use include_dir::{include_dir, Dir};

use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_health::attribute_advice::{
    Advisor, Advisory, DeprecatedAdvisor, EnumAdvisor, NameFormatAdvisor, NamespaceAdvisor,
    StabilityAdvisor, TypeAdvisor,
};
use weaver_health::attribute_file_ingester::AttributeFileIngester;
use weaver_health::attribute_health::{AttributeHealthChecker, HealthStatistics};
use weaver_health::attribute_json_file_ingester::AttributeJsonFileIngester;
use weaver_health::attribute_json_stdin_ingester::AttributeJsonStdinIngester;
use weaver_health::attribute_stdin_ingester::AttributeStdinIngester;
use weaver_health::{Error, Ingester};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

use super::otlp::attribute_otlp_ingester::AttributeOtlpIngester;

/// Embedded default health templates
pub(crate) static DEFAULT_HEALTH_TEMPLATES: Dir<'_> = include_dir!("defaults/health_templates");

/// The type of ingester to use
#[derive(Debug, Clone)]
enum IngesterType {
    AttributeFile,
    AttributeStdin,
    AttributeJsonFile,
    AttributeJsonStdin,
    AttributeOtlp,
    GroupFile,
}

impl From<String> for IngesterType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "attribute_file" | "AF" | "af" => IngesterType::AttributeFile,
            "attribute_stdin" | "AS" | "as" => IngesterType::AttributeStdin,
            "attribute_json_file" | "AJF" | "ajf" => IngesterType::AttributeJsonFile,
            "attribute_json_stdin" | "AJS" | "ajs" => IngesterType::AttributeJsonStdin,
            "attribute_otlp" | "AO" | "ao" => IngesterType::AttributeOtlp,
            "group_file" | "GF" | "gf" => IngesterType::GroupFile,
            _ => IngesterType::AttributeFile,
        }
    }
}

/// Parameters for the `registry health` sub-command
#[derive(Debug, Args)]
pub struct RegistryHealthArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// The path to the file containing sample telemetry data.
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Ingester type
    ///
    /// - `attribute_file_ingester` or `AFI` or `afi` (default)
    #[arg(short = 'g', long)]
    ingester: IngesterType,

    /// Format used to render the report. Predefined formats are: ansi, json
    #[arg(long, default_value = "ansi")]
    format: String,

    /// Path to the directory where the templates are located.
    #[arg(long, default_value = "health_templates")]
    templates: PathBuf,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the report is printed to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    pub otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317")]
    pub otlp_grpc_port: u16,

    /// Port used by the HTTP admin port (endpoints: /stop).
    #[clap(long, default_value = "4320")]
    pub admin_port: u16,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long, default_value = "10")]
    pub inactivity_timeout: u64,
}

/// Perform a health check on sample data by comparing it to a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone + 'static,
    args: &RegistryHealthArgs,
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

    logger.log("Weaver Registry Health");

    // Prepare the registry
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!(
        "Performing health check with registry `{}`",
        args.registry.registry
    ));

    // Create the health checker with advisors
    let advisors: Vec<Box<dyn Advisor>> = vec![
        Box::new(DeprecatedAdvisor),
        Box::new(NameFormatAdvisor::default()),
        Box::new(StabilityAdvisor),
        Box::new(TypeAdvisor),
        Box::new(EnumAdvisor),
    ];

    let mut health_checker = AttributeHealthChecker::new(registry, advisors);
    let namespace_advisor = NamespaceAdvisor::new('.', &health_checker);
    health_checker.add_advisor(Box::new(namespace_advisor));

    // Prepare the template engine
    let loader = EmbeddedFileLoader::try_new(
        &DEFAULT_HEALTH_TEMPLATES,
        args.templates.clone(),
        &args.format,
    )
    .map_err(|e| {
        DiagnosticMessages::from(Error::OutputError {
            error: format!(
                "Failed to create the embedded file loader for the health templates: {}",
                e
            ),
        })
    })?;
    let config = WeaverConfig::try_from_loader(&loader).map_err(|e| {
        DiagnosticMessages::from(Error::OutputError {
            error: format!(
                "Failed to load `defaults/health_templates/weaver.yaml`: {}",
                e
            ),
        })
    })?;
    let engine = TemplateEngine::new(config, loader, Params::default());

    // Prepare the ingester
    let ingester = match args.ingester {
        IngesterType::AttributeFile => {
            let path = match &args.input {
                Some(p) => Ok(p),
                None => Err(Error::IngestError {
                    error: "No input path provided".to_owned(),
                }),
            }?;
            AttributeFileIngester::new(path).ingest(logger.clone())?
        }

        IngesterType::AttributeStdin => AttributeStdinIngester::new().ingest(logger.clone())?,

        IngesterType::AttributeJsonFile => {
            let path = match &args.input {
                Some(p) => Ok(p),
                None => Err(Error::IngestError {
                    error: "No input path provided".to_owned(),
                }),
            }?;
            AttributeJsonFileIngester::new(path).ingest(logger.clone())?
        }

        IngesterType::AttributeJsonStdin => {
            AttributeJsonStdinIngester::new().ingest(logger.clone())?
        }

        IngesterType::AttributeOtlp => (AttributeOtlpIngester {
            otlp_grpc_address: args.otlp_grpc_address.clone(),
            otlp_grpc_port: args.otlp_grpc_port,
            admin_port: args.admin_port,
            inactivity_timeout: args.inactivity_timeout,
        })
        .ingest(logger.clone())?,

        IngesterType::GroupFile => {
            return Err(DiagnosticMessages::from(Error::OutputError {
                error: "Invalid ingester type".to_owned(),
            }))
        }
    };

    // If this is a stream - process the attributes one by one
    // TODO this should be a setting not a format and it must force STDOUT output
    if args.format == "stream" {
        let mut stats = HealthStatistics::default();
        for attribute in ingester {
            let health_attribute = health_checker.create_health_attribute(&attribute);
            stats.update(&health_attribute);
            // Set the exit_code to a non-zero code if there are any violations
            if let Some(Advisory::Violation) = health_attribute.highest_advisory {
                exit_code = 1;
            }
            engine
                .generate(
                    logger.clone(),
                    &health_attribute,
                    output.as_path(),
                    &output_directive,
                )
                .map_err(|e| {
                    DiagnosticMessages::from(Error::OutputError {
                        error: e.to_string(),
                    })
                })?;
        }
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
        let results = health_checker.check_attributes(attributes);
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
        "Performed health check for registry `{}`",
        args.registry.registry
    ));

    // Only print warnings when the output is to a file
    if !diag_msgs.is_empty() && args.output.is_some() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code,
        quiet_mode: args.output.is_none(),
    })
}
