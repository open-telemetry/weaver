//! Weaver CLI tool.

#![allow(clippy::print_stdout)]

use std::path::PathBuf;

use clap::CommandFactory;
use clap::{Args, Parser};
use clap_complete::{generate, Shell};
use log::info;
use std::io;
use std::io::Write;

use registry::semconv_registry;
use weaver_common::diagnostic::{enable_future_mode, DiagnosticMessages};
use weaver_common::log_error;
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

    /// Send the output to stdout instead of stderr.
    #[arg(long)]
    pub(crate) diagnostic_stdout: bool,
}

impl Default for DiagnosticArgs {
    fn default() -> Self {
        Self {
            diagnostic_format: "ansi".to_owned(),
            diagnostic_template: PathBuf::from("diagnostic_templates"),
            diagnostic_stdout: false,
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
    /// Non-error diagnostic messages to log out.
    warnings: Option<DiagnosticMessages>,
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

    if !cli.quiet {
        // Initialize the logger
        env_logger::builder()
            .filter(None, log::LevelFilter::Info)
            .format(|buf, record| writeln!(buf, "{}", record.args()))
            .init();
    }

    let exit_directives = run_command(&cli);

    let elapsed = start.elapsed();
    info!("\nTotal execution time: {:?}s", elapsed.as_secs_f64());

    // Exit the process with the exit code provided by the `run_command` function.
    #[allow(clippy::exit)]
    std::process::exit(exit_directives.exit_code);
}

/// Run the command specified by the CLI arguments and return the exit directives.
fn run_command(cli: &Cli) -> ExitDirectives {
    if cli.future {
        enable_future_mode();
    }
    let cmd_result = match &cli.command {
        Some(Commands::Registry(params)) => semconv_registry(params),
        Some(Commands::Diagnostic(params)) => diagnostic::diagnostic(params),
        Some(Commands::Completion(completions)) => {
            if let Err(e) = generate_completion(&completions.shell, &completions.completion_file) {
                log_error(&e);
                return ExitDirectives {
                    exit_code: 1,
                    warnings: None,
                };
            }
            return ExitDirectives {
                exit_code: 0,
                warnings: None,
            };
        }
        None => {
            return ExitDirectives {
                exit_code: 0,
                warnings: None,
            }
        }
    };

    process_diagnostics(cmd_result)
}

fn print_diagnostics(
    diagnostic_args: &DiagnosticArgs,
    diagnostic_messages: &DiagnosticMessages,
) -> Result<(), weaver_forge::error::Error> {
    let loader = EmbeddedFileLoader::try_new(
        &DEFAULT_DIAGNOSTIC_TEMPLATES,
        diagnostic_args.diagnostic_template.clone(),
        &diagnostic_args.diagnostic_format,
    )
    .expect("Failed to create the embedded file loader for the diagnostic templates");
    let config = WeaverConfig::try_from_loader(&loader)
        .expect("Failed to load `defaults/diagnostic_templates/weaver.yaml`");
    let engine = TemplateEngine::new(config, loader, Params::default());
    let output_directive = if diagnostic_args.diagnostic_stdout {
        OutputDirective::Stdout
    } else {
        OutputDirective::Stderr
    };
    engine.generate(
        diagnostic_messages,
        PathBuf::new().as_path(),
        &output_directive,
    )
}

/// Render the diagnostic messages based on the diagnostic configuration and return the exit
/// directives based on the diagnostic messages and the CmdResult quiet mode.
fn process_diagnostics(cmd_result: CmdResult) -> ExitDirectives {
    let diagnostic_args = cmd_result.diagnostic_args.unwrap_or_default();
    let mut exit_directives = if let Ok(exit_directives) = &cmd_result.command_result {
        exit_directives.clone()
    } else {
        ExitDirectives {
            exit_code: 0,
            warnings: None,
        }
    };

    if let Err(diagnostic_messages) = cmd_result.command_result {
        match print_diagnostics(&diagnostic_args, &diagnostic_messages) {
            Ok(_) => {}
            Err(e) => {
                log_error(format!(
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
    } else if let Some(ref warnings) = exit_directives.warnings {
        if !warnings.is_empty() {
            match print_diagnostics(&diagnostic_args, warnings) {
                Ok(_) => {}
                Err(e) => {
                    log_error(format!(
                        "Failed to render the diagnostic messages. Error: {}",
                        e
                    ));
                }
            }
        }
    }

    exit_directives
}

fn generate_completion(shell: &Shell, output_file: &Option<PathBuf>) -> Result<(), String> {
    let mut app = Cli::command();
    let bin_name = app.get_name().to_owned();

    if let Some(file_path) = output_file {
        let file = std::fs::File::create(file_path)
            .map_err(|e| format!("Failed to create file '{}': {}", file_path.display(), e))?;
        generate(*shell, &mut app, bin_name, &mut io::BufWriter::new(file));
    } else {
        // Default to writing to STDOUT
        generate(*shell, &mut app, bin_name, &mut io::stdout());
    }

    Ok(())
}
