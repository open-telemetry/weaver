// SPDX-License-Identifier: Apache-2.0

//! MCP server subcommand for the registry.
//!
//! This module provides the `weaver registry mcp` subcommand that runs an MCP
//! (Model Context Protocol) server exposing the semantic conventions registry
//! to LLMs.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use clap::Args;
use log::info;

use crate::registry::{load_config, RegistryArgs};
use crate::weaver::{Error, Resolved, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_config::{
    EffectivePolicyConfig, EffectiveRegistryConfig, EffectiveResolveConfig, WeaverCommand,
    WeaverConfig,
};
use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_macros::weaver_command;
use weaver_mcp::{McpConfig as ServerConfig, McpError, RegistryLoader, WeaverMcpService};

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

    /// Advice policies directory or virtual directory. Set this to override the default policies.
    #[arg(long)]
    #[config]
    pub advice_policies: Option<VirtualDirectoryPath>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    #[arg(long)]
    #[config]
    pub advice_preprocessor: Option<PathBuf>,

    /// Virtual directory, file, or glob pattern pointing to additional JSON/YAML files to load into OPA rego data.
    /// Files are nested in OPA data using their relative path inside the glob base directory (e.g. schemas/user.json is loaded at data.user).
    #[arg(long)]
    #[config]
    pub advice_data: Option<VirtualDirectoryPath>,
}

/// Resolves registries with the command's effective registry, policy, resolution and auth settings.
struct CliRegistryLoader {
    registry: EffectiveRegistryConfig,
    policy: EffectivePolicyConfig,
    resolve: EffectiveResolveConfig,
    auth: HttpAuthResolver,
}

impl CliRegistryLoader {
    /// Loads and resolves the registry at `path` as a v2 template registry.
    fn resolve(&self, path: &VirtualDirectoryPath) -> Result<ForgeResolvedRegistry, Error> {
        let registry_config = EffectiveRegistryConfig {
            registry: path.clone(),
            ..self.registry.clone()
        };
        let weaver = WeaverEngine::new(&registry_config, &self.policy, &self.resolve, &self.auth);
        let mut diag_msgs = DiagnosticMessages::empty();
        let resolved_v2 = match weaver.load_and_resolve_main(&mut diag_msgs)? {
            Resolved::V1(v) => v.try_into()?,
            Resolved::V2(v) => v,
        };
        Ok(resolved_v2.into_template_schema())
    }
}

impl RegistryLoader for CliRegistryLoader {
    fn load(&self, source: &str) -> Result<ForgeResolvedRegistry, McpError> {
        let path = VirtualDirectoryPath::from_str(source)
            .map_err(|e| McpError::new(format!("Invalid registry path: {e}")))?;
        self.resolve(&path)
            .map_err(|e| McpError::new(e.to_string()))
    }
}

/// Run the MCP server for the semantic convention registry.
pub(crate) fn command(
    args: &RegistryMcpArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg)?;
    info!("Loading semantic convention registry for MCP server");

    let loader = CliRegistryLoader {
        registry: cmd_config.registry,
        policy: cmd_config.policy,
        resolve: cmd_config.resolve,
        auth: auth.clone(),
    };
    let source = loader.registry.registry.to_string();
    let forge_registry = loader.resolve(&loader.registry.registry)?;

    info!("Starting MCP server (communicating over stdio)");
    info!("The server will run until stdin is closed.");

    // Build MCP config from effective config, resolving virtual directories upfront
    let advice_policies_dir =
        VirtualDirectory::try_from_opt_with_auth(cmd_config.config.advice_policies.as_ref(), auth)
            .map_err(DiagnosticMessages::from_error)?;

    let advice_data_dir =
        VirtualDirectory::try_from_opt_with_auth(cmd_config.config.advice_data.as_ref(), auth)
            .map_err(DiagnosticMessages::from_error)?;

    let mcp_config = ServerConfig {
        advice_policies: advice_policies_dir.as_ref().map(VirtualDirectory::path_buf),
        advice_data: advice_data_dir
            .as_ref()
            .map(|v| v.path_str().map(ToOwned::to_owned))
            .transpose()
            .map_err(DiagnosticMessages::from_error)?,
        advice_preprocessor: cmd_config.config.advice_preprocessor,
    };

    let service = WeaverMcpService::new(source, forge_registry, mcp_config)
        .with_registry_loader(Arc::new(loader));
    weaver_mcp::run(service).map_err(DiagnosticMessages::from_error)?;

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
