// SPDX-License-Identifier: Apache-2.0

//! Manage command line arguments

#[cfg(feature = "experimental")]
use crate::gen_client::GenClientCommand;
#[cfg(feature = "experimental")]
use crate::languages::LanguagesParams;
use crate::registry::RegistryCommand;
#[cfg(feature = "experimental")]
use crate::resolve::ResolveCommand;
#[cfg(feature = "experimental")]
use crate::search::SearchCommand;
use clap::{Parser, Subcommand};

/// Command line arguments.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    /// Turn the quiet mode on (i.e., minimal output)
    #[arg(short, long)]
    pub quiet: bool,

    /// List of supported commands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Supported commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Resolve a semantic convention registry or a telemetry schema
    #[cfg(feature = "experimental")]
    Resolve(ResolveCommand),
    /// Generate a client SDK or client API
    #[cfg(feature = "experimental")]
    GenClient(GenClientCommand),
    /// List all supported languages
    #[cfg(feature = "experimental")]
    Languages(LanguagesParams),
    /// Search in a semantic convention registry or a telemetry schema
    #[cfg(feature = "experimental")]
    Search(SearchCommand),
    /// Manage Semantic Convention Registry
    Registry(RegistryCommand),
}
