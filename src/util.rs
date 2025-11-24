// SPDX-License-Identifier: Apache-2.0

//! Utility functions for resolving a semantic convention registry and checking policies.
//! This module supports the `schema` and `registry` commands.

use crate::registry::{PolicyArgs, RegistryArgs};
use miette::Diagnostic;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use std::path::PathBuf;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, Error, PolicyStage, SEMCONV_REGO};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages, ResultExt};
use weaver_common::log_success;
use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectory;
use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::SchemaResolver;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::semconv::SemConvSpec;
use weaver_semconv::semconv::SemConvSpecWithProvenance;

/// Errors that could occur in these utilities.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum PolicyError {
    /// The usage of "before-resolution" rego policies is unsupported.
    #[error("The usage of \"before-resolution\" rego policies is unsupported with V2 schema.")]
    #[diagnostic(severity(Warning))]
    BeforeResolutionUnsupported,

    /// Issue running V2 policy enforcement due to underlying error.
    #[error(
        "V2 Policy enforcement requests, but repository cannot be converted in to v2: {error}"
    )]
    InvalidV2RepositoryNeedingV2Policies { error: String },
}

/// Loads the semantic convention specifications from a registry path.
///
/// # Arguments
///
/// * `registry_repo` - The registry repository.
///
/// # Returns
///
/// A `Result` containing a vector of tuples with file names and `SemConvSpec` on success,
/// or a `weaver_resolver::Error` on failure.
pub(crate) fn load_semconv_specs(
    registry_repo: &RegistryRepo,
    follow_symlinks: bool,
) -> WResult<Vec<SemConvSpecWithProvenance>, weaver_semconv::Error> {
    SchemaResolver::load_semconv_specs(registry_repo, true, follow_symlinks).inspect(
        |semconv_specs, _| {
            log_success(format!(
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

    // TODO(jsuereth) - Only include standard policies in legacy mode.

    // Add the standard semconv policies
    // Note: `add_policy` the package name, we ignore it here as we don't need it
    _ = engine
        .add_policy("defaults/rego/semconv.rego", SEMCONV_REGO)
        .map_err(DiagnosticMessages::from_error)?;

    // Add policies from the registry
    _ = engine.add_policies(registry_repo.path(), "*.rego")?;

    // Add the user-provided policies
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
                        violation: Box::new(violation),
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
    semconv_specs: &[SemConvSpecWithProvenance],
) -> WResult<(), Error> {
    // Check policies in parallel
    let results = semconv_specs
        .par_iter()
        .map(|semconv| {
            // Create a local policy engine inheriting the policies
            // from the global policy engine
            let mut policy_engine = policy_engine.clone();
            check_policy_stage::<SemConvSpec, ()>(
                &mut policy_engine,
                PolicyStage::BeforeResolution,
                semconv.provenance.path.as_str(),
                &semconv.spec,
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
///
/// # Returns
///
/// A `Result` containing the `ResolvedTelemetrySchema` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn resolve_semconv_specs(
    registry: &mut SemConvRegistry,
    include_unreferenced: bool,
) -> WResult<ResolvedTelemetrySchema, DiagnosticMessage> {
    let registry_id = registry.id().to_owned();
    match SchemaResolver::resolve_semantic_convention_registry(registry, include_unreferenced) {
        WResult::Ok(resolved_schema) => {
            log_success(format!("`{registry_id}` semconv registry resolved"));
            WResult::Ok(resolved_schema)
        }
        WResult::OkWithNFEs(resolved_schema, errs) => {
            log_success(format!("`{registry_id}` semconv registry resolved"));
            let nfes = errs.into_iter().map(DiagnosticMessage::new).collect();
            WResult::OkWithNFEs(resolved_schema, nfes)
        }
        WResult::FatalErr(err) => WResult::FatalErr(DiagnosticMessage::new(err)),
    }
}

/// Resolves the telemetry schema from the given semantic convention specifications.
pub(crate) fn resolve_telemetry_schema(
    registry_repo: &RegistryRepo,
    semconv_specs: Vec<SemConvSpecWithProvenance>,
    include_unreferenced: bool,
) -> WResult<ResolvedTelemetrySchema, DiagnosticMessage> {
    let mut registry = match SemConvRegistry::from_semconv_specs(registry_repo, semconv_specs) {
        Ok(registry) => registry,
        Err(e) => return WResult::FatalErr(DiagnosticMessage::new(e)),
    };
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    resolve_semconv_specs(&mut registry, include_unreferenced)
}

/// Prepares the Rego policy engine given the command line argument input.
pub(crate) fn prepare_policy_engine(
    policy_args: &PolicyArgs,
    registry_repo: &RegistryRepo,
) -> Result<Option<Engine>, DiagnosticMessages> {
    if !policy_args.skip_policies {
        // Create and hold all VirtualDirectory instances to keep them from being dropped
        let policy_vdirs: Vec<VirtualDirectory> = policy_args
            .policies
            .iter()
            .map(|path| {
                VirtualDirectory::try_new(path).map_err(|e| {
                    DiagnosticMessages::from_error(weaver_common::Error::InvalidVirtualDirectory {
                        path: path.to_string(),
                        error: e.to_string(),
                    })
                })
            })
            .collect::<Result<_, _>>()?;

        // Extract paths from VirtualDirectory instances
        let policy_paths: Vec<PathBuf> = policy_vdirs
            .iter()
            .map(|vdir| vdir.path().to_owned())
            .collect();

        Ok(Some(init_policy_engine(
            registry_repo,
            &policy_paths,
            policy_args.display_policy_coverage,
        )?))
    } else {
        Ok(None)
    }
}

/// Resolves the main registry and optionally checks policies.
/// This is a common starting point for some `registry` commands.
/// e.g., `check`, `generate`, `resolve`
///
/// # Arguments
///
/// * `registry_args` - The common CLI args for the main registry.
/// * `policy_args` - The common CLI args for policies.
/// * `diag_msgs` - The DiagnosticMessages to append to.
///
/// # Returns
///
/// A `Result` containing the `ResolvedRegistry` and `PolicyEngine` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn prepare_main_registry(
    registry_args: &RegistryArgs,
    policy_args: &PolicyArgs,
    diag_msgs: &mut DiagnosticMessages,
) -> Result<(ResolvedRegistry, Option<Engine>), DiagnosticMessages> {
    let registry_path = &registry_args.registry;

    let main_registry_repo = RegistryRepo::try_new("main", registry_path)?;

    // Load the semantic convention specs
    let main_semconv_specs = load_semconv_specs(&main_registry_repo, registry_args.follow_symlinks)
        .capture_non_fatal_errors(diag_msgs)?;

    // Optionally init policy engine
    let mut policy_engine = prepare_policy_engine(policy_args, &main_registry_repo)?;

    // Check pre-resolution policies
    if let Some(engine) = policy_engine.as_ref() {
        check_policy(engine, &main_semconv_specs)
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    log_success(format!(
                        "All `before_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    log_success("No `before_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
    }

    // Resolve the main registry
    let mut main_registry =
        SemConvRegistry::from_semconv_specs(&main_registry_repo, main_semconv_specs)?;
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    let main_resolved_schema =
        resolve_semconv_specs(&mut main_registry, registry_args.include_unreferenced)
            .capture_non_fatal_errors(diag_msgs)?;

    // This creates the template/json friendly registry.
    let main_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
        &main_resolved_schema.registry,
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
                log_success(format!(
                    "All `after_resolution` policies checked ({} violations found)",
                    violations.len()
                ));
            } else {
                log_success("No `after_resolution` policy violation");
            }
        })
        .capture_non_fatal_errors(diag_msgs)?;
    }

    Ok((main_resolved_registry, policy_engine))
}

/// Resolves the main registry and optionally checks policies.
/// This is a common starting point for some `registry` commands.
/// e.g., `check`, `generate`, `resolve`
///
/// # Arguments
///
/// * `registry_args` - The common CLI args for the main registry.
/// * `policy_args` - The common CLI args for policies.
/// * `diag_msgs` - The DiagnosticMessages to append to.
///
/// # Returns
///
/// A `Result` containing the `ResolvedRegistry` and `PolicyEngine` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn prepare_main_registry_v2(
    registry_args: &RegistryArgs,
    policy_args: &PolicyArgs,
    diag_msgs: &mut DiagnosticMessages,
) -> Result<
    (
        ResolvedRegistry,
        weaver_forge::v2::registry::ForgeResolvedRegistry,
        Option<Engine>,
    ),
    DiagnosticMessages,
> {
    let (v1, opt_v2, opt_engine) =
        prepare_main_registry_opt_v2(registry_args, policy_args, diag_msgs)?;
    Ok((v1, opt_v2?, opt_engine))
}

/// Resolves the main registry and optionally checks policies.
/// This is a common starting point for some `registry` commands.
/// e.g., `check`, `generate`, `resolve`
///
/// # Arguments
///
/// * `registry_args` - The common CLI args for the main registry.
/// * `policy_args` - The common CLI args for policies.
/// * `diag_msgs` - The DiagnosticMessages to append to.
///
/// # Returns
///
/// A `Result` containing the `ResolvedRegistry` and `PolicyEngine` on success, or
/// `DiagnosticMessages` on failure.
pub(crate) fn prepare_main_registry_opt_v2(
    registry_args: &RegistryArgs,
    policy_args: &PolicyArgs,
    diag_msgs: &mut DiagnosticMessages,
) -> Result<
    (
        ResolvedRegistry,
        Result<weaver_forge::v2::registry::ForgeResolvedRegistry, DiagnosticMessages>,
        Option<Engine>,
    ),
    DiagnosticMessages,
> {
    let registry_path = &registry_args.registry;

    let main_registry_repo = RegistryRepo::try_new("main", registry_path)?;

    // Load the semantic convention specs
    let main_semconv_specs = load_semconv_specs(&main_registry_repo, registry_args.follow_symlinks)
        .capture_non_fatal_errors(diag_msgs)?;

    // Optionally init policy engine
    let mut policy_engine = prepare_policy_engine(policy_args, &main_registry_repo)?;

    // Check pre-resolution policies
    if let Some(engine) = policy_engine.as_ref() {
        if registry_args.v2 {
            // Issue a warning so we fail --future.
            if engine.has_stage(PolicyStage::BeforeResolution) {
                diag_msgs.extend(PolicyError::BeforeResolutionUnsupported.into());
            }
        } else {
            check_policy(engine, &main_semconv_specs)
                .inspect(|_, violations| {
                    if let Some(violations) = violations {
                        log_success(format!(
                            "All `before_resolution` policies checked ({} violations found)",
                            violations.len()
                        ));
                    } else {
                        log_success("No `before_resolution` policy violation");
                    }
                })
                .capture_non_fatal_errors(diag_msgs)?;
        }
    }

    // Resolve the main registry
    let mut main_registry =
        SemConvRegistry::from_semconv_specs(&main_registry_repo, main_semconv_specs)?;
    // Resolve the semantic convention specifications.
    // If there are any resolution errors, they should be captured into the ongoing list of
    // diagnostic messages and returned immediately because there is no point in continuing
    // as the resolution is a prerequisite for the next stages.
    let main_resolved_schema =
        resolve_semconv_specs(&mut main_registry, registry_args.include_unreferenced)
            .capture_non_fatal_errors(diag_msgs)?;

    // This creates the template/json friendly registry.
    let main_resolved_registry = ResolvedRegistry::try_from_resolved_registry(
        &main_resolved_schema.registry,
        main_resolved_schema.catalog(),
    )
    .combine_diag_msgs_with(diag_msgs)?;

    // Check post-resolution policies for v1
    if !registry_args.v2 {
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
                    log_success(format!(
                        "All `after_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    log_success("No `after_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
        }
    }

    // TODO - fix error passing here so original error is diagnostic.
    let opt_v2_schema: Result<
        weaver_resolved_schema::v2::ResolvedTelemetrySchema,
        weaver_forge::error::Error,
    > = main_resolved_schema
        .try_into()
        .map_err(|e: weaver_resolved_schema::error::Error| {
            weaver_forge::error::Error::TemplateEngineError {
                error: e.to_string(),
            }
        });
    let opt_v2_resolved_registry: Result<
        weaver_forge::v2::registry::ForgeResolvedRegistry,
        weaver_forge::error::Error,
    > = opt_v2_schema.and_then(|v2_schema| {
        weaver_forge::v2::registry::ForgeResolvedRegistry::try_from_resolved_schema(v2_schema)
    });

    // Check post-resolution policies for v2
    if registry_args.v2 {
        let v2_resolved_registry = match opt_v2_resolved_registry.as_ref() {
            Ok(v2_resolved_registry) => v2_resolved_registry,
            Err(e) => {
                return Err(PolicyError::InvalidV2RepositoryNeedingV2Policies {
                    error: e.to_string(),
                }
                .into());
            }
        };
        if let Some(engine) = policy_engine.as_mut() {
            check_policy_stage::<weaver_forge::v2::registry::ForgeResolvedRegistry, ()>(
                engine,
                PolicyStage::AfterResolution,
                main_registry_repo.registry_path_repr(),
                v2_resolved_registry,
                &[],
            )
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    log_success(format!(
                        "All `after_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    log_success("No `after_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
        }
    }
    Ok((
        main_resolved_registry,
        opt_v2_resolved_registry.map_err(|e| e.into()),
        policy_engine,
    ))
}

impl From<PolicyError> for DiagnosticMessages {
    fn from(error: PolicyError) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}
