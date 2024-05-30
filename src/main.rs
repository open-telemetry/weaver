//! Weaver CLI tool.

#![allow(clippy::print_stdout)]

use std::path::PathBuf;

use clap::{Args, Parser};

use registry::semconv_registry;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::quiet::QuietLogger;
use weaver_common::{ConsoleLogger, Logger};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};

use crate::cli::{Cli, Commands};
use crate::diagnostic::DEFAULT_DIAGNOSTIC_TEMPLATES;
use crate::schema::telemetry_schema;

mod cli;
mod diagnostic;
mod format;
mod registry;
mod schema;
mod util;

/// Set of parameters used to specify the diagnostic format.
#[derive(Args, Debug, Clone)]
pub(crate) struct DiagnosticArgs {
    /// Format used to render the diagnostic messages. Predefined formats are: ansi, json,
    /// gh_workflow_command.
    #[arg(long, default_value = "ansi")]
    pub(crate) diagnostic_format: String,

    /// Path to the directory where the diagnostic templates are located.
    #[arg(long, default_value = "diagnostic_templates")]
    pub(crate) diagnostic_template: PathBuf,
}

impl Default for DiagnosticArgs {
    fn default() -> Self {
        Self {
            diagnostic_format: "ansi".to_owned(),
            diagnostic_template: PathBuf::from("diagnostic_templates"),
        }
    }
}

/// Result of a command execution.
#[derive(Debug)]
pub(crate) struct CmdResult {
    pub(crate) command_result: Result<(), DiagnosticMessages>,
    pub(crate) diagnostic_args: Option<DiagnosticArgs>,
}

impl CmdResult {
    /// Create a new command result.
    pub(crate) fn new(
        command_result: Result<(), DiagnosticMessages>,
        diagnostic_args: Option<DiagnosticArgs>,
    ) -> Self {
        Self {
            command_result,
            diagnostic_args,
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let cli = Cli::parse();

    let start = std::time::Instant::now();
    let exit_code = if cli.quiet {
        let log = QuietLogger::new();
        run_command(&cli, log)
    } else {
        let log = ConsoleLogger::new(cli.debug);
        run_command(&cli, log)
    };

    if !cli.quiet {
        let elapsed = start.elapsed();
        println!("Total execution time: {:?}s", elapsed.as_secs_f64());
    }

    // Exit the process with the exit code provided by the `run_command` function.
    #[allow(clippy::exit)]
    std::process::exit(exit_code);
}

/// Run the command specified by the CLI arguments and return the exit code.
#[cfg(not(tarpaulin_include))]
fn run_command(cli: &Cli, log: impl Logger + Sync + Clone) -> i32 {
    let cmd_result = match &cli.command {
        Some(Commands::Registry(params)) => semconv_registry(log.clone(), params),
        Some(Commands::Schema(params)) => telemetry_schema(log.clone(), params),
        Some(Commands::Diagnostic(params)) => diagnostic::diagnostic(log.clone(), params),
        None => return 0,
    };

    process_diagnostics(cmd_result, log.clone())
}

/// Render the diagnostic messages based on the diagnostic configuration and return the exit code
/// based on the diagnostic messages.
fn process_diagnostics(cmd_result: CmdResult, logger: impl Logger + Sync + Clone) -> i32 {
    let diagnostic_args = if let Some(diagnostic_args) = cmd_result.diagnostic_args {
        diagnostic_args
    } else {
        DiagnosticArgs::default() // Default diagnostic arguments;
    };

    if let Err(diagnostic_messages) = cmd_result.command_result {
        let loader = EmbeddedFileLoader::try_new(
            &DEFAULT_DIAGNOSTIC_TEMPLATES,
            diagnostic_args.diagnostic_template,
            &diagnostic_args.diagnostic_format,
        )
        .expect("Failed to create the embedded file loader for the diagnostic templates");
        match TemplateEngine::try_new(loader) {
            Ok(engine) => {
                match engine.generate(
                    logger.clone(),
                    &diagnostic_messages,
                    PathBuf::new().as_path(),
                    &OutputDirective::Stdout,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        logger.error(&format!(
                            "Failed to render the diagnostic messages. Error: {}",
                            e
                        ));
                        return 1;
                    }
                }
            }
            Err(e) => {
                logger.error(&format!("Failed to create the template engine to render the diagnostic messages. Error: {}", e));
                return 1;
            }
        }
        return if diagnostic_messages.has_error() {
            1
        } else {
            0
        };
    }

    // Return 0 if there are no diagnostic messages
    0
}
