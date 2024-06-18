// SPDX-License-Identifier: Apache-2.0

//! Utility functions for resolving a semantic convention registry and checking policies.
//! This module supports the `schema` and `registry` commands.

use crate::registry::RegistryPath;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;
use serde::Serialize;
use weaver_cache::Cache;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, Error, PolicyStage};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::error::handle_errors;
use weaver_common::Logger;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::SchemaResolver;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::semconv::SemConvSpec;

/// Converts a `RegistryPath` to a `weaver_semconv::path::RegistryPath`.
///
/// # Arguments
///
/// * `registry` - A reference to the `RegistryPath`.
/// * `path` - An optional string representing the path.
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

/// Loads the semantic convention specifications from a registry path.
///
/// # Arguments
///
/// * `registry_path` - The path to the semantic convention registry.
/// * `cache` - The cache for loading the registry.
/// * `log` - The logger for logging messages.
///
/// # Returns
///
/// A `Result` containing a vector of tuples with file names and `SemConvSpec` on success,
/// or a `weaver_resolver::Error` on failure.
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

/// Initializes the policy engine with policies from the registry and command line.
///
/// # Arguments
///
/// * `registry_path` - The path to the semantic convention registry.
/// * `cache` - The cache for loading the registry.
/// * `policies` - A list of paths to policy files.
/// * `policy_coverage` - A flag to enable policy coverage.
///
/// # Returns
///
/// A `Result` containing the initialized `Engine` on success, or `DiagnosticMessages`
/// on failure.
pub(crate) fn init_policy_engine(
    registry_path: &weaver_semconv::path::RegistryPath,
    cache: &Cache,
    policies: &[PathBuf],
    policy_coverage: bool,
) -> Result<Engine, DiagnosticMessages> {
    let mut engine = Engine::new();

    if policy_coverage {
        engine.enable_coverage();
    }

    // Add policies from the registry
    let (registry_path, _) = SchemaResolver::path_to_registry(registry_path, cache)?;
    _ = engine.add_policies(registry_path.as_path(), "*.rego")?;

    // Add policies from the command line
    for policy in policies {
        _ = engine.add_policy(policy)?;
    }

    Ok(engine)
}

/// Runs the policy engine on a serializable input and returns
/// a list of policy violations represented as errors.
///
/// # Arguments
///
/// * `policy_engine` - The policy engine.
/// * `policy_stage` - The policy stage to check.
/// * `policy_file` - The policy file to check.
/// * `input` - The input to check.
///
/// # Returns
///
/// A list of policy violations represented as errors.
pub(crate) fn check_policy_stage<T: Serialize>(
    policy_engine: &mut Engine,
    policy_stage: PolicyStage,
    policy_file: &str,
    input: &T,
) -> Vec<Error> {
    let mut errors = vec![];

    match policy_engine.set_input(input) {
        Ok(_) => match policy_engine.check(policy_stage) {
            Ok(violations) => {
                for violation in violations {
                    errors.push(PolicyViolation {
                        provenance: policy_file.to_owned(),
                        violation,
                    });
                }
            }
            Err(e) => errors.push(InvalidPolicyFile {
                file: policy_file.to_owned(),
                error: e.to_string(),
            }),
        },
        Err(e) => errors.push(InvalidPolicyFile {
            file: policy_file.to_owned(),
            error: e.to_string(),
        }),
    }
    errors
}

/// Checks the policies of a semantic convention registry.
///
/// # Arguments
///
/// * `policy_engine` - The pre-configured policy engine for checking policies.
/// * `semconv_specs` - The semantic convention specifications to check.
///
/// # Returns
///
/// A `Result` which is `Ok` if all policies are checked successfully, or an `Error`
/// if any policy violations occur.
pub(crate) fn check_policy(
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
            check_policy_stage(
                &mut policy_engine,
                PolicyStage::BeforeResolution,
                path,
                semconv
            )
        })
        .collect::<Vec<Error>>();

    handle_errors(policy_errors)?;
    Ok(())
}

/// Checks the policies of a semantic convention registry.
///
/// # Arguments
///
/// * `policy_engine` - The policy engine.
/// * `semconv_specs` - The semantic convention specifications to check.
/// * `logger` - The logger for logging messages.
///
/// # Returns
///
/// A `Result` which is `Ok` if all policies are checked successfully,
/// or `DiagnosticMessages` if any policy violations occur.
pub(crate) fn check_policies(
    policy_engine: &Engine,
    semconv_specs: &[(String, SemConvSpec)],
    logger: impl Logger + Sync + Clone,
) -> Result<(), DiagnosticMessages> {
    if policy_engine.policy_package_count() > 0 {
        check_policy(&policy_engine, semconv_specs).map_err(|e| {
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

/// Resolves the semantic convention specifications and returns the resolved schema.
///
/// # Arguments
///
/// * `registry` - The semantic convention registry to resolve.
/// * `logger` - The logger for logging messages.
///
/// # Returns
///
/// A `Result` containing the `ResolvedTelemetrySchema` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn resolve_semconv_specs(
    registry: &mut SemConvRegistry,
    logger: impl Logger + Sync + Clone,
) -> Result<ResolvedTelemetrySchema, DiagnosticMessages> {
    let resolved_schema = SchemaResolver::resolve_semantic_convention_registry(registry)?;

    logger.success("SemConv registry resolved");
    Ok(resolved_schema)
}
