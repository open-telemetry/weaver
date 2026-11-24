//! High-level registry loading, resolution, diffing, and policy evaluation components.

use std::collections::HashMap;
use std::path::PathBuf;

use miette::Diagnostic;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use weaver_checker::Error::{InvalidPolicyFile, PolicyViolation};
use weaver_checker::{Engine, FindingMatcher, PolicyStage, SEMCONV_REGO};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::log_success;
use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectory;
use weaver_config::{
    EffectivePolicyConfig, EffectiveRegistryConfig, EffectiveResolveConfig, PolicyFindingFilter,
};
use weaver_forge::v1::registry::ResolvedRegistry;
use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as ResolvedTelemetrySchemaV1;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as ResolvedTelemetrySchemaV2;
use weaver_resolver::{
    DefaultSchemaVisitor, LoadedSemconvRegistry, ResolutionTarget, ResolvedSchemaCache,
    SchemaLoadingVisitor, WeaverResolvedSchema, WeaverResolver, WeaverResolverConfig,
};
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::semconv::{SemConvSpecWithProvenance, Versioned};
use weaver_version::schema_changes::SchemaChanges as SchemaChangesV1;
use weaver_version::v2::{SchemaChanges as SchemaChangesV2, SchemaItemChange};

/// Visitor that runs Rego policy evaluation during semantic convention loading.
struct PolicyVisitor<'a> {
    engine: &'a Engine,
    diag_msgs: &'a mut DiagnosticMessages,
    fatal_err: &'a mut Option<weaver_checker::Error>,
}

impl<'a> SchemaLoadingVisitor for PolicyVisitor<'a> {
    /// Evaluates `before_resolution` Rego policies on raw loaded semantic convention specs, aborting on fatal errors.
    fn check_raw_loaded_schema_files(&mut self, loaded: &LoadedSemconvRegistry) -> bool {
        if let LoadedSemconvRegistry::Unresolved { specs, .. } = loaded {
            let mut local_diags = DiagnosticMessages::empty();
            let res = check_policy(self.engine, specs).capture_non_fatal_errors(&mut local_diags);
            self.diag_msgs.extend(local_diags);
            if let Err(e) = res {
                *self.fatal_err = Some(e);
                return false;
            }
        }
        true
    }
}

/// Engine that loads, resolves, and policy-checks a semantic convention registry.
pub struct WeaverEngine<'a> {
    registry_config: &'a EffectiveRegistryConfig,
    policy_config: &'a EffectivePolicyConfig,
    resolve_config: &'a EffectiveResolveConfig,
    /// Per-URL HTTP credential resolver built from `.weaver.toml` (`[[auth]]`).
    auth: &'a HttpAuthResolver,
}

impl<'a> WeaverEngine<'a> {
    /// Creates a [`WeaverEngine`] from effective registry, policy, resolution, and HTTP auth configurations.
    pub fn new(
        registry: &'a EffectiveRegistryConfig,
        policy: &'a EffectivePolicyConfig,
        resolve: &'a EffectiveResolveConfig,
        auth: &'a HttpAuthResolver,
    ) -> Self {
        Self {
            registry_config: registry,
            policy_config: policy,
            resolve_config: resolve,
            auth,
        }
    }

