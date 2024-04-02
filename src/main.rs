//! Weaver CLI tool.

#![allow(clippy::print_stdout)]

use clap::Parser;

use registry::semconv_registry;
use weaver_logger::quiet::QuietLogger;
use weaver_logger::{ConsoleLogger, Logger};

use crate::cli::{Cli, Commands};
#[cfg(feature = "experimental")]
use crate::gen_client::command_gen_client;
#[cfg(feature = "experimental")]
use crate::resolve::command_resolve;

mod cli;
#[cfg(feature = "experimental")]
mod gen_client;
#[cfg(feature = "experimental")]
mod languages;
mod registry;
#[cfg(feature = "experimental")]
mod resolve;
#[cfg(feature = "experimental")]
mod search;

#[no_coverage]
fn main() {
    let cli = Cli::parse();

    let start = std::time::Instant::now();
    if cli.quiet {
        let log = QuietLogger::new();
        run_command(&cli, log);
    } else {
        let log = ConsoleLogger::new(cli.debug);
        run_command(&cli, log);
    };
    let elapsed = start.elapsed();
    println!("Total execution time: {:?}s", elapsed.as_secs_f64());
}

#[no_coverage]
fn run_command(cli: &Cli, log: impl Logger + Sync + Clone) {
    match &cli.command {
        #[cfg(feature = "experimental")]
        Some(Commands::Resolve(params)) => {
            command_resolve(log, params);
        }
        #[cfg(feature = "experimental")]
        Some(Commands::GenClient(params)) => {
            command_gen_client(log, params);
        }
        #[cfg(feature = "experimental")]
        Some(Commands::Languages(params)) => {
            languages::command_languages(log, params);
        }
        #[cfg(feature = "experimental")]
        Some(Commands::Search(params)) => {
            search::command_search(log, params);
        }
        Some(Commands::Registry(params)) => {
            semconv_registry(log, params);
        }
        None => {}
    }
}
