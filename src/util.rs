// SPDX-License-Identifier: Apache-2.0

//! Utility functions for resolving a semantic convention registry and checking policies.
//! This module supports the `schema` and `registry` commands.

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use std::path::PathBuf;
use weaver_cache::RegistryRepo;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, Error, PolicyStage, SEMCONV_REGO};
use weaver_common::diagnostic::{DiagnosticMessages, ResultExt};
use weaver_common::result::WResult;
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::SchemaResolver;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::semconv::SemConvSpec;

use crate::registry::{PolicyArgs, RegistryArgs};

/// Loads the semantic convention specifications from a registry path.
///
/// # Arguments
///
/// * `registry_repo` - The registry repository.
/// * `log` - The logger for logging messages.
///
/// # Returns
///
/// A `Result` containing a vector of tuples with file names and `SemConvSpec` on success,
/// or a `weaver_resolver::Error` on failure.
pub(crate) fn load_semconv_specs(
    registry_repo: &RegistryRepo,
    log: impl Logger + Sync + Clone,
    follow_symlinks: bool,
) -> WResult<Vec<(String, SemConvSpec)>, weaver_semconv::Error> {
    SchemaResolver::load_semconv_specs(registry_repo, follow_symlinks).inspect(
        |semconv_specs, _| {
            log.success(&format!(
                "`{}` semconv registry `{}` loaded ({} files)",
                registry_repo.id(),
                registry_repo.registry_path_repr(),
                semconv_specs.len()
            ));
        },
    )
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
    registry_repo: &RegistryRepo,
    policies: &[PathBuf],
    policy_coverage: bool,
) -> Result<Engine, DiagnosticMessages> {
    let mut engine = Engine::new();

    if policy_coverage {
        engine.enable_coverage();
    }

    // Add the standard semconv policies
    // Note: `add_policy` the package name, we ignore it here as we don't need it
    _ = engine
        .add_policy("defaults/rego/semconv.rego", SEMCONV_REGO)
        .map_err(DiagnosticMessages::from_error)?;

    // Add policies from the registry
    _ = engine.add_policies(registry_repo.path(), "*.rego")?;

    // Add policies from the command line
    for policy in policies {
        engine.add_policy_from_file_or_dir(policy)?;
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
pub(crate) fn check_policy_stage<T: Serialize, U: Serialize>(
    policy_engine: &mut Engine,
    policy_stage: PolicyStage,
    policy_file: &str,
    input: &T,
    data: &[U],
) -> WResult<(), Error> {
    let mut errors = vec![];

    for d in data {
        if let Err(err) = policy_engine.add_data(d) {
            errors.push(InvalidPolicyFile {
                file: policy_file.to_owned(),
                error: err.to_string(),
            });
        }
    }

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
    WResult::with_non_fatal_errors((), errors)
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
) -> WResult<(), Error> {
    // Check policies in parallel
    let results = semconv_specs
        .par_iter()
        .map(|(path, semconv)| {
            // Create a local policy engine inheriting the policies
            // from the global policy engine
            let mut policy_engine = policy_engine.clone();
            check_policy_stage::<SemConvSpec, ()>(
                &mut policy_engine,
                PolicyStage::BeforeResolution,
                path,
                semconv,
                &[],
            )
        })
        .collect::<Vec<WResult<(), Error>>>();

    let mut nfes = vec![];
    for result in results {
        match result {
            WResult::Ok(_) => {}
            WResult::OkWithNFEs(_, errors) => {
                nfes.extend(errors);
            }
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        }
    }
    WResult::with_non_fatal_errors((), nfes)
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
    let registry_id = registry.id().to_owned();
    let resolved_schema = SchemaResolver::resolve_semantic_convention_registry(registry)?;

    logger.success(&format!("`{}` semconv registry resolved", registry_id));
    Ok(resolved_schema)
}

/// Resolves the main registry and optionally checks policies.
/// This is a common starting point for some `registry` commands.
/// e.g., `check`, `generate`, `resolve`
///
/// # Arguments
///
/// * `registry_args` - The common CLI args for the main registry.
/// * `policy_args` - The common CLI args for policies.
/// * `logger` - The logger for logging messages.
/// * `diag_msgs` - The DiagnosticMessages to append to.
///
/// # Returns
///
/// A `Result` containing the `ResolvedRegistry` and `PolicyEngine` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn prepare_main_registry(
    registry_args: &RegistryArgs,
    policy_args: &PolicyArgs,
    logger: impl Logger + Sync + Clone,
    diag_msgs: &mut DiagnosticMessages,
) -> Result<(ResolvedRegistry, Option<Engine>), DiagnosticMessages> {
    let registry_path = &registry_args.registry;

    let main_registry_repo = RegistryRepo::try_new("main", registry_path)?;

    // Load the semantic convention specs
    let main_semconv_specs = load_semconv_specs(
        &main_registry_repo,
        logger.clone(),
        registry_args.follow_symlinks,
    )
    .capture_non_fatal_errors(diag_msgs)?;

    // Optionally init policy engine
    let mut policy_engine = if !policy_args.skip_policies {
        Some(init_policy_engine(
            &main_registry_repo,
            &policy_args.policies,
            policy_args.display_policy_coverage,
        )?)
    } else {
        None
    };

    // Check pre-resolution policies
    if let Some(engine) = policy_engine.as_ref() {
        check_policy(engine, &main_semconv_specs)
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    logger.success(&format!(
                        "All `before_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    logger.success("No `before_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
    }

    // Resolve the main registry
    let mut main_registry =
        SemConvRegistry::from_semconv_specs(main_registry_repo.id(), main_semconv_specs);
    let main_resolved_schema = resolve_semconv_specs(&mut main_registry, logger.clone())
        .combine_diag_msgs_with(diag_msgs)?;

    let main_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
        main_resolved_schema
            .registry(main_registry_repo.id())
            .expect("Failed to get the registry from the resolved schema"),
        main_resolved_schema.catalog(),
    )
    .combine_diag_msgs_with(diag_msgs)?;

    // Check post-resolution policies
    if let Some(engine) = policy_engine.as_mut() {
        check_policy_stage::<ResolvedRegistry, ()>(
            engine,
            PolicyStage::AfterResolution,
            main_registry_repo.registry_path_repr(),
            &main_resolved_registry,
            &[],
        )
        .inspect(|_, violations| {
            if let Some(violations) = violations {
                logger.success(&format!(
                    "All `after_resolution` policies checked ({} violations found)",
                    violations.len()
                ));
            } else {
                logger.success("No `after_resolution` policy violation");
            }
        })
        .capture_non_fatal_errors(diag_msgs)?;
    }

    Ok((main_resolved_registry, policy_engine))
}
