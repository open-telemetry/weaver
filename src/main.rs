use clap::Parser;

use weaver_logger::ConsoleLogger;

use crate::cli::{Cli, Commands};
use crate::gen_client::command_gen_client;
use crate::resolve::command_resolve;

mod cli;
mod gen_client;
mod languages;
mod resolve;
mod search;

fn main() {
    let cli = Cli::parse();
    let log = ConsoleLogger::new(cli.debug);

    match &cli.command {
        Some(Commands::Resolve(params)) => {
            command_resolve(log, params);
        }
        Some(Commands::GenClient(params)) => {
            command_gen_client(log, params);
        }
        Some(Commands::Languages(params)) => {
            languages::command_languages(log, params);
        }
        Some(Commands::Search(params)) => {
            search::command_search(log, params);
        }
        None => {}
    }
}
