// SPDX-License-Identifier: Apache-2.0

//! MCP server subcommand for the registry.
//!
//! This module provides the `weaver registry mcp` subcommand that runs an MCP
//! (Model Context Protocol) server exposing the semantic conventions registry
//! to LLMs.

use std::path::PathBuf;

use clap::Args;
use log::info;

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::diagnostic::DiagnosticMessages;

/// Parameters for the `registry mcp` subcommand.
#[derive(Debug, Args)]
pub struct RegistryMcpArgs {
    /// Registry arguments.
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Diagnostic arguments.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Advice policies directory. Set this to override the default policies.
    #[arg(long)]
    pub advice_policies: Option<PathBuf>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    ///
    /// Rego policies are run for each sample as it arrives. The preprocessor
    /// can be used to create a new data structure that is more efficient for the rego policies
    /// versus processing the data for every sample.
    #[arg(long)]
    pub advice_preprocessor: Option<PathBuf>,
}

/// Run the MCP server for the semantic convention registry.
pub(crate) fn command(args: &RegistryMcpArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Loading semantic convention registry for MCP server");

    let mut diag_msgs = DiagnosticMessages::empty();

    // Create empty policy args (MCP server doesn't need policy checks)
    let policy_args = PolicyArgs {
        policies: Vec::new(),
        skip_policies: true,
        display_policy_coverage: false,
    };

    // Use WeaverEngine to load and resolve the registry (always use v2)
    let weaver = WeaverEngine::new(&args.registry, &policy_args);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    // Convert to V2 ForgeResolvedRegistry
    let resolved_v2 = resolved.try_into_v2()?;
    let forge_registry = resolved_v2.into_template_schema();

    info!("Starting MCP server (communicating over stdio)");
    info!("The server will run until stdin is closed.");

    // Build MCP config from command line args
    let config = weaver_mcp::McpConfig {
        advice_policies: args.advice_policies.clone(),
        advice_preprocessor: args.advice_preprocessor.clone(),
    };

    // Run the MCP server
    if let Err(e) = weaver_mcp::run_with_config(forge_registry, config) {
        return Err(DiagnosticMessages::from_error(e));
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}
