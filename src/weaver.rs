//! A module containing all the "process" of running weaver as components.

use std::path::PathBuf;

use miette::Diagnostic;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, PolicyStage, SEMCONV_REGO};
use weaver_common::diagnostic::DiagnosticMessage;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::log_success;
use weaver_common::vdir::VirtualDirectory;
use weaver_common::{diagnostic::DiagnosticMessages, result::WResult};
use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::{LoadedSemconvRegistry, SchemaResolver};
use weaver_semconv::semconv::Versioned;
use weaver_semconv::{registry_repo::RegistryRepo, semconv::SemConvSpecWithProvenance};
use weaver_version::schema_changes::SchemaChanges;

use crate::registry::{PolicyArgs, RegistryArgs};

/// Defines an engine that can
pub struct WeaverEngine<'a> {
    // TODO - divorce config from args
    registry_config: &'a RegistryArgs,
    policy_config: &'a PolicyArgs,
    /// Per-URL HTTP credential resolver built from `.weaver.toml` (`[[auth]]`).
    auth: HttpAuthResolver,
}
impl<'a> WeaverEngine<'a> {
    /// Engine with no HTTP credentials. Prefer [`Self::new_with_auth`] for
    /// subcommands that may pull private remote registries.
    pub fn new(registry: &'a RegistryArgs, policy: &'a PolicyArgs) -> Self {
        Self::new_with_auth(registry, policy, HttpAuthResolver::empty())
    }

    /// Engine that resolves credentials for remote registry / dependency /
    /// policy fetches through `auth`.
    pub fn new_with_auth(
        registry: &'a RegistryArgs,
        policy: &'a PolicyArgs,
        auth: HttpAuthResolver,
    ) -> Self {
        Self {
            registry_config: registry,
            policy_config: policy,
            auth,
        }
    }

