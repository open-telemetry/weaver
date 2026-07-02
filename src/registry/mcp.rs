// SPDX-License-Identifier: Apache-2.0

//! MCP server subcommand for the registry.
//!
//! This module provides the `weaver registry mcp` subcommand that runs an MCP
//! (Model Context Protocol) server exposing the semantic conventions registry
//! to LLMs.

use std::path::PathBuf;

use clap::Args;
use log::info;

use crate::registry::{load_config, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_config::{WeaverCommand, WeaverConfig};
use weaver_macros::weaver_command;

/// Expose a semantic convention registry over the Model Context Protocol (MCP).
#[weaver_command(section = "mcp", no_policy)]
#[derive(Debug, Args, WeaverCommand)]
pub struct RegistryMcpArgs {
    /// Registry arguments.
    #[command(flatten)]
    #[shared(registry)]
    pub registry: RegistryArgs,

    /// Diagnostic arguments.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,

    /// Advice policies directory. Set this to override the default policies.
    #[arg(long)]
    #[config]
    pub advice_policies: Option<PathBuf>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    #[arg(long)]
    #[config]
    pub advice_preprocessor: Option<PathBuf>,

    /// Glob pattern pointing to additional JSON/YAML files to load into OPA rego data.
    /// Files are nested in OPA data using their relative path inside the glob base directory (e.g. schemas/user.json is loaded at data.user).
    #[arg(long)]
    #[config]
    pub advice_data: Option<String>,

    /// Namespace separator used in attribute keys. Defaults to ".".
    /// Used by namespace browsing and search token splitting.
    #[arg(long)]
    #[config(default = ".")]
    pub namespace_separator: Option<String>,
}

/// Run the MCP server for the semantic convention registry.
pub(crate) fn command(
    args: &RegistryMcpArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    info!("Loading semantic convention registry for MCP server");

    let mut diag_msgs = DiagnosticMessages::empty();

    // Use WeaverEngine to load and resolve the registry (always use v2)
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;

    // Convert to V2 ForgeResolvedRegistry
    let resolved_v2 = match resolved {
        crate::weaver::Resolved::V1(v) => v.try_into().map_err(DiagnosticMessages::from_error)?,
        crate::weaver::Resolved::V2(v) => v,
    };
    let forge_registry = resolved_v2.into_template_schema();

    info!("Starting MCP server (communicating over stdio)");
    info!("The server will run until stdin is closed.");

    // Build MCP config from effective config
    let mcp_config = weaver_mcp::McpConfig {
        advice_policies: cmd_config.config.advice_policies,
        advice_data: cmd_config.config.advice_data,
        advice_preprocessor: cmd_config.config.advice_preprocessor,
        namespace_separator: cmd_config.config.namespace_separator,
    };

    // Run the MCP server
    if let Err(e) = weaver_mcp::run_with_config(forge_registry, mcp_config) {
        return Err(DiagnosticMessages::from_error(e));
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::registry::mcp::RegistryMcpArgs;

    #[test]
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryMcpArgs>();
    }
}
