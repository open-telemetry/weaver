// SPDX-License-Identifier: Apache-2.0

//! Set of utility functions to resolve a semantic convention registry and check policies.
//! This module is used by the `schema` and `registry` commands.

use crate::registry::RegistryPath;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;
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

/// Convert a `RegistryPath` to a `weaver_semconv::path::RegistryPath`.
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
            let mut errors = vec![];

            match policy_engine.set_input(semconv) {
                Ok(_) => match policy_engine.check(PolicyStage::BeforeResolution) {
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
pub(crate) fn check_policies(
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
pub(crate) fn resolve_semconv_specs(
    registry: &mut SemConvRegistry,
    logger: impl Logger + Sync + Clone,
) -> Result<ResolvedTelemetrySchema, DiagnosticMessages> {
    let resolved_schema = SchemaResolver::resolve_semantic_convention_registry(registry)?;

    logger.success("SemConv registry resolved");
    Ok(resolved_schema)
}
