// SPDX-License-Identifier: Apache-2.0

//! Manage command line arguments

use crate::gen_client::GenClientCommand;
use crate::languages::LanguagesParams;
use crate::resolve::ResolveCommand;
use crate::search::SearchCommand;
use clap::{Parser, Subcommand};
use crate::check::CheckCommand;

/// Command line arguments.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    /// List of supported commands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Supported commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Resolve a semantic convention registry or a telemetry schema
    Resolve(ResolveCommand),
    /// Generate a client SDK or client API
    GenClient(GenClientCommand),
    /// List all supported languages
    Languages(LanguagesParams),
    /// Search in a semantic convention registry or a telemetry schema
    Search(SearchCommand),
    /// Check a semantic convention registry or a telemetry schema
    Check(CheckCommand),
}
