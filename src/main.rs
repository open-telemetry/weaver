use clap::Parser;

use registry::semconv_registry;
use weaver_logger::ConsoleLogger;

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

fn main() {
    let cli = Cli::parse();
    let log = ConsoleLogger::new(cli.debug);

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
