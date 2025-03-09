// SPDX-License-Identifier: Apache-2.0

//! Perform a health check on sample telemetry by comparing it to a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_health::attribute_advice::{Advisor, DeprecatedAdvisor, WrongCaseAdvisor};
use weaver_health::attribute_file_ingester::AttributeFileIngester;
use weaver_health::attribute_health::AttributeHealthChecker;
use weaver_health::{Error, Ingester};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

/// The type of ingester to use
#[derive(Debug, Clone)]
enum IngesterType {
    AttributeFileIngester,
}

impl From<String> for IngesterType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "attribute_file_ingester" | "AFI" | "afi" => IngesterType::AttributeFileIngester,
            _ => IngesterType::AttributeFileIngester,
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
    #[arg(short = 'i', long)]
    input_path: Option<PathBuf>,

    /// Ingester type
    ///
    /// - `attribute_file_ingester` or `AFI` or `afi` (default)
    #[arg(short = 'g', long)]
    ingester: IngesterType,
}

/// Perform a health check on sample data by comparing it to a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryHealthArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
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
        IngesterType::AttributeFileIngester => {
            let path = match &args.input_path {
                Some(p) => Ok(p),
                None => Err(Error::IngestError {
                    error: "No input path provided".to_owned(),
                }),
            }?;

            let ingester = AttributeFileIngester::new();
            ingester.ingest(path)?
        }
    };

    let advisors: Vec<Box<dyn Advisor>> =
        vec![Box::new(DeprecatedAdvisor), Box::new(WrongCaseAdvisor)];

    let health_checker = AttributeHealthChecker::new(attributes, registry, advisors);

    let results = health_checker.check_attributes();

    for result in results.iter() {
        logger.log(&format!("{:?}", result));
    }

    logger.success(&format!(
        "Performed health check for registry `{}`",
        args.registry.registry
    ));

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
