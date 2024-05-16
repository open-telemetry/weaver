// SPDX-License-Identifier: Apache-2.0

//! Commands to manage a semantic convention registry.

use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Subcommand};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use check::RegistryCheckArgs;
use weaver_cache::Cache;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, Error, PolicyPackage};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::error::handle_errors;
use weaver_common::Logger;
use weaver_forge::{GeneratorConfig, TemplateEngine};
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::SchemaResolver;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::semconv::SemConvSpec;

use crate::registry::generate::RegistryGenerateArgs;
use crate::registry::resolve::RegistryResolveArgs;
use crate::registry::search::RegistrySearchArgs;
use crate::registry::stats::RegistryStatsArgs;
use crate::registry::update_markdown::RegistryUpdateMarkdownArgs;

mod check;
mod generate;
mod resolve;
mod search;
mod stats;
mod update_markdown;

/// Parameters for the `registry` command
#[derive(Debug, Args)]
pub struct RegistryCommand {
    /// Define the sub-commands for the `registry` command
    #[clap(subcommand)]
    pub command: RegistrySubCommand,
}

/// Sub-commands to manage a `registry`.
#[derive(Debug, Subcommand)]
pub enum RegistrySubCommand {
    /// Validates a registry (i.e., parsing, resolution of references, extends clauses, and constraints).
    Check(RegistryCheckArgs),
    /// Generates artifacts from a registry.
    Generate(RegistryGenerateArgs),
    /// Resolves a registry.
    Resolve(RegistryResolveArgs),
    /// Searches a registry (not yet implemented).
    Search(RegistrySearchArgs),
    /// Calculate and display a set of general statistics on a registry (not yet implemented).
    Stats(RegistryStatsArgs),
    /// Update markdown files that contain markers indicating the templates used to update the specified sections.
    UpdateMarkdown(RegistryUpdateMarkdownArgs),
}

/// Path to a semantic convention registry.
/// The path can be a local directory or a Git URL.
#[derive(Debug, Clone)]
pub enum RegistryPath {
    Local(String),
    Url(String),
}

/// Implement the `FromStr` trait for `RegistryPath`, so that it can be used as
/// a command-line argument.
impl FromStr for RegistryPath {
    type Err = String;

    /// Parse a string into a `RegistryPath`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(Self::Url(s.to_owned()))
        } else {
            Ok(Self::Local(s.to_owned()))
        }
    }
}

/// Implement the `Display` trait for `RegistryPath`, so that it can be printed
/// to the console.
impl Display for RegistryPath {
    /// Format the `RegistryPath` as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryPath::Local(path) => write!(f, "{}", path),
            RegistryPath::Url(url) => write!(f, "{}", url),
        }
    }
}

/// Set of parameters used to specify a semantic convention registry.
#[derive(Args, Debug)]
pub struct RegistryArgs {
    /// Local path or Git URL of the semantic convention registry.
    #[arg(
        short = 'r',
        long,
        default_value = "https://github.com/open-telemetry/semantic-conventions.git"
    )]
    pub registry: RegistryPath,

    /// Optional path in the Git repository where the semantic convention
    /// registry is located
    #[arg(short = 'd', long, default_value = "model")]
    pub registry_git_sub_dir: Option<String>,
}

/// Set of parameters used to specify the diagnostic format.
#[derive(Args, Debug, Clone)]
pub struct DiagnosticArgs {
    /// Format used to render the diagnostic messages. Predefined formats are: ansi, json,
    /// gh_workflow_command.
    #[arg(long, default_value = "ansi")]
    pub diagnostic_format: String,

    /// Path to the directory where the diagnostic templates are located.
    #[arg(long, default_value = "diagnostic_templates")]
    pub diagnostic_template: PathBuf,
}

/// Manage a semantic convention registry and return the exit code.
#[cfg(not(tarpaulin_include))]
pub fn semconv_registry(log: impl Logger + Sync + Clone, command: &RegistryCommand) -> i32 {
    let cache = match Cache::try_new() {
        Ok(cache) => cache,
        Err(e) => {
            log.error(&format!("Failed to create cache: {}", e));
            return 1;
        }
    };

    let (cmd_result, diag_args) = match &command.command {
        RegistrySubCommand::Check(args) => (
            check::command(log.clone(), &cache, args),
            args.diagnostic.clone(),
        ),
        RegistrySubCommand::Generate(args) => (
            generate::command(log.clone(), &cache, args),
            args.diagnostic.clone(),
        ),
        RegistrySubCommand::Stats(args) => (
            stats::command(log.clone(), &cache, args),
            args.diagnostic.clone(),
        ),
        RegistrySubCommand::Resolve(args) => (
            resolve::command(log.clone(), &cache, args),
            args.diagnostic.clone(),
        ),
        RegistrySubCommand::Search(_) => unimplemented!(),
        RegistrySubCommand::UpdateMarkdown(args) => (
            update_markdown::command(log.clone(), &cache, args),
            args.diagnostic.clone(),
        ),
    };

    process_diagnostics(cmd_result, diag_args, log.clone())
}

