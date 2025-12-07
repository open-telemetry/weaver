// SPDX-License-Identifier: Apache-2.0

//! Web API server for exploring the semantic convention registry.

use std::net::SocketAddr;

use clap::Args;
use log::info;
use weaver_common::diagnostic::DiagnosticMessages;

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::{CmdResult, DiagnosticArgs, ExitDirectives};

mod handlers;
mod search;
mod server;
mod types;

pub use server::run_server;

/// Parameters for the `weaver serve` command.
#[derive(Debug, Args)]
pub struct ServeCommand {
    /// Parameters to specify the semantic convention registry.
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Parameters to specify the policy engine.
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Address to bind the server to.
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Execute the `weaver serve` command.
pub fn command(args: &ServeCommand) -> CmdResult {
    CmdResult::new(run_serve(args), Some(args.diagnostic.clone()))
}

fn run_serve(args: &ServeCommand) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Loading registry from `{}`", args.registry.registry);

    let mut diag_msgs = DiagnosticMessages::empty();

    // Create a weaver engine and load/resolve the registry using V2 schema
    let weaver = crate::weaver::WeaverEngine::new(&args.registry, &args.policy);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    // Convert to V2 ForgeResolvedRegistry
    let resolved_v2 = resolved.try_into_v2()?;
    let forge_registry = resolved_v2.into_template_schema();

    if !diag_msgs.is_empty() {
        // Log warnings but continue
        diag_msgs.log();
    }

    info!("Registry loaded successfully");
    info!(
        "Found {} attributes, {} metrics, {} spans, {} events, {} entities",
        forge_registry.attributes.len(),
        forge_registry.signals.metrics.len(),
        forge_registry.signals.spans.len(),
        forge_registry.signals.events.len(),
        forge_registry.signals.entities.len(),
    );
    info!("Starting server on {}", args.bind);

    // Run the async server using tokio runtime
    tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async { run_server(args.bind, forge_registry).await })
        .map_err(DiagnosticMessages::from_error)?;

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}
