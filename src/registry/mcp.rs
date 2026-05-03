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
use weaver_config::{
    excluded_args, override_if_set, CliOverrides, EffectiveDiagnosticConfig,
    EffectiveRegistryConfig, McpConfig, WeaverConfig,
};

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

    /// Namespace separator used in attribute keys. Defaults to ".".
    /// Used by namespace browsing and search token splitting.
    #[arg(long)]
    pub namespace_separator: Option<String>,
}

impl CliOverrides for RegistryMcpArgs {
    type Config = McpConfig;
    const SUBCOMMAND: &'static str = "mcp";

    fn extract_config(weaver_config: &WeaverConfig) -> McpConfig {
        weaver_config.mcp.clone()
    }

    fn excluded_args() -> &'static [&'static str] {
        excluded_args!(RegistryArgs::EXCLUDED_ARGS, DiagnosticArgs::EXCLUDED_ARGS,)
    }

    fn apply_overrides(&self, config: &mut McpConfig) {
        override_if_set!(config.advice_policies, self.advice_policies, optional);
        override_if_set!(
            config.advice_preprocessor,
            self.advice_preprocessor,
            optional
        );
        override_if_set!(config.namespace_separator, self.namespace_separator);
    }

    fn apply_registry_overrides(&self, config: &mut EffectiveRegistryConfig) {
        self.registry.apply_to(config);
    }

    fn apply_diagnostic_overrides(&self, config: &mut EffectiveDiagnosticConfig) {
        self.diagnostic.apply_to(config);
    }

    fn uses_policy() -> bool {
        false
    }
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