    /// Loads and resolves the main configured registry repository, running any `before_resolution` policies.
    pub fn load_and_resolve_main(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Resolved, Error> {
        let registry_path = &self.registry_config.registry;
        let mut nfes = vec![];
        let main_registry_repo =
            RegistryRepo::try_new_with_auth(None, registry_path, &mut nfes, self.auth)?;
        diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

        self.load_and_resolve_repo(main_registry_repo, diag_msgs)
    }

    /// Loads and resolves any OpenTelemetry repository (V1 or V2), evaluating all configured Rego policies.
    pub fn load_and_resolve_repo(
        &self,
        repo: RegistryRepo,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<Resolved, Error> {
        let registry_path_repr: String = repo.registry_path_repr().to_owned();
        let policy_engine = prepare_policy_engine(self.policy_config, &repo, self.auth)?;

        let config = WeaverResolverConfig::default()
            .with_follow_symlinks(self.registry_config.follow_symlinks)
            .with_include_unreferenced(self.registry_config.include_unreferenced)
            .with_auth(self.auth.clone())
            .with_schema_url_overrides(self.resolve_config.schema_url_overrides.clone())
            .with_target(if self.registry_config.v2 {
                ResolutionTarget::V2
            } else {
                ResolutionTarget::V1
            });
        let mut resolver = WeaverResolver::new(config);

        let mut fatal_err = None;
        let resolved_bundle = if let (false, Some(policy_eng)) =
            (self.registry_config.v2, policy_engine.as_ref())
        {
            let visitor = PolicyVisitor {
                engine: policy_eng,
                diag_msgs,
                fatal_err: &mut fatal_err,
            };
            match resolver.load_and_resolve_schema(repo, visitor) {
                WResult::Ok(r) => r,
                WResult::OkWithNFEs(r, nfes) => {
                    diag_msgs
                        .extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());
                    r
                }
                WResult::FatalErr(weaver_resolver::Error::LoadingAbortedByVisitor) => {
                    return Err(Error::Checker(
                        fatal_err.expect("LoadingAbortedByVisitor implies fatal_err is populated"),
                    ));
                }
                WResult::FatalErr(e) => return Err(e.into()),
            }
        } else {
            if self.registry_config.v2 {
                if let Some(engine) = policy_engine.as_ref() {
                    if engine.has_stage(PolicyStage::BeforeResolution) {
                        diag_msgs.extend(PolicyError::BeforeResolutionUnsupported.into());
                    }
                }
            }
            match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
                WResult::Ok(r) => r,
                WResult::OkWithNFEs(r, nfes) => {
                    diag_msgs
                        .extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());
                    r
                }
                WResult::FatalErr(weaver_resolver::Error::LoadingAbortedByVisitor) => {
                    unreachable!("Dummy visitor never aborts");
                }
                WResult::FatalErr(e) => return Err(e.into()),
            }
        };

        if self.registry_config.v2 || matches!(resolved_bundle, WeaverResolvedSchema::V2(_)) {
            if !self.registry_config.v2 {
                diag_msgs.extend(Error::V2FlagMissingWarning.into());
            }
            let v2_resolved = resolved_bundle.into_v2()?;
            let template = match ForgeResolvedRegistry::try_from_resolved_schema(
                v2_resolved.clone(),
                &mut resolver,
            ) {
                WResult::Ok(t) => t,
                WResult::OkWithNFEs(t, nfes) => {
                    diag_msgs
                        .extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());
                    t
                }
                WResult::FatalErr(e) => return Err(Error::Forge(e)),
            };
            return Ok(Resolved::V2(ResolvedV2 {
                resolved_schema: v2_resolved,
                template_schema: template,
                registry_path_repr,
                policy_engine,
            }));
        }

        let schema_cache = resolver
            .schema_cache_for(&resolved_bundle)
            .capture_non_fatal_errors(diag_msgs)?;
        let resolved = resolved_bundle.into_v1()?;
        let template =
            ResolvedRegistry::try_from_resolved_registry(&resolved.registry, resolved.catalog())?;
        Ok(Resolved::V1(ResolvedV1 {
            resolved_schema: resolved,
            template_schema: template,
            registry_path_repr,
            policy_engine,
            schema_cache,
        }))
    }
}

/// A resolved Weaver repository, either loaded from a published schema or resolved from raw definitions.
pub enum Resolved {
    /// Version 1 resolved repository.
    V1(ResolvedV1),
    /// Version 2 resolved repository.
    V2(ResolvedV2),
}

impl Resolved {
    /// Evaluates `after_resolution` Rego policies against the resolved registry.
    pub fn check_after_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        match self {
            Resolved::V1(v) => v.check_after_resolution_policy(diag_msgs),
            Resolved::V2(v) => v.check_after_resolution_policy(diag_msgs),
        }
    }

    /// Computes schema changes between `self` (head) and `baseline`, returning an error if versions differ.
    pub fn diff(&self, baseline: &Resolved) -> Result<DiffResult, Error> {
        match (self, baseline) {
            (Resolved::V1(h), Resolved::V1(b)) => Ok(DiffResult::V1(h.diff(b))),
            (Resolved::V2(h), Resolved::V2(b)) => Ok(DiffResult::V2(h.diff(b))),
            _ => Err(Error::IncompatibleRegistries),
        }
    }

    /// Evaluates `comparison_after_resolution` Rego policies between `self` and `baseline`.
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

/// Schema diff result for either V1 or V2 registries.
#[allow(
    clippy::large_enum_variant,
    reason = "We plan to remove the variant in the future, and want DiffV2 (largest) to remain on stack."
)]
pub enum DiffResult {
    /// Version 1 schema diff.
    V1(Diff),
    /// Version 2 schema diff.
    V2(DiffV2),
}