    /// Loads  previously resolved schemas or loads and resolves "raw" definitions, executing all policies there-in.
    pub fn load_and_resolve_main(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Resolved, Error> {
        let loaded = self.load_main_definitions(diag_msgs)?;
        if self.registry_config.v2 {
            // Issue a warning so we fail --future.
            if loaded.has_before_resolution_policy() {
                diag_msgs.extend(PolicyError::BeforeResolutionUnsupported.into());
            }
        } else {
            loaded.check_before_resolution_policy(diag_msgs)?;
        }
        self.resolve(loaded, diag_msgs)
    }

    /// Loads "main" weaver definition files (from our config).
    pub fn load_main_definitions(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Loaded, Error> {
        let registry_path = &self.registry_config.registry;
        let mut nfes = vec![];
        let main_registry_repo =
            RegistryRepo::try_new_with_auth(None, registry_path, &mut nfes, &self.auth)?;

        diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

        self.load_definitions(main_registry_repo, diag_msgs)
    }

    /// Loads "raw" weaver definitions files from some external source.
    pub fn load_definitions(
        &self,
        repo: RegistryRepo,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Loaded, Error> {
        // TODO - avoid cloning the repo here.
        let loaded = SchemaResolver::load_semconv_repository_with_auth(
            repo.clone(),
            self.registry_config.follow_symlinks,
            &self.auth,
        )
        .capture_non_fatal_errors(diag_msgs)?;

        // Optionally init policy engine
        let policy_engine = prepare_policy_engine(self.policy_config, &repo, &self.auth)?;
        Ok(Loaded {
            loaded,
            policy_engine,
        })
    }

    /// Resolves a loaded set of weaver definitions into a Resolved Registry.
    pub fn resolve(
        &self,
        loaded: Loaded,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Resolved, Error> {
        let registry_path_repr: String = loaded.loaded.registry_path_repr().to_owned();
        let res_v1 = match loaded.loaded {
            LoadedSemconvRegistry::Unresolved { .. } | LoadedSemconvRegistry::Resolved(_) => {
                let resolved = SchemaResolver::resolve(
                    loaded.loaded,
                    self.registry_config.include_unreferenced,
                )
                .capture_non_fatal_errors(diag_msgs)?;

                // This creates the template/json friendly registry.
                let template = ResolvedRegistry::try_from_resolved_registry(
                    &resolved.registry,
                    resolved.catalog(),
                )?;

                Resolved::V1(ResolvedV1 {
                    resolved_schema: resolved,
                    template_schema: template,
                    registry_path_repr,
                    policy_engine: loaded.policy_engine,
                })
            }
            LoadedSemconvRegistry::ResolvedV2(resolved) => {
                if !self.registry_config.v2 {
                    diag_msgs.extend(Error::V2FlagMissingWarning.into());
                }
                let template =
                    weaver_forge::v2::registry::ForgeResolvedRegistry::try_from_resolved_schema(
                        resolved.clone(),
                    )?;
                return Ok(Resolved::V2(ResolvedV2 {
                    resolved_schema: resolved,
                    template_schema: template,
                    registry_path_repr,
                    policy_engine: loaded.policy_engine,
                }));
            }
        };

        if self.registry_config.v2 {
            if let Resolved::V1(v) = res_v1 {
                return Ok(Resolved::V2(v.try_into()?));
            }
        }
        Ok(res_v1)
    }
}

/// A loaded set of weaver definition files.
///
/// Contains the repository definition and raw files and an optional policy engine with policies for this repo.
pub struct Loaded {
    loaded: LoadedSemconvRegistry,
    policy_engine: Option<Engine>,
}
impl Loaded {
    /// Checks if we have any before resolution policies.
    pub fn has_before_resolution_policy(&self) -> bool {
        self.policy_engine
            .as_ref()
            .map(|engine| engine.has_stage(PolicyStage::BeforeResolution))
            .unwrap_or(false)
    }

    /// Checks before resolution policies.
    pub fn check_before_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(policy_engine) = self.policy_engine.as_ref() {
            // Note: We can't check polices on resolved registries.
            if let LoadedSemconvRegistry::Unresolved { specs, .. } = &self.loaded {
                check_policy(policy_engine, specs).capture_non_fatal_errors(diag_msgs)?;
            }
        }
        Ok(())
    }
}

/// A resolved weaver repository. Could have been derived from raw definitions or loaded directly.
///
/// Contains the optimised schema, a template schema and optional policy engine.
pub enum Resolved {
    V1(ResolvedV1),
    V2(ResolvedV2),
}

impl Resolved {
    pub fn check_after_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        match self {
            Resolved::V1(v) => v.check_after_resolution_policy(diag_msgs),
            Resolved::V2(v) => v.check_after_resolution_policy(diag_msgs),
        }
    }

    pub fn diff(&self, baseline: &Resolved) -> Result<DiffResult, Error> {
        match (self, baseline) {
            (Resolved::V1(h), Resolved::V1(b)) => Ok(DiffResult::V1(h.diff(b))),
            (Resolved::V2(h), Resolved::V2(b)) => Ok(DiffResult::V2(h.diff(b))),
            _ => Err(Error::IncompatibleRegistries),
        }
    }

    pub fn check_comparison_after_resolution(
        &self,
        baseline: &Resolved,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        match (self, baseline) {
            (Resolved::V1(h), Resolved::V1(b)) => h.check_comparison_after_resolution(b, diag_msgs),
            (Resolved::V2(h), Resolved::V2(b)) => h.check_comparison_after_resolution(b, diag_msgs),
            _ => Err(Error::IncompatibleRegistries),
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "We plan to remove the variant in the future, and want DiffV2 (largest) to remain on stack."
)]
pub enum DiffResult {
    V1(Diff),
    V2(DiffV2),
}

/// A resolved weaver repository. Could have been derived from raw definitions or loaded directly.
///
/// Contains the optimised schema, a template schema and optional policy engine.
pub struct ResolvedV1 {
    resolved_schema: ResolvedTelemetrySchema,
    template_schema: ResolvedRegistry,
    registry_path_repr: String,
    policy_engine: Option<Engine>,
}
impl ResolvedV1 {
    /// Returns the resolved schema.
    pub fn resolved_schema(&self) -> &ResolvedTelemetrySchema {
        &self.resolved_schema
    }

    /// Drops resolved and just gives the resolved schema.
    pub fn into_resolved_schema(self) -> ResolvedTelemetrySchema {
        self.resolved_schema
    }

    /// Returns the schema available for templating.
    pub fn template_schema(&self) -> &ResolvedRegistry {
        &self.template_schema
    }

    /// Drops resolved and just gives the template schema.
    pub fn into_template_schema(self) -> ResolvedRegistry {
        self.template_schema
    }

    /// Checks after resolution policies.
    pub fn check_after_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(engine) = self.policy_engine.as_ref() {
            let mut e = engine.clone();
            check_policy_stage::<ResolvedRegistry, ()>(
                &mut e,
                PolicyStage::AfterResolution,
                &self.registry_path_repr,
                &self.template_schema,
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
        Ok(())
    }

    /// Compares this resolved vs. a baseline.
    pub fn check_comparison_after_resolution(
        &self,
        baseline: &ResolvedV1,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(engine) = self.policy_engine.as_ref() {
            let mut policy_engine = engine.clone();
            check_policy_stage(
                &mut policy_engine,
                PolicyStage::ComparisonAfterResolution,
                &self.registry_path_repr,
                &self.template_schema(),
                &[baseline.template_schema()],
            )
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    log_success(format!(
                        "All `comparison_after_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    log_success("No `comparison_after_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
        }
        Ok(())
    }

    /// Differences two repositories.
    pub fn diff(&self, other: &Self) -> Diff {
        let changes = self.resolved_schema.diff(&other.resolved_schema);
        Diff { changes }
    }
}

pub struct ResolvedV2 {
    resolved_schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
    template_schema: weaver_forge::v2::registry::ForgeResolvedRegistry,
    registry_path_repr: String,
    policy_engine: Option<Engine>,
}

impl ResolvedV2 {
    /// Returns the resolved schema.
    pub fn resolved_schema(&self) -> &weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        &self.resolved_schema
    }

    /// Drops resolved and just gives the template schema.
    pub fn into_resolved_schema(self) -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        self.resolved_schema
    }

    /// Returns the schema available for templating.
    pub fn template_schema(&self) -> &weaver_forge::v2::registry::ForgeResolvedRegistry {
        &self.template_schema
    }

    /// Drops resolved and just gives the template schema.
    pub fn into_template_schema(self) -> weaver_forge::v2::registry::ForgeResolvedRegistry {
        self.template_schema
    }

    /// Checks after resolution policies.
    pub fn check_after_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(engine) = self.policy_engine.as_ref() {
            let mut e = engine.clone();
            check_policy_stage::<weaver_forge::v2::registry::ForgeResolvedRegistry, ()>(
                &mut e,
                PolicyStage::AfterResolution,
                &self.registry_path_repr,
                &self.template_schema,
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
        Ok(())
    }

    /// Compares this resolved vs. a baseline.
    pub fn check_comparison_after_resolution(
        &self,
        baseline: &ResolvedV2,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(engine) = self.policy_engine.as_ref() {
            let mut policy_engine = engine.clone();
            check_policy_stage(
                &mut policy_engine,
                PolicyStage::ComparisonAfterResolution,
                &self.registry_path_repr,
                &self.template_schema(),
                &[baseline.template_schema()],
            )
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    log_success(format!(
                        "All `comparison_after_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    log_success("No `comparison_after_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(diag_msgs)?;
        }
        Ok(())
    }

    /// Calculates the difference between this and another schema.
    pub fn diff(&self, other: &ResolvedV2) -> DiffV2 {
        let changes = self.resolved_schema.diff(&other.resolved_schema);
        DiffV2 {
            changes,
            head_semconv_version: self.template_schema.schema_url.version().to_owned(),
            baseline_semconv_version: other.template_schema.schema_url.version().to_owned(),
        }
    }
}

impl TryFrom<ResolvedV1> for ResolvedV2 {
    type Error = Error;

    fn try_from(value: ResolvedV1) -> Result<Self, Self::Error> {
        let resolved_schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
            value.resolved_schema.try_into()?;
        let template_schema =
            weaver_forge::v2::registry::ForgeResolvedRegistry::try_from_resolved_schema(
                resolved_schema.clone(),
            )?;
        Ok(Self {
            resolved_schema,
            template_schema,
            registry_path_repr: value.registry_path_repr,
            policy_engine: value.policy_engine,
        })
    }
}

/// The difference between two resolved repositories.
pub struct Diff {
    changes: SchemaChanges,
}

impl Diff {
    /// Returns the context we'll use to render diffs.
    pub fn as_template_context(&self) -> &SchemaChanges {
        &self.changes
    }
}

#[derive(serde::Serialize)]
pub struct DiffV2Context<'a> {
    pub changes: std::collections::HashMap<String, &'a Vec<weaver_version::v2::SchemaItemChange>>,
    pub head: DiffManifest,
    pub baseline: DiffManifest,
}

#[derive(serde::Serialize)]
pub struct DiffManifest {
    pub semconv_version: String,
}

/// The difference between two resolved v2 repositories.
pub struct DiffV2 {
    changes: weaver_version::v2::SchemaChanges,
    head_semconv_version: String,
    baseline_semconv_version: String,
}

impl DiffV2 {
    /// Returns the context we'll use to render diffs.
    pub fn as_template_context(&self) -> DiffV2Context<'_> {
        let mut changes_map = std::collections::HashMap::new();
        let _ = changes_map.insert(
            "registry_attributes".to_owned(),
            &self.changes.registry.attribute_changes,
        );
        let _ = changes_map.insert(
            "attribute_groups".to_owned(),
            &self.changes.registry.attribute_group_changes,
        );
        let _ = changes_map.insert("metrics".to_owned(), &self.changes.registry.metric_changes);
        let _ = changes_map.insert("events".to_owned(), &self.changes.registry.event_changes);
        let _ = changes_map.insert("spans".to_owned(), &self.changes.registry.span_changes);
        let _ = changes_map.insert(
            "resources".to_owned(),
            &self.changes.registry.entity_changes,
        );

        DiffV2Context {
            changes: changes_map,
            head: DiffManifest {
                semconv_version: self.head_semconv_version.clone(),
            },
            baseline: DiffManifest {
                semconv_version: self.baseline_semconv_version.clone(),
            },
        }
    }
}

/// Errors we expect from the weaver engine.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Common(#[from] weaver_common::Error),
    #[error(transparent)]
    Checker(#[from] weaver_checker::Error),
    #[error(transparent)]
    Semconv(#[from] weaver_semconv::Error),
    #[error(transparent)]
    Resolver(#[from] weaver_resolver::Error),
    #[error(transparent)]
    Forge(#[from] weaver_forge::error::Error),
    #[error(transparent)]
    ResolvedSchema(#[from] weaver_resolved_schema::error::Error),
    #[error("Cannot compare or diff a V1 registry with a V2 registry")]
    IncompatibleRegistries,
    #[error("Loaded a V2 resolved registry without the `--v2` flag. The V2 schema will be used.")]
    #[diagnostic(severity(warning))]
    V2FlagMissingWarning,
}
// TODO - transparently convert to diagnostic messages.
impl From<Error> for DiagnosticMessages {
    fn from(value: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(value)])
    }
}

/// Prepares the Rego policy engine given the command line argument input.
fn prepare_policy_engine(
    policy_args: &PolicyArgs,
    registry_repo: &RegistryRepo,
    auth: &HttpAuthResolver,
) -> Result<Option<Engine>, Error> {
    if !policy_args.skip_policies {
        // Create and hold all VirtualDirectory instances to keep them from being dropped
        let policy_vdirs: Vec<VirtualDirectory> = policy_args
            .policies
            .iter()
            .map(|p| VirtualDirectory::try_new_with_auth(p, auth))
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
fn init_policy_engine(
    registry_repo: &RegistryRepo,
    policies: &[PathBuf],
    policy_coverage: bool,
) -> Result<Engine, Error> {
    let mut engine = Engine::new();

    if policy_coverage {
        engine.enable_coverage();
    }

    // TODO(jsuereth) - Only include standard policies in legacy mode.

    // Add the standard semconv policies
    // Note: `add_policy` the package name, we ignore it here as we don't need it
    _ = engine.add_policy("defaults/rego/semconv.rego", SEMCONV_REGO)?;

    // Add policies from the registry
    _ = engine.add_policies(registry_repo.path(), "*.rego")?;

    // Add the user-provided policies
    for policy in policies {
        engine.add_policy_from_file_or_dir(policy)?;
    }

    Ok(engine)
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
) -> WResult<(), weaver_checker::Error> {
    // Check policies in parallel
    let results = semconv_specs
        .par_iter()
        .map(|semconv| {
            // Create a local policy engine inheriting the policies
            // from the global policy engine
            let mut policy_engine = policy_engine.clone();
            check_policy_stage::<Versioned, ()>(
                &mut policy_engine,
                PolicyStage::BeforeResolution,
                semconv.provenance.path.as_str(),
                &semconv.spec,
                &[],
            )
        })
        .collect::<Vec<WResult<(), weaver_checker::Error>>>();

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
) -> WResult<(), weaver_checker::Error> {
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

impl From<PolicyError> for DiagnosticMessages {
    fn from(error: PolicyError) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}
