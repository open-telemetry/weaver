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
    Advisor, CorrectCaseAdvisor, DeprecatedAdvisor, StabilityAdvisor,
};
use weaver_health::attribute_file_ingester::AttributeFileIngester;
use weaver_health::attribute_health::AttributeHealthChecker;
use weaver_health::attribute_stdin_ingester::AttributeStdinIngester;
use weaver_health::{Error, Ingester};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

/// Embedded default health templates
pub(crate) static DEFAULT_HEALTH_TEMPLATES: Dir<'_> = include_dir!("defaults/health_templates");

/// The type of ingester to use
#[derive(Debug, Clone)]
enum IngesterType {
    AttributeFile,
    AttributeStdin,
}

impl From<String> for IngesterType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "attribute_file" | "AF" | "af" => IngesterType::AttributeFile,
            "attribute_stdin" | "AS" | "as" => IngesterType::AttributeStdin,
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
}

/// Perform a health check on sample data by comparing it to a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryHealthArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut output = PathBuf::from("output");
    let output_directive = if let Some(path_buf) = &args.output {
        output = path_buf.clone();
        OutputDirective::File
    } else {
        logger.mute();
        OutputDirective::Stdout
    };

    logger.log("Weaver Registry Health");
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!(
        "Performing health check with registry `{}`",
        args.registry.registry
    ));

    let attributes = match args.ingester {
        IngesterType::AttributeFile => {
            let path = match &args.input {
                Some(p) => Ok(p),
                None => Err(Error::IngestError {
                    error: "No input path provided".to_owned(),
                }),
            }?;

            let ingester = AttributeFileIngester::new();
            ingester.ingest(path)?
        }
        IngesterType::AttributeStdin => {
            let ingester = AttributeStdinIngester::new();
            ingester.ingest(())?
        }
    };

    let advisors: Vec<Box<dyn Advisor>> = vec![
        Box::new(DeprecatedAdvisor),
        Box::new(CorrectCaseAdvisor),
        Box::new(StabilityAdvisor),
    ];

    let health_checker = AttributeHealthChecker::new(attributes, registry, advisors);

    let results = health_checker.check_attributes();

    logger.success(&format!(
        "Performed health check for registry `{}`",
        args.registry.registry
    ));

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

    match engine.generate(
        logger.clone(),
        &results,
        output.as_path(),
        &output_directive,
    ) {
        Ok(_) => {}
        Err(e) => {
            return Err(DiagnosticMessages::from(Error::OutputError {
                error: e.to_string(),
            }));
        }
    }

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: args.output.is_none(),
    })
}
