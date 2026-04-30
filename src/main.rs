//! Weaver CLI tool.

#![allow(clippy::print_stdout)]

use std::path::PathBuf;

use clap::CommandFactory;
use clap::{Args, Parser};
use clap_complete::{generate, Shell};
use log::info;
use std::io;
use std::io::Write;

use registry::{resolve_weaver_config, semconv_registry};
use weaver_common::diagnostic::{enable_future_mode, DiagnosticMessages};
use weaver_common::log_error;
use weaver_forge::{OutputProcessor, OutputTarget};

use crate::cli::{Cli, Commands};
use crate::diagnostic::DEFAULT_DIAGNOSTIC_TEMPLATES;

mod cli;
mod diagnostic;
mod registry;
mod serve;
mod weaver;

/// Default value for `--diagnostic-format`.
pub(crate) const DEFAULT_DIAGNOSTIC_FORMAT: &str = "ansi";
/// Default value for `--diagnostic-template`.
pub(crate) const DEFAULT_DIAGNOSTIC_TEMPLATE: &str = "diagnostic_templates";

/// Set of parameters used to specify the diagnostic format.
#[derive(Args, Debug, Clone)]
pub(crate) struct DiagnosticArgs {
    /// Format used to render the diagnostic messages. Predefined formats are: ansi, json,
    /// gh_workflow_command.
    #[arg(long, default_value = DEFAULT_DIAGNOSTIC_FORMAT)]
    pub(crate) diagnostic_format: String,

    /// Path to the directory where the diagnostic templates are located.
    #[arg(long, default_value = DEFAULT_DIAGNOSTIC_TEMPLATE)]
    pub(crate) diagnostic_template: PathBuf,

    /// Send the output to stdout instead of stderr.
    #[arg(long)]
    pub(crate) diagnostic_stdout: bool,
}

impl Default for DiagnosticArgs {
    fn default() -> Self {
        Self {
            diagnostic_format: DEFAULT_DIAGNOSTIC_FORMAT.to_owned(),
            diagnostic_template: PathBuf::from(DEFAULT_DIAGNOSTIC_TEMPLATE),
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
        let level = if cli.debug == 1 {
            log::LevelFilter::Debug
        } else if cli.debug >= 2 {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        };
        env_logger::builder()
            .filter(None, level)
            .format(|buf, record| writeln!(buf, "{}", record.args()))
            .init();

        log::debug!("Debug is set to {}", cli.debug);
    }

    // Force the `miette` context to 5 lines.
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .context_lines(5)
                .build(),
        )
    }))
    .expect("Failed to set miette hook");

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
    if cli.allow_git_credentials {
        weaver_common::vdir::enable_git_credentials();
    }
    // Resolve `.weaver.toml` (global `--config` overrides cwd discovery) and
    // build the HTTP auth resolver once for the whole invocation.
    let weaver_config = match resolve_weaver_config(cli.config.as_deref()) {
        Ok(wc) => wc,
        Err(diag) => {
            return process_diagnostics(CmdResult::new(Err(diag), Some(DiagnosticArgs::default())));
        }
    };
    let cfg = weaver_config.as_ref();
    let auth = registry::auth_resolver_from_config(cfg);
    let cmd_result = match &cli.command {
        Some(Commands::Registry(params)) => semconv_registry(params, cfg, &auth),
        Some(Commands::Diagnostic(params)) => diagnostic::diagnostic(params),
        Some(Commands::Serve(params)) => serve::command(params, &auth),
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
        Some(Commands::MarkdownHelp) => {
            // Generate: cargo run -- --quiet markdown-help > docs/usage.md
            clap_markdown::print_help_markdown::<Cli>();
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
    let target = if diagnostic_args.diagnostic_stdout {
        OutputTarget::Stdout
    } else {
        OutputTarget::Stderr
    };
    let mut output = OutputProcessor::new(
        &diagnostic_args.diagnostic_format,
        "errors",
        Some(&DEFAULT_DIAGNOSTIC_TEMPLATES),
        Some(diagnostic_args.diagnostic_template.clone()),
        target,
    )?;
    output.generate(diagnostic_messages)
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
                    "Failed to render the diagnostic messages. Error: {e}"
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
                        "Failed to render the diagnostic messages. Error: {e}"
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