/// Render the diagnostic messages based on the diagnostic configuration and return the exit code
/// based on the diagnostic messages.
fn process_diagnostics(
    cmd_result: Result<(), DiagnosticMessages>,
    diagnostic_args: DiagnosticArgs,
    logger: impl Logger + Sync + Clone,
) -> i32 {
    if let Err(diag_msgs) = cmd_result {
        let config = GeneratorConfig::new(diagnostic_args.diagnostic_template);
        match TemplateEngine::try_new(&diagnostic_args.diagnostic_format, config) {
            Ok(engine) => {
                match engine.generate(logger.clone(), &diag_msgs, PathBuf::new().as_path()) {
                    Ok(_) => {}
                    Err(e) => {
                        logger.error(&format!(
                            "Failed to render the diagnostic messages. Error: {}",
                            e
                        ));
                        return 1;
                    }
                }
            }
            Err(e) => {
                logger.error(&format!("Failed to create the template engine to render the diagnostic messages. Error: {}", e));
                return 1;
            }
        }
        return if diag_msgs.has_error() { 1 } else { 0 };
    }

    // Return 0 if there are no diagnostic messages
    0
}

/// Convert a `RegistryPath` to a `weaver_semconv::path::RegistryPath`.
#[cfg(not(tarpaulin_include))]
pub(crate) fn semconv_registry_path_from(
    registry: &RegistryPath,
    path: &Option<String>,
) -> weaver_semconv::path::RegistryPath {
    match registry {
        RegistryPath::Local(path) => weaver_semconv::path::RegistryPath::Local {
            path_pattern: path.clone(),
        },
        RegistryPath::Url(url) => weaver_semconv::path::RegistryPath::GitUrl {
            git_url: url.clone(),
            path: path.clone(),
        },
    }
}

/// Load the semantic convention specifications from a registry path.
///
/// # Arguments
///
/// * `registry_path` - The path to the semantic convention registry.
/// * `cache` - The cache to use for loading the registry.
/// * `log` - The logger to use for logging messages.
#[cfg(not(tarpaulin_include))]
pub(crate) fn load_semconv_specs(
    registry_path: &weaver_semconv::path::RegistryPath,
    cache: &Cache,
    log: impl Logger + Sync + Clone,
) -> Result<Vec<(String, SemConvSpec)>, weaver_resolver::Error> {
    let semconv_specs = SchemaResolver::load_semconv_specs(registry_path, cache)?;
    log.success(&format!(
        "SemConv registry loaded ({} files)",
        semconv_specs.len()
    ));
    Ok(semconv_specs)
}

/// Check the policies of a semantic convention registry.
///
/// # Arguments
///
/// * `policy_engine` - The pre-configured policy engine to use for checking the policies.
/// * `semconv_specs` - The semantic convention specifications to check.
#[cfg(not(tarpaulin_include))]
pub fn check_policy(
    policy_engine: &Engine,
    semconv_specs: &[(String, SemConvSpec)],
) -> Result<(), Error> {
    // Check policies in parallel
    let policy_errors = semconv_specs
        .par_iter()
        .flat_map(|(path, semconv)| {
            // Create a local policy engine inheriting the policies
            // from the global policy engine
            let mut policy_engine = policy_engine.clone();
            let mut errors = vec![];

            match policy_engine.set_input(semconv) {
                Ok(_) => match policy_engine.check(PolicyPackage::BeforeResolution) {
                    Ok(violations) => {
                        for violation in violations {
                            errors.push(PolicyViolation {
                                provenance: path.clone(),
                                violation,
                            });
                        }
                    }
                    Err(e) => errors.push(InvalidPolicyFile {
                        file: path.to_string(),
                        error: e.to_string(),
                    }),
                },
                Err(e) => errors.push(InvalidPolicyFile {
                    file: path.to_string(),
                    error: e.to_string(),
                }),
            }
            errors
        })
        .collect::<Vec<Error>>();

    handle_errors(policy_errors)?;
    Ok(())
}

/// Check the policies of a semantic convention registry.
///
/// # Arguments
///
/// * `policies` - The list of policy files to check.
/// * `semconv_specs` - The semantic convention specifications to check.
/// * `logger` - The logger to use for logging messages.
#[cfg(not(tarpaulin_include))]
fn check_policies(
    registry_path: &weaver_semconv::path::RegistryPath,
    cache: &Cache,
    policies: &[PathBuf],
    semconv_specs: &[(String, SemConvSpec)],
    logger: impl Logger + Sync + Clone,
) -> Result<(), DiagnosticMessages> {
    let mut engine = Engine::new();

    // Add policies from the registry
    let (registry_path, _) = SchemaResolver::path_to_registry(registry_path, cache)?;
    let added_policies_count = engine.add_policies(registry_path.as_path(), "*.rego")?;

    // Add policies from the command line
    for policy in policies {
        engine.add_policy(policy)?;
    }

    if added_policies_count + policies.len() > 0 {
        check_policy(&engine, semconv_specs).map_err(|e| {
            if let Error::CompoundError(errors) = e {
                DiagnosticMessages::from_errors(errors)
            } else {
                DiagnosticMessages::from_error(e)
            }
        })?;
        logger.success("Policies checked");
    } else {
        logger.success("No policy found");
    }
    Ok(())
}

/// Resolve the semantic convention specifications and return the resolved schema.
///
/// # Arguments
///
/// * `registry_id` - The ID of the semantic convention registry.
/// * `semconv_specs` - The semantic convention specifications to resolve.
/// * `logger` - The logger to use for logging messages.
#[cfg(not(tarpaulin_include))]
pub(crate) fn resolve_semconv_specs(
    registry: &mut SemConvRegistry,
    logger: impl Logger + Sync + Clone,
) -> Result<ResolvedTelemetrySchema, DiagnosticMessages> {
    let resolved_schema = SchemaResolver::resolve_semantic_convention_registry(registry)?;

    logger.success("SemConv registry resolved");
    Ok(resolved_schema)
}