/// A resolved V1 repository containing the resolved schema, template schema, and optional policy engine.
pub struct ResolvedV1 {
    resolved_schema: ResolvedTelemetrySchemaV1,
    template_schema: ResolvedRegistry,
    registry_path_repr: String,
    policy_engine: Option<Engine>,
    /// Cached dependency schemas from resolution, used when converting to `ResolvedV2` offline.
    schema_cache: ResolvedSchemaCache,
}

impl ResolvedV1 {
    /// Returns a reference to the resolved schema.
    pub fn resolved_schema(&self) -> &ResolvedTelemetrySchemaV1 {
        &self.resolved_schema
    }

    /// Consumes `self` and returns the resolved schema.
    pub fn into_resolved_schema(self) -> ResolvedTelemetrySchemaV1 {
        self.resolved_schema
    }

    /// Returns the schema available for templating.
    pub fn template_schema(&self) -> &ResolvedRegistry {
        &self.template_schema
    }

    /// Consumes `self` and returns the template schema.
    pub fn into_template_schema(self) -> ResolvedRegistry {
        self.template_schema
    }

    /// Evaluates `after_resolution` Rego policies against this V1 registry.
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

    /// Evaluates `comparison_after_resolution` Rego policies against `baseline`.
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

    /// Computes schema changes between `self` and `other`.
    pub fn diff(&self, other: &Self) -> Diff {
        let changes = self.resolved_schema.diff(&other.resolved_schema);
        Diff { changes }
    }
}

/// A resolved V2 repository containing the resolved schema, template schema, and optional policy engine.
pub struct ResolvedV2 {
    resolved_schema: ResolvedTelemetrySchemaV2,
    template_schema: ForgeResolvedRegistry,
    registry_path_repr: String,
    policy_engine: Option<Engine>,
}

impl ResolvedV2 {
    /// Returns a reference to the resolved V2 schema.
    pub fn resolved_schema(&self) -> &ResolvedTelemetrySchemaV2 {
        &self.resolved_schema
    }

    /// Consumes `self` and returns the resolved V2 schema.
    pub fn into_resolved_schema(self) -> ResolvedTelemetrySchemaV2 {
        self.resolved_schema
    }

    /// Returns the V2 schema available for templating.
    pub fn template_schema(&self) -> &ForgeResolvedRegistry {
        &self.template_schema
    }

    /// Consumes `self` and returns the V2 template schema.
    pub fn into_template_schema(self) -> ForgeResolvedRegistry {
        self.template_schema
    }

