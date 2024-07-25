//! Weaver CLI tool.

#![allow(clippy::print_stdout)]

use std::path::PathBuf;

use clap::{Args, Parser};

use registry::semconv_registry;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::quiet::QuietLogger;
use weaver_common::{ConsoleLogger, Logger};
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::EmbeddedFileLoader;
use weaver_forge::{OutputDirective, TemplateEngine};

use crate::cli::{Cli, Commands};
use crate::diagnostic::DEFAULT_DIAGNOSTIC_TEMPLATES;

mod cli;
mod diagnostic;
mod format;
mod registry;
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
    pub(crate) command_result: Result<ExitDirectives, DiagnosticMessages>,
    pub(crate) diagnostic_args: Option<DiagnosticArgs>,
}

/// Exit directives.
#[derive(Debug, Clone)]
pub(crate) struct ExitDirectives {
    /// Exit code.
    exit_code: i32,
    /// Quiet mode.
    quiet_mode: bool,
}

impl CmdResult {
    /// Create a new command result.
    pub(crate) fn new(
        command_result: Result<ExitDirectives, DiagnosticMessages>,
        diagnostic_args: Option<DiagnosticArgs>,
    ) -> Self {
        Self {
            command_result,
            diagnostic_args,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let start = std::time::Instant::now();
    let exit_directives = if cli.quiet {
        let log = QuietLogger::new();
        run_command(&cli, log)
    } else {
        let log = ConsoleLogger::new(cli.debug);
        run_command(&cli, log)
    };

    if !cli.quiet && !exit_directives.quiet_mode {
        let elapsed = start.elapsed();
        println!("\nTotal execution time: {:?}s", elapsed.as_secs_f64());
    }

    // Exit the process with the exit code provided by the `run_command` function.
    #[allow(clippy::exit)]
    std::process::exit(exit_directives.exit_code);
}

/// Run the command specified by the CLI arguments and return the exit directives.
fn run_command(cli: &Cli, log: impl Logger + Sync + Clone) -> ExitDirectives {
    let cmd_result = match &cli.command {
        Some(Commands::Registry(params)) => semconv_registry(log.clone(), params),
        Some(Commands::Diagnostic(params)) => diagnostic::diagnostic(log.clone(), params),
        None => {
            return ExitDirectives {
                exit_code: 0,
                quiet_mode: false,
            }
        }
    };

    process_diagnostics(cmd_result, log.clone())
}

/// Render the diagnostic messages based on the diagnostic configuration and return the exit
/// directives based on the diagnostic messages and the CmdResult quiet mode.
fn process_diagnostics(
    cmd_result: CmdResult,
    logger: impl Logger + Sync + Clone,
) -> ExitDirectives {
    let diagnostic_args = cmd_result.diagnostic_args.unwrap_or_default();
    let mut exit_directives = if let Ok(exit_directives) = &cmd_result.command_result {
        exit_directives.clone()
    } else {
        ExitDirectives {
            exit_code: 0,
            quiet_mode: false,
        }
    };

    if let Err(diagnostic_messages) = cmd_result.command_result {
        let loader = EmbeddedFileLoader::try_new(
            &DEFAULT_DIAGNOSTIC_TEMPLATES,
            diagnostic_args.diagnostic_template,
            &diagnostic_args.diagnostic_format,
        )
        .expect("Failed to create the embedded file loader for the diagnostic templates");
        let config = WeaverConfig::try_from_loader(&loader)
            .expect("Failed to load `defaults/diagnostic_templates/weaver.yaml`");
        let engine = TemplateEngine::new(config, loader, Params::default());
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
                exit_directives.exit_code = 1;
                return exit_directives;
            }
        }
        if diagnostic_messages.has_error() {
            exit_directives.exit_code = 1;
        }
    }

    exit_directives
}
