// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use emit::RegistryEmitArgs;
use miette::Diagnostic;
use serde::Serialize;

use crate::registry::diff::RegistryDiffArgs;
use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::infer::RegistryInferArgs;
use crate::registry::json_schema::RegistryJsonSchemaArgs;
use crate::registry::live_check::RegistryLiveCheckArgs;
use crate::registry::mcp::RegistryMcpArgs;
use crate::registry::package::RegistryPackageArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;
use crate::CmdResult;
use check::RegistryCheckArgs;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::log_warn;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_config::CliOverrides;

mod check;
mod diff;
mod emit;
mod generate;
mod infer;
mod json_schema;
mod live_check;
mod mcp;
mod otlp;
mod package;
mod resolve;
mod search;
mod stats;
mod update_markdown;

/// Errors emitted by the `registry` sub-commands
#[derive(thiserror::Error, Debug, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Invalid parameter passed to the command line
    #[error("The parameter `--param {param}` is invalid. {error}")]
    InvalidParam { param: String, error: String },

    /// Invalid params file passed to the command line
    #[error("The params file `{params_file}` is invalid. {error}")]
    InvalidParams { params_file: PathBuf, error: String },

    #[error(transparent)]
    Schema(#[from] weaver_resolved_schema::error::Error),

    /// Packaging requires a v2 registry
    #[error("Packaging is only supported for v2 registries. Pass `--v2` to enable v2 schema.")]
    PackagingRequiresV2,

    /// Packaging requires a manifest file
    #[error("Registry `{registry}` does not contain a manifest file")]
    PackagingRequiresManifest { registry: String },

    /// Failed to write an output file during packaging
    #[error("Failed to write output file `{path}`: {error}")]
    OutputWrite { path: PathBuf, error: String },

    /// Configuration error (loading or parsing `.weaver.toml`)
    #[error("{error}")]
    Config { error: String },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Parameters for the `registry` command
#[derive(Debug, Args)]
pub struct RegistryCommand {
    /// Define the sub-commands for the `registry` command
    #[clap(subcommand)]
    pub command: RegistrySubCommand,
}

/// Sub-commands to manage a `registry`.
#[derive(Debug, Subcommand)]
#[clap(verbatim_doc_comment)]
pub enum RegistrySubCommand {
    /// Validates a semantic convention registry.
    ///
    /// The validation process for a semantic convention registry involves several steps:
    /// - Loading the semantic convention specifications from a local directory or a git repository.
    /// - Parsing the loaded semantic convention specifications.
    /// - Resolving references and extends clauses within the specifications.
    /// - Checking compliance with specified Rego policies, if provided.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the registry validation is successful.
    #[clap(verbatim_doc_comment)]
    Check(RegistryCheckArgs),
    /// Generates artifacts from a semantic convention registry.
    ///
    /// Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.
    ///
    /// Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.
    ///
    /// The process exits with a code of 0 if the generation is successful.
    #[clap(verbatim_doc_comment)]
    Generate(RegistryGenerateArgs),
    /// DEPRECATED - Resolves a semantic convention registry. This command is deprecated and will be removed in a future version.
    /// Please use 'weaver registry generate' or 'weaver registry package' instead.
    #[clap(verbatim_doc_comment)]
    Resolve(RegistryResolveArgs),
    /// DEPRECATED - Searches a registry. This command is deprecated and will be removed in a future version.
    /// It is not compatible with V2 schema. Please search the generated documentation instead.
    Search(RegistrySearchArgs),
    /// Calculate a set of general statistics on a semantic convention registry.
    Stats(RegistryStatsArgs),
    /// Update markdown files that contain markers indicating the templates used to update the specified sections.
    UpdateMarkdown(RegistryUpdateMarkdownArgs),
    /// Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.
    ///
    /// The produced JSON Schema can be used to generate documentation of the resolved registry format or to generate code in your language of choice if you need to interact with the resolved registry format for any reason.
    #[clap(verbatim_doc_comment)]
    JsonSchema(RegistryJsonSchemaArgs),
    /// Generate a diff between two versions of a semantic convention registry.
    ///
    /// This diff can then be rendered in multiple formats:
    /// - a console-friendly format (default: ansi),
    /// - a structured document in JSON format,
    /// - ...
    #[clap(verbatim_doc_comment)]
    Diff(RegistryDiffArgs),

    /// Emits a semantic convention registry as example signals to your OTLP receiver.
    ///
    /// This uses the standard OpenTelemetry SDK, defaulting to OTLP gRPC on localhost:4317.
    #[clap(verbatim_doc_comment)]
    Emit(RegistryEmitArgs),
    /// Perform a live check on sample telemetry by comparing it to a semantic convention registry.
    ///
    /// Includes: Flexible input ingestion, configurable assessment, and template-based output.
    #[clap(verbatim_doc_comment)]
    LiveCheck(RegistryLiveCheckArgs),

    /// Run an MCP (Model Context Protocol) server for the semantic convention registry.
    ///
    /// This server exposes the registry to LLMs, enabling natural language
    /// queries for finding and understanding semantic conventions while writing
    /// instrumentation code.
    ///
    /// The server communicates over stdio using JSON-RPC.
    #[clap(verbatim_doc_comment)]
    Mcp(RegistryMcpArgs),

    /// Generates a schema file by inferring the schema from a OTLP message.
    #[clap(verbatim_doc_comment)]
    Infer(RegistryInferArgs),

    /// Packages a semantic convention registry into a self-contained artifact.
    #[clap(verbatim_doc_comment)]
    Package(RegistryPackageArgs),
}

/// Default value for `--registry`.
pub const DEFAULT_REGISTRY: &str =
    "https://github.com/open-telemetry/semantic-conventions.git[model]";

/// Set of parameters used to specify a semantic convention registry.
#[derive(Args, Debug, Clone)]
pub struct RegistryArgs {
    /// Local folder, Git repo URL, or Git archive URL of the semantic
    /// convention registry. For Git URLs, a reference can be specified
    /// using the `@refspec` syntax and a sub-folder can be specified
    /// using the `[sub-folder]` syntax after the URL.
    #[arg(short = 'r', long, default_value = DEFAULT_REGISTRY)]
    pub registry: VirtualDirectoryPath,

    /// Boolean flag to specify whether to follow symlinks when loading the registry.
    /// Default is false.
    #[arg(short = 's', long)]
    pub(crate) follow_symlinks: bool,

    /// Boolean flag to include signals and attributes defined in dependency registries,
    /// even if they are not explicitly referenced in the current (custom) registry.
    #[arg(long)]
    pub(crate) include_unreferenced: bool,

    /// Whether or not to output version 2 of the schema.
    /// Note: this will impact both output to templates *and* policies.
    #[arg(long, default_value = "false")]
    pub v2: bool,
}

/// Set of common parameters used for policy checks.
#[derive(Args, Debug, Clone)]
pub struct PolicyArgs {
    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry.  If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<VirtualDirectoryPath>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Display the policy coverage report (useful for debugging).
    #[arg(long, default_value = "false")]
    pub display_policy_coverage: bool,
}

/// Apply shared registry config onto a `RegistryArgs`, using config values
/// as defaults that CLI flags can override.
///
/// Config values only apply when the CLI arg has its default value — explicit
/// CLI flags always win.
///
/// TODO(phase 2): rework this to use the standard `CliOverrides` pattern once
/// `RegistryArgs`/`PolicyArgs`/`DiagnosticArgs` are converted to `Option<T>`
/// fields and downstream callers take resolved `Config` structs. The current
/// "compare against clap default" heuristic is fragile and breaks if defaults
/// change.
pub fn apply_registry_config(args: &mut RegistryArgs, config: &weaver_config::RegistryConfig) {
    if let Some(path) = &config.path {
        if args.registry.to_string() == DEFAULT_REGISTRY {
            if let Ok(parsed) = path.parse() {
                args.registry = parsed;
            }
        }
    }
    if let Some(v) = config.follow_symlinks {
        if !args.follow_symlinks {
            args.follow_symlinks = v;
        }
    }
    if let Some(v) = config.include_unreferenced {
        if !args.include_unreferenced {
            args.include_unreferenced = v;
        }
    }
    if let Some(v) = config.v2 {
        if !args.v2 {
            args.v2 = v;
        }
    }
}

/// Apply shared policy config onto a `PolicyArgs`.
pub fn apply_policy_config(args: &mut PolicyArgs, config: &weaver_config::PolicyConfig) {
    if let Some(paths) = &config.paths {
        if args.policies.is_empty() {
            args.policies = paths.iter().filter_map(|p| p.parse().ok()).collect();
        }
    }
    if let Some(v) = config.skip {
        if !args.skip_policies {
            args.skip_policies = v;
        }
    }
}

/// Load configuration for a command that implements `CliOverrides`.
///
/// Applies the standard layering: defaults → `.weaver.toml` → CLI overrides.
/// Logs the config file path when one is found.
///
/// Returns the command-specific config and the loaded `WeaverConfig` (if found).
/// The caller can use the `WeaverConfig` to apply shared overrides to
/// `RegistryArgs`, `PolicyArgs`, and `DiagnosticArgs` via the `apply_*_config`
/// helper functions.
pub fn load_config<A: CliOverrides>(
    args: &A,
) -> Result<(A::Config, Option<weaver_config::WeaverConfig>), DiagnosticMessages> {
    let found = if let Some(path) = args.config_path() {
        Some((
            path.clone(),
            weaver_config::load(path).map_err(|e| {
                DiagnosticMessages::from(Error::Config {
                    error: e.to_string(),
                })
            })?,
        ))
    } else {
        let cwd = std::env::current_dir().map_err(|e| {
            DiagnosticMessages::from(Error::Config {
                error: format!("Failed to get current directory: {e}"),
            })
        })?;
        weaver_config::discover_and_load(&cwd).map_err(|e| {
            DiagnosticMessages::from(Error::Config {
                error: e.to_string(),
            })
        })?
    };

    let (mut config, weaver_config) = match found {
        Some((path, wc)) => {
            log_warn(format!("Experimental! - Found config: {}", path.display()));
            (A::extract_config(&wc), Some(wc))
        }
        None => (A::Config::default(), None),
    };

    args.apply_overrides(&mut config);
    Ok((config, weaver_config))
}

/// Manage a semantic convention registry and return the exit code.
pub fn semconv_registry(command: &RegistryCommand) -> CmdResult {
    match &command.command {
        RegistrySubCommand::Check(args) => {
            CmdResult::new(check::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Generate(args) => {
            CmdResult::new(generate::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Stats(args) => {
            CmdResult::new(stats::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Resolve(args) => {
            CmdResult::new(resolve::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Search(args) => {
            CmdResult::new(search::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::UpdateMarkdown(args) => CmdResult::new(
            update_markdown::command(args),
            Some(args.diagnostic.clone()),
        ),
        RegistrySubCommand::JsonSchema(args) => {
            CmdResult::new(json_schema::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Diff(args) => {
            CmdResult::new(diff::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::LiveCheck(args) => {
            CmdResult::new(live_check::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Emit(args) => {
            CmdResult::new(emit::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Mcp(args) => {
            CmdResult::new(mcp::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Infer(args) => {
            CmdResult::new(infer::command(args), Some(args.diagnostic.clone()))
        }
        RegistrySubCommand::Package(args) => {
            CmdResult::new(package::command(args), Some(args.diagnostic.clone()))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::cli::Cli;
    use clap::CommandFactory;
    use schemars::schema_for;
    use std::collections::BTreeSet;
    use weaver_config::CliOverrides;

    /// Collect all leaf property names from a JSON schema value, flattening
    /// nested objects. For nested objects like `otlp.grpc_port`, produces
    /// `otlp_grpc_port`. Resolves `$ref` pointers against the root schema.
    fn schema_field_names(
        value: &serde_json::Value,
        root: &serde_json::Value,
        prefix: &str,
        out: &mut BTreeSet<String>,
    ) {
        let Some(obj) = value.as_object() else {
            return;
        };
        let Some(properties) = obj.get("properties").and_then(|v| v.as_object()) else {
            return;
        };
        for (name, sub_schema) in properties {
            let full_name = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}_{name}")
            };
            // Resolve $ref if present (e.g. "#/$defs/LiveCheckOtlpConfig")
            let resolved = sub_schema
                .as_object()
                .and_then(|o| o.get("$ref"))
                .and_then(|r| r.as_str())
                .and_then(|ref_path| {
                    let path = ref_path.strip_prefix("#/")?;
                    let mut current = root;
                    for segment in path.split('/') {
                        current = current.get(segment)?;
                    }
                    Some(current)
                })
                .unwrap_or(sub_schema);

            let is_nested = resolved
                .as_object()
                .and_then(|o| o.get("properties"))
                .is_some();
            if is_nested {
                schema_field_names(resolved, root, &full_name, out);
            } else {
                let _ = out.insert(full_name);
            }
        }
    }

    /// Assert that every config field has a corresponding CLI arg and vice versa
    /// for a given `CliOverrides` implementor.
    ///
    /// Uses schemars `JsonSchema` to discover config fields and clap `CommandFactory`
    /// to discover CLI args. The mapping metadata comes from the `CliOverrides` trait
    /// methods: `config_only_fields()`, `cli_only_args()`, and `field_mappings()`.
    ///
    /// This is fully automatic — adding a config field or CLI arg without the
    /// corresponding counterpart causes a test failure with no manual test upkeep.
    pub(crate) fn assert_config_cli_consistency<A: CliOverrides>() {
        let config_only: BTreeSet<&str> = A::config_only_fields().iter().copied().collect();
        let cli_only: BTreeSet<&str> = A::cli_only_args().iter().copied().collect();
        let name_mappings = A::field_mappings();

        // Extract config field names from the JSON schema
        let schema = schema_for!(A::Config);
        let root = schema.as_value();
        let mut config_fields = BTreeSet::new();
        schema_field_names(root, root, "", &mut config_fields);

        // Extract CLI arg names from clap introspection
        let cmd = Cli::command();
        let registry_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "registry")
            .expect("registry subcommand");
        let sub_cmd = registry_cmd
            .get_subcommands()
            .find(|c| c.get_name() == A::SUBCOMMAND)
            .unwrap_or_else(|| panic!("subcommand '{}' not found", A::SUBCOMMAND));
        let cli_args: BTreeSet<String> = sub_cmd
            .get_arguments()
            .filter_map(|arg| arg.get_long())
            .map(|name| name.replace('-', "_"))
            .collect();

        // Map config field names to their CLI equivalents
        let mapped_config: BTreeSet<String> = config_fields
            .iter()
            .filter(|f| !config_only.contains(f.as_str()))
            .map(|f| {
                name_mappings
                    .iter()
                    .find(|m| m.config_name == f.as_str())
                    .map_or_else(|| f.clone(), |m| m.cli_name.to_owned())
            })
            .collect();

        let cli_comparable: BTreeSet<String> = cli_args
            .iter()
            .filter(|a| !cli_only.contains(a.as_str()))
            .cloned()
            .collect();

        let missing_cli: Vec<_> = mapped_config.difference(&cli_comparable).collect();
        let missing_config: Vec<_> = cli_comparable.difference(&mapped_config).collect();

        assert!(
            missing_cli.is_empty(),
            "[{cmd}] Config fields without CLI args: {missing_cli:?}\n\
             Add a CLI arg, or list in `config_only_fields()`/`field_mappings()`.",
            cmd = A::SUBCOMMAND,
        );
        assert!(
            missing_config.is_empty(),
            "[{cmd}] CLI args without config fields: {missing_config:?}\n\
             Add a config field, or list in `cli_only_args()`.",
            cmd = A::SUBCOMMAND,
        );
    }
}