    /// Evaluates `after_resolution` Rego policies against this V2 registry.
    pub fn check_after_resolution_policy(
        &self,
        diag_msgs: &mut DiagnosticMessages,
    ) -> Result<(), Error> {
        if let Some(engine) = self.policy_engine.as_ref() {
            let mut e = engine.clone();
            check_policy_stage::<ForgeResolvedRegistry, ()>(
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

    /// Evaluates `comparison_after_resolution` Rego policies against `baseline`.
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

    /// Computes schema changes between `self` and `other`.
    pub fn diff(&self, other: &ResolvedV2) -> DiffV2 {
        let changes = self.resolved_schema.diff(&other.resolved_schema);
        DiffV2 {
            changes,
            head_semconv_version: self.template_schema.schema_url.version().to_owned(),
            baseline_semconv_version: other.template_schema.schema_url.version().to_owned(),
        }
    }
}

/// Converts a resolved V1 repository into V2 offline using its cached dependency schemas.
impl TryFrom<ResolvedV1> for ResolvedV2 {
    type Error = Error;

    /// Converts `value` into a [`ResolvedV2`] using its cached dependency schemas.
    fn try_from(mut value: ResolvedV1) -> Result<Self, Self::Error> {
        let resolved_schema: ResolvedTelemetrySchemaV2 = value.resolved_schema.try_into()?;
        let template_schema = match ForgeResolvedRegistry::try_from_resolved_schema(
            resolved_schema.clone(),
            &mut value.schema_cache,
        ) {
            WResult::Ok(t) => t,
            WResult::OkWithNFEs(t, _) => t,
            WResult::FatalErr(e) => return Err(Error::Forge(e)),
        };
        Ok(Self {
            resolved_schema,
            template_schema,
            registry_path_repr: value.registry_path_repr,
            policy_engine: value.policy_engine,
        })
    }
}

/// The difference between two resolved V1 repositories.
pub struct Diff {
    changes: SchemaChangesV1,
}

impl Diff {
    /// Returns the schema changes context for diff template rendering.
    pub fn as_template_context(&self) -> &SchemaChangesV1 {
        &self.changes
    }
}

/// Serializable template context for rendering V2 schema diffs.
#[derive(Serialize)]
pub struct DiffV2Context<'a> {
    /// Schema item changes grouped by signal category.
    pub changes: HashMap<String, &'a Vec<SchemaItemChange>>,
    /// Metadata for the head registry.
    pub head: DiffManifest,
    /// Metadata for the baseline registry.
    pub baseline: DiffManifest,
}

/// Version metadata for one side of a V2 schema diff.
#[derive(Serialize)]
pub struct DiffManifest {
    /// Semantic convention version string.
    pub semconv_version: String,
}

/// The difference between two resolved V2 repositories.
pub struct DiffV2 {
    changes: SchemaChangesV2,
    head_semconv_version: String,
    baseline_semconv_version: String,
}

impl DiffV2 {
    /// Builds the template rendering context for V2 schema diffs.
    pub fn as_template_context(&self) -> DiffV2Context<'_> {
        let mut changes_map = HashMap::new();
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

/// Errors returned by [`WeaverEngine`] and resolved registry operations.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Error from `weaver_common`.
    #[error(transparent)]
    Common(#[from] weaver_common::Error),
    /// Error from `weaver_checker`.
    #[error(transparent)]
    Checker(#[from] weaver_checker::Error),
    /// Error from `weaver_semconv`.
    #[error(transparent)]
    Semconv(#[from] weaver_semconv::Error),
    /// Error from `weaver_resolver`.
    #[error(transparent)]
    Resolver(#[from] weaver_resolver::Error),
    /// Error from `weaver_forge`.
    #[error(transparent)]
    Forge(#[from] weaver_forge::error::Error),
    /// Error from `weaver_resolved_schema`.
    #[error(transparent)]
    ResolvedSchema(#[from] weaver_resolved_schema::error::Error),
    /// Attempted to diff or compare a V1 registry against a V2 registry.
    #[error("Cannot compare or diff a V1 registry with a V2 registry")]
    IncompatibleRegistries,
    /// Warning emitted when a V2 resolved registry is loaded without `--v2`.
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

/// Prepares the Rego policy engine given effective policy settings.
fn prepare_policy_engine(
    policy_args: &EffectivePolicyConfig,
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
            &policy_args.finding_filters,
        )?))
    } else {
        Ok(None)
    }
}

/// Initializes a Rego [`Engine`] with built-in, registry, and user-provided policies.
fn init_policy_engine(
    registry_repo: &RegistryRepo,
    policies: &[PathBuf],
    policy_coverage: bool,
    finding_filters: &[PolicyFindingFilter],
) -> Result<Engine, Error> {
    let mut engine = Engine::new();
    let matchers = finding_filters
        .iter()
        .map(FindingMatcher::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    engine.set_finding_filters(matchers);

    if policy_coverage {
        engine.enable_coverage();
    }

    // TODO(jsuereth) - Only include standard policies in legacy mode.

    // Add the standard semconv policies (`add_policy` returns the package name, ignored here).
    _ = engine.add_policy("defaults/rego/semconv.rego", SEMCONV_REGO)?;

    // Add policies from the registry
    _ = engine.add_policies(registry_repo.path(), "*.rego")?;

    // Add the user-provided policies
    for policy in policies {
        engine.add_policy_from_file_or_dir(policy)?;
    }

    Ok(engine)
}

/// Evaluates `before_resolution` Rego policies in parallel across `semconv_specs`.
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

/// Evaluates `policy_stage` policies against `input` with optional `data` documents.
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use weaver_common::diagnostic::DiagnosticMessages;
    use weaver_common::http_auth::HttpAuthResolver;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_config::{EffectivePolicyConfig, EffectiveRegistryConfig, EffectiveResolveConfig};

    use super::{Resolved, ResolvedV2, WeaverEngine};

    fn assert_rendered_forge_schema(registry_dir: &str, expected_schema_file: &str) {
        let registry_config = EffectiveRegistryConfig {
            registry: VirtualDirectoryPath::LocalFolder {
                path: registry_dir.to_owned(),
            },
            v2: true,
            follow_symlinks: false,
            include_unreferenced: false,
        };
        let policy_config = EffectivePolicyConfig {
            policies: vec![],
            skip_policies: true,
            display_policy_coverage: false,
            finding_filters: vec![],
        };
        let auth = HttpAuthResolver::default();
        let resolve_config = EffectiveResolveConfig::default();
        let engine = WeaverEngine::new(&registry_config, &policy_config, &resolve_config, &auth);
        let mut diag_msgs = DiagnosticMessages::empty();

        let resolved = engine
            .load_and_resolve_main(&mut diag_msgs)
            .expect("Failed to load and resolve registry");

        let Resolved::V2(v2) = resolved else {
            panic!("Expected Resolved::V2 for {registry_dir}");
        };

        let observed_yaml = serde_saphyr::to_string(&v2.template_schema)
            .expect("Failed to serialize forge schema to YAML");

        let observed_dir = PathBuf::from(format!("observed_output/{registry_dir}"));
        let _ = std::fs::create_dir_all(&observed_dir);
        let _ = std::fs::write(observed_dir.join("forge_schema.yaml"), &observed_yaml);

        let expected_path = Path::new(expected_schema_file);
        if !expected_path.exists() {
            let _ = std::fs::write(expected_path, &observed_yaml);
        }

        let expected_yaml = std::fs::read_to_string(expected_path).unwrap_or_else(|_| {
            panic!("Failed to read expected schema file: {expected_schema_file}")
        });

        let observed_json: serde_json::Value = serde_saphyr::from_str(&observed_yaml)
            .expect("Failed to deserialize observed yaml to json");
        let expected_json: serde_json::Value = serde_saphyr::from_str(&expected_yaml)
            .expect("Failed to deserialize expected yaml to json");

        assert_eq!(
            observed_json,
            expected_json,
            "Rendered forge schema does not match expected for `{registry_dir}`.\nDiff:\n{}",
            weaver_diff::diff_output(&expected_yaml, &observed_yaml)
        );
    }

    #[test]
    fn test_weaver_engine_render_forge_schema_v2_model() {
        assert_rendered_forge_schema(
            "tests/v2_forge/model",
            "tests/v2_forge/expected_schema.yaml",
        );
    }

    #[test]
    fn test_weaver_engine_render_forge_schema_v2_dependencies() {
        assert_rendered_forge_schema(
            "tests/v2_forge_dep/root",
            "tests/v2_forge_dep/expected_schema.yaml",
        );
    }

    #[test]
    fn test_weaver_engine_render_forge_schema_published_v2() {
        assert_rendered_forge_schema(
            "tests/published_v2_registry",
            "tests/published_v2_registry/expected_schema.yaml",
        );
    }

    #[test]
    fn test_weaver_engine_schema_url_overrides() {
        let registry_config = EffectiveRegistryConfig {
            registry: VirtualDirectoryPath::LocalFolder {
                path: "tests/v2_forge_dep/root".to_owned(),
            },
            v2: true,
            follow_symlinks: false,
            include_unreferenced: false,
        };
        let policy_config = EffectivePolicyConfig::skip_all();
        let mut resolve_config = EffectiveResolveConfig::default();
        _ = resolve_config.schema_url_overrides.insert(
            "https://dep.example.com/schemas/1.0.0".try_into().unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "tests/v2_forge_dep/dep".to_owned(),
            },
        );
        let auth = HttpAuthResolver::default();
        let engine = WeaverEngine::new(&registry_config, &policy_config, &resolve_config, &auth);
        let mut diag_msgs = DiagnosticMessages::empty();

        let resolved = engine
            .load_and_resolve_main(&mut diag_msgs)
            .expect("Failed to load and resolve registry with overrides");

        let Resolved::V2(v2) = resolved else {
            panic!("Expected Resolved::V2");
        };
        assert_eq!(
            v2.resolved_schema().schema_url.as_str(),
            "https://root.example.com/schemas/1.0.0"
        );
    }

    #[test]
    fn test_weaver_toml_resolve_override_integration() {
        let toml_str = r#"
[resolve.schema_url_overrides]
"https://dep.example.com/schemas/1.0.0" = "tests/v2_forge_dep/dep"
"#;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".weaver.toml");
        std::fs::write(&config_path, toml_str).unwrap();
        let config = weaver_config::load(&config_path).unwrap();
        let mut resolve_config = EffectiveResolveConfig::default();
        resolve_config.layer_config(&config.resolve);

        let registry_config = EffectiveRegistryConfig {
            registry: VirtualDirectoryPath::LocalFolder {
                path: "tests/v2_forge_dep/root".to_owned(),
            },
            v2: true,
            follow_symlinks: false,
            include_unreferenced: false,
        };
        let policy_config = EffectivePolicyConfig::skip_all();
        let auth = HttpAuthResolver::default();
        let engine = WeaverEngine::new(&registry_config, &policy_config, &resolve_config, &auth);
        let mut diag_msgs = DiagnosticMessages::empty();

        let resolved = engine
            .load_and_resolve_main(&mut diag_msgs)
            .expect("Failed to load and resolve registry using weaver.toml overrides");

        let Resolved::V2(v2) = resolved else {
            panic!("Expected Resolved::V2");
        };
        assert_eq!(
            v2.resolved_schema().schema_url.as_str(),
            "https://root.example.com/schemas/1.0.0"
        );
    }

    #[test]
    fn test_try_from_resolved_v1_to_resolved_v2_with_dependencies() {
        let registry_config = EffectiveRegistryConfig {
            registry: VirtualDirectoryPath::LocalFolder {
                path: "tests/v2_forge_dep/root".to_owned(),
            },
            v2: false,
            follow_symlinks: false,
            include_unreferenced: false,
        };
        let policy_config = EffectivePolicyConfig::skip_all();
        let resolve_config = EffectiveResolveConfig::default();
        let auth = HttpAuthResolver::default();
        let engine = WeaverEngine::new(&registry_config, &policy_config, &resolve_config, &auth);
        let mut diag_msgs = DiagnosticMessages::empty();

        let resolved = engine
            .load_and_resolve_main(&mut diag_msgs)
            .expect("Failed to load and resolve registry with v2=false");

        let Resolved::V1(v1) = resolved else {
            panic!("Expected Resolved::V1 when v2=false");
        };

        let resolved_v2 = ResolvedV2::try_from(v1)
            .expect("ResolvedV2::try_from(ResolvedV1) failed for registry with dependencies");

        assert_eq!(
            resolved_v2.resolved_schema().schema_url.as_str(),
            "https://root.example.com/schemas/1.0.0"
        );
    }

    #[test]
    fn test_published_v2_root_loads_dependencies_for_forge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = r#"
file_format: manifest/2.0
schema_url: https://root.example.com/schemas/1.0.0
description: Test published root registry.
stability: stable
resolved_registry_uri: resolved.yaml
dependencies:
  - schema_url: https://dep.example.com/schemas/1.0.0
"#;
        let resolved_yaml = r#"
file_format: resolved/2.0
schema_url: https://root.example.com/schemas/1.0.0
attribute_catalog: []
registry:
  attributes: []
  attribute_groups: []
  spans: []
  metrics: []
  events: []
  entities: []
refinements:
  spans: []
  metrics: []
  events: []
  entities: []
dependencies:
  - https://dep.example.com/schemas/1.0.0
"#;
        std::fs::write(dir.path().join("manifest.yaml"), manifest).expect("write manifest");
        std::fs::write(dir.path().join("resolved.yaml"), resolved_yaml).expect("write resolved");

        let registry_config = EffectiveRegistryConfig {
            registry: VirtualDirectoryPath::LocalFolder {
                path: dir.path().display().to_string(),
            },
            v2: true,
            follow_symlinks: false,
            include_unreferenced: false,
        };
        let policy_config = EffectivePolicyConfig::skip_all();
        let mut resolve_config = EffectiveResolveConfig::default();
        _ = resolve_config.schema_url_overrides.insert(
            "https://dep.example.com/schemas/1.0.0".try_into().unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "tests/v2_forge_dep/dep".to_owned(),
            },
        );
        let auth = HttpAuthResolver::default();
        let engine = WeaverEngine::new(&registry_config, &policy_config, &resolve_config, &auth);
        let mut diag_msgs = DiagnosticMessages::empty();

        let resolved = engine
            .load_and_resolve_main(&mut diag_msgs)
            .expect("Failed to load published V2 registry with dependency override");

        let Resolved::V2(v2) = resolved else {
            panic!("Expected Resolved::V2");
        };
        let dep_url: weaver_semconv::schema_url::SchemaUrl =
            "https://dep.example.com/schemas/1.0.0".try_into().unwrap();
        assert!(
            v2.template_schema().dependencies.contains_key(&dep_url),
            "published V2 root should load its dependencies into ForgeResolvedRegistry"
        );
    }
}
