// SPDX-License-Identifier: Apache-2.0

//! Centralized resolution engine and dispatch for telemetry schemas.

use lru::LruCache;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_resolved_schema::error::Error as ResolvedSchemaError;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::manifest::Dependency;

use crate::error::Error;
use crate::graph;
use crate::graph::chosen_versions::collect_chosen_versions;
use crate::graph::loader::{LoadedSemconvRegistry, SourceFormat};
use crate::{v1, v2};

/// The target output version requested by the caller for schema resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolutionTarget {
    /// Resolve to OpenTelemetry V1 schema.
    #[default]
    V1,
    /// Resolve to OpenTelemetry V2 schema.
    ///
    /// Pure-V2 definitions and published V2 registries resolve directly into [`WeaverResolvedSchema::V2`],
    /// while V1 and mixed registries resolve through the V1 engine and convert into [`WeaverResolvedSchema::V2`].
    V2,
}

/// A unified, version-agnostic bundle representing an optimized OpenTelemetry schema (either V1 or V2).
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum WeaverResolvedSchema {
    /// An optimized OpenTelemetry V1 schema.
    V1(ResolvedTelemetrySchema),
    /// An optimized OpenTelemetry V2 schema.
    V2(V2Schema),
}

impl WeaverResolvedSchema {
    /// Returns the active schema URL string for this bundle.
    #[must_use]
    pub fn schema_url_str(&self) -> &str {
        match self {
            Self::V1(s) => s.schema_url.as_str(),
            Self::V2(s) => s.schema_url.as_str(),
        }
    }

    /// Converts this bundle into an OpenTelemetry V1 schema if compatible, or returns an error.
    pub fn into_v1(self) -> Result<ResolvedTelemetrySchema, Error> {
        match self {
            Self::V1(s) => Ok(s),
            Self::V2(_) => Err(Error::ConvertingV2ToV1Unsupported),
        }
    }

    /// Converts this bundle into an OpenTelemetry V2 schema, automatically upgrading a V1 schema
    /// when resolving V1 or mixed registries under [`ResolutionTarget::V2`].
    pub fn into_v2(self) -> Result<V2Schema, Error> {
        match self {
            Self::V2(s) => Ok(s),
            Self::V1(s) => s
                .try_into()
                .map_err(|e: ResolvedSchemaError| Error::ConversionError {
                    message: e.to_string(),
                }),
        }
    }

    /// Returns an OpenTelemetry V1 schema reference if this bundle holds a V1 schema.
    #[must_use]
    pub fn as_v1(&self) -> Option<&ResolvedTelemetrySchema> {
        match self {
            Self::V1(s) => Some(s),
            Self::V2(_) => None,
        }
    }

    /// Returns an OpenTelemetry V2 schema reference if this bundle holds a V2 schema.
    #[must_use]
    pub fn as_v2(&self) -> Option<&V2Schema> {
        match self {
            Self::V1(_) => None,
            Self::V2(s) => Some(s),
        }
    }

    /// Returns the set of dependency schema URLs for this schema.
    #[must_use]
    pub fn dependencies(&self) -> &BTreeSet<SchemaUrl> {
        match self {
            Self::V1(s) => &s.dependencies,
            Self::V2(s) => &s.dependencies,
        }
    }
}

/// Runtime configuration for [`WeaverResolver`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct WeaverResolverConfig {
    /// Maximum number of resolved schemas retained in the LRU cache.
    pub cache_capacity: NonZeroUsize,

    /// Whether to follow symbolic links during directory traversal.
    pub follow_symlinks: bool,

    /// Whether to include unreferenced groups in the resolved catalog.
    pub include_unreferenced: bool,

    /// HTTP authentication credentials resolver for remote registry fetches.
    pub auth: HttpAuthResolver,

    /// Overrides mapping a `SchemaUrl` to an alternative `VirtualDirectoryPath`.
    pub schema_url_overrides: BTreeMap<SchemaUrl, VirtualDirectoryPath>,

    /// Target schema version for resolution.
    pub target: ResolutionTarget,
}

impl Default for WeaverResolverConfig {
    fn default() -> Self {
        Self {
            cache_capacity: NonZeroUsize::new(32).expect("32 is valid non-zero capacity"),
            follow_symlinks: false,
            include_unreferenced: false,
            auth: HttpAuthResolver::empty(),
            schema_url_overrides: BTreeMap::new(),
            target: ResolutionTarget::default(),
        }
    }
}

impl WeaverResolverConfig {
    /// Sets the maximum number of resolved schemas retained in the LRU cache.
    #[must_use]
    pub fn with_cache_capacity(mut self, cache_capacity: NonZeroUsize) -> Self {
        self.cache_capacity = cache_capacity;
        self
    }

    /// Sets whether symbolic links are followed during directory traversal.
    #[must_use]
    pub fn with_follow_symlinks(mut self, follow_symlinks: bool) -> Self {
        self.follow_symlinks = follow_symlinks;
        self
    }

    /// Sets whether unreferenced groups and attributes are retained in the resolved schema.
    #[must_use]
    pub fn with_include_unreferenced(mut self, include_unreferenced: bool) -> Self {
        self.include_unreferenced = include_unreferenced;
        self
    }

    /// Sets the HTTP authentication credentials resolver for remote registry fetches.
    #[must_use]
    pub fn with_auth(mut self, auth: HttpAuthResolver) -> Self {
        self.auth = auth;
        self
    }

    /// Sets explicit overrides mapping `SchemaUrl`s to alternative `VirtualDirectoryPath`s.
    #[must_use]
    pub fn with_schema_url_overrides(
        mut self,
        schema_url_overrides: BTreeMap<SchemaUrl, VirtualDirectoryPath>,
    ) -> Self {
        self.schema_url_overrides = schema_url_overrides;
        self
    }

    /// Sets the target schema version (`V1` or `V2`) for resolution.
    #[must_use]
    pub fn with_target(mut self, target: ResolutionTarget) -> Self {
        self.target = target;
        self
    }
}

/// Visitor hook invoked on raw loaded specifications before resolution.
pub trait SchemaLoadingVisitor {
    /// Inspects raw loaded specifications; returning `false` aborts resolution.
    fn check_raw_loaded_schema_files(&mut self, _loaded: &LoadedSemconvRegistry) -> bool {
        true
    }
}

/// No-op [`SchemaLoadingVisitor`] implementation.
#[derive(Debug, Clone, Default)]
pub struct DefaultSchemaVisitor;

impl SchemaLoadingVisitor for DefaultSchemaVisitor {}

/// Resolved direct dependencies and chosen-version metadata for a registry resolution pass.
pub(crate) struct ResolvedDependencyGraph {
    pub dependencies: Vec<Arc<WeaverResolvedSchema>>,
    pub chosen_versions: HashMap<String, SchemaUrl>,
    pub dependency_urls: BTreeSet<SchemaUrl>,
    pub pinned_schemas: HashMap<SchemaUrl, Arc<WeaverResolvedSchema>>,
}

/// Loads and resolves telemetry schemas with an internal LRU dependency cache.
pub struct WeaverResolver {
    cache: LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>,
    active_graph_schemas: HashMap<SchemaUrl, Arc<WeaverResolvedSchema>>,
    direct_dependencies: HashMap<SchemaUrl, Vec<SchemaUrl>>,
    config: WeaverResolverConfig,
    warned_v1_dependencies: BTreeSet<SchemaUrl>,
    root_is_pure_v2: bool,
}

impl WeaverResolver {
    /// Instantiates a new [`WeaverResolver`] from explicit configuration settings.
    #[must_use]
    pub fn new(config: WeaverResolverConfig) -> Self {
        Self {
            cache: LruCache::new(config.cache_capacity),
            active_graph_schemas: HashMap::new(),
            direct_dependencies: HashMap::new(),
            config,
            warned_v1_dependencies: BTreeSet::new(),
            root_is_pure_v2: false,
        }
    }

    /// Redirects fetches of `schema_url` to `target_path` during resolution.
    pub fn add_schema_url_override(
        &mut self,
        schema_url: SchemaUrl,
        target_path: VirtualDirectoryPath,
    ) {
        _ = self
            .config
            .schema_url_overrides
            .insert(schema_url, target_path);
    }

    /// Loads a semantic convention repository and its dependencies without running resolution.
    pub(crate) fn load_repository(
        &mut self,
        registry_repo: RegistryRepo,
    ) -> WResult<LoadedSemconvRegistry, Error> {
        graph::loader::load_semconv_repository_with_cache(
            Some(&self.cache),
            registry_repo,
            self.config.follow_symlinks,
            &self.config.auth,
            &self.config.schema_url_overrides,
        )
    }

    /// Records the direct dependencies of `schema_url` if non-empty and not already recorded,
    /// preserving version-arbitrated entries.
    fn record_direct_dependencies(&mut self, schema_url: &SchemaUrl, direct: Vec<SchemaUrl>) {
        if direct.is_empty() || self.direct_dependencies.contains_key(schema_url) {
            return;
        }
        let _ = self.direct_dependencies.insert(schema_url.clone(), direct);
    }

    /// Returns a shared reference to the resolved schema LRU cache.
    pub(crate) fn cache_ref(&self) -> &LruCache<SchemaUrl, Arc<WeaverResolvedSchema>> {
        &self.cache
    }

    /// Looks up `url` in the active resolution graph or peeks the LRU cache without updating
    /// recency.
    pub(crate) fn active_or_cached_schema(
        &self,
        url: &SchemaUrl,
    ) -> Option<Arc<WeaverResolvedSchema>> {
        self.active_graph_schemas
            .get(url)
            .cloned()
            .or_else(|| self.cache.peek(url).cloned())
    }

    /// Resolves direct dependencies, records dependency edges, arbitrates chosen versions,
    /// and pre-resolves superseded versions into `pinned_schemas`.
    pub(crate) fn resolve_dependency_graph(
        &mut self,
        schema_url: &SchemaUrl,
        dependencies: Vec<LoadedSemconvRegistry>,
        keep_nfe: impl Fn(&Error) -> bool,
    ) -> WResult<ResolvedDependencyGraph, Error> {
        let mut resolved_dependencies = Vec::with_capacity(dependencies.len());
        let mut non_fatal_errors = Vec::new();
        for d in dependencies {
            match self.resolve_dependency(d) {
                WResult::Ok(schema) => resolved_dependencies.push(schema),
                WResult::OkWithNFEs(schema, nfes) => {
                    resolved_dependencies.push(schema);
                    non_fatal_errors.extend(nfes.into_iter().filter(&keep_nfe));
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        let direct: Vec<SchemaUrl> = resolved_dependencies
            .iter()
            .filter_map(|d| SchemaUrl::try_from(d.schema_url_str()).ok())
            .collect();
        let _ = self.direct_dependencies.insert(schema_url.clone(), direct);

        let mut chosen_versions = HashMap::new();
        for d in &resolved_dependencies {
            collect_chosen_versions(
                SchemaUrl::try_from(d.schema_url_str()).ok().as_ref(),
                d.dependencies(),
                &mut chosen_versions,
            );
        }
        let dependency_urls: BTreeSet<SchemaUrl> = chosen_versions.values().cloned().collect();

        let mut pinned_schemas = self.active_graph_schemas.clone();
        for chosen_url in chosen_versions.values() {
            if !pinned_schemas.contains_key(chosen_url) {
                if let Some(cached) = self.active_or_cached_schema(chosen_url) {
                    _ = pinned_schemas.insert(chosen_url.clone(), cached);
                }
            }
        }

        // Pre-resolve uncached chosen versions that supersede a version used in this graph.
        for (reg_name, chosen_url) in &chosen_versions {
            let supersedes_loaded = resolved_dependencies.iter().any(|d| {
                let self_url = SchemaUrl::try_from(d.schema_url_str()).ok();
                self_url
                    .as_ref()
                    .is_some_and(|u| u.name() == reg_name && u != chosen_url)
                    || d.dependencies()
                        .iter()
                        .any(|sub| sub.name() == reg_name && sub != chosen_url)
            });
            if supersedes_loaded && !pinned_schemas.contains_key(chosen_url) {
                match self.resolve_dependency_schema(chosen_url) {
                    WResult::Ok(resolved) => {
                        _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                    }
                    WResult::OkWithNFEs(resolved, nfes) => {
                        _ = pinned_schemas.insert(chosen_url.clone(), resolved);
                        non_fatal_errors.extend(nfes.into_iter().filter(&keep_nfe));
                    }
                    WResult::FatalErr(e) => return WResult::FatalErr(e),
                }
            }
        }

        WResult::with_non_fatal_errors(
            ResolvedDependencyGraph {
                dependencies: resolved_dependencies,
                chosen_versions,
                dependency_urls,
                pinned_schemas,
            },
            non_fatal_errors,
        )
    }

    /// Converts a V1 schema into V2 when resolving a root registry under [`ResolutionTarget::V2`].
    fn finalize_v1_bundle(
        &self,
        schema: ResolvedTelemetrySchema,
        is_root: bool,
    ) -> Result<WeaverResolvedSchema, Error> {
        if is_root && self.config.target == ResolutionTarget::V2 {
            WeaverResolvedSchema::V1(schema)
                .into_v2()
                .map(WeaverResolvedSchema::V2)
        } else {
            Ok(WeaverResolvedSchema::V1(schema))
        }
    }

    /// Inserts `arc` into both the active graph map and the LRU cache under `url`.
    fn record_cached_schema(&mut self, url: SchemaUrl, arc: Arc<WeaverResolvedSchema>) {
        let _ = self.active_graph_schemas.insert(url.clone(), arc.clone());
        let _ = self.cache.put(url, arc);
    }

    /// Returns whether unreferenced groups and attributes should be retained.
    pub(crate) fn include_unreferenced(&self) -> bool {
        self.config.include_unreferenced
    }

    /// Resolves a loaded dependency registry, returning a cached schema when available.
    pub(crate) fn resolve_dependency(
        &mut self,
        loaded: LoadedSemconvRegistry,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        let schema_url = match &loaded {
            LoadedSemconvRegistry::Unresolved { repo, .. } => {
                let manifest = match repo.to_v2_manifest() {
                    Ok(m) => m,
                    Err(e) => return WResult::FatalErr(e.into()),
                };
                manifest.schema_url().clone()
            }
            LoadedSemconvRegistry::Resolved { schema, .. } => {
                match SchemaUrl::try_from(schema.schema_url.as_str()) {
                    Ok(url) => url,
                    Err(_) => return WResult::FatalErr(Error::FailToResolveSchemaUrl {}),
                }
            }
            LoadedSemconvRegistry::ResolvedV2 { schema, .. } => schema.schema_url.clone(),
        };

        if let Some(cached) = self.active_graph_schemas.get(&schema_url).cloned() {
            return WResult::Ok(cached);
        }
        if let Some(cached) = self.cache.get(&schema_url) {
            let arc = cached.clone();
            let _ = self.active_graph_schemas.insert(schema_url, arc.clone());
            return WResult::Ok(arc);
        }

        let mut dep_warnings = Vec::new();
        if self.config.target == ResolutionTarget::V2 && self.root_is_pure_v2 {
            self.collect_v2_dependency_warnings(&loaded, &mut dep_warnings);
        }

        let res = self.resolve_loaded_internal(loaded, false);
        if !dep_warnings.is_empty() {
            res.extend_non_fatal_errors(dep_warnings)
        } else {
            res
        }
    }

    /// Extracts the [`SchemaUrl`] from a loaded semantic convention registry, if valid.
    fn schema_url_of(loaded: &LoadedSemconvRegistry) -> Option<SchemaUrl> {
        match loaded {
            LoadedSemconvRegistry::Unresolved { repo, .. } => {
                repo.to_v2_manifest().ok().map(|m| m.schema_url().clone())
            }
            LoadedSemconvRegistry::Resolved { schema, .. } => {
                SchemaUrl::try_from(schema.schema_url.as_str()).ok()
            }
            LoadedSemconvRegistry::ResolvedV2 { schema, .. } => Some(schema.schema_url.clone()),
        }
    }

    /// Appends V2-fallback warnings for `loaded` and its dependencies when targeting V2.
    fn collect_v2_fallback_warnings(
        &mut self,
        loaded: &LoadedSemconvRegistry,
        is_root: bool,
        out: &mut Vec<Error>,
    ) {
        if self.config.target != ResolutionTarget::V2 {
            return;
        }
        if is_root && loaded.source_format() == Some(SourceFormat::Mixed) {
            if let Some(url) = Self::schema_url_of(loaded) {
                out.push(Error::MixedRegistryUsesV1Path {
                    schema_url: url.as_str().to_owned(),
                });
            }
            return;
        }
        if !self.root_is_pure_v2 {
            return;
        }
        if let LoadedSemconvRegistry::Unresolved { dependencies, .. } = loaded {
            for dep in dependencies {
                self.collect_v2_dependency_warnings(dep, out);
            }
        }
    }

    /// Appends deduplicated [`Error::V1DependencyUsesV1Path`] warnings for `loaded` and its
    /// dependency subtree.
    fn collect_v2_dependency_warnings(
        &mut self,
        loaded: &LoadedSemconvRegistry,
        out: &mut Vec<Error>,
    ) {
        let mut visited = HashSet::new();
        self.collect_v2_dependency_warnings_recursive(loaded, &mut visited, out);
    }

    /// Traverses `loaded` and its dependencies to record [`Error::V1DependencyUsesV1Path`]
    /// warnings once per URL.
    fn collect_v2_dependency_warnings_recursive(
        &mut self,
        loaded: &LoadedSemconvRegistry,
        visited: &mut HashSet<SchemaUrl>,
        out: &mut Vec<Error>,
    ) {
        if let Some(url) = Self::schema_url_of(loaded) {
            if !visited.insert(url.clone()) {
                return;
            }
        }
        if let LoadedSemconvRegistry::Unresolved { dependencies, .. } = loaded {
            if let Some(format @ (SourceFormat::V1 | SourceFormat::Mixed)) = loaded.source_format()
            {
                if let Some(url) = Self::schema_url_of(loaded) {
                    if self.warned_v1_dependencies.insert(url.clone()) {
                        out.push(Error::V1DependencyUsesV1Path {
                            schema_url: url.as_str().to_owned(),
                            format,
                        });
                    }
                }
            }
            for dep in dependencies {
                self.collect_v2_dependency_warnings_recursive(dep, visited, out);
            }
        }
    }

    /// Resolves an already loaded semantic convention repository, executing graph deduplication
    /// and populating the internal LRU cache.
    pub(crate) fn resolve_loaded(
        &mut self,
        loaded: LoadedSemconvRegistry,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        self.resolve_loaded_internal(loaded, true)
    }

    /// Dispatches `loaded` to the V1 or V2 resolution engine and caches the resulting schema.
    fn resolve_loaded_internal(
        &mut self,
        loaded: LoadedSemconvRegistry,
        is_root: bool,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        let source_format = loaded.source_format();
        if is_root {
            self.active_graph_schemas.clear();
            self.root_is_pure_v2 = source_format == Some(SourceFormat::V2);
        }
        let mut v2_warnings = Vec::new();
        self.collect_v2_fallback_warnings(&loaded, is_root, &mut v2_warnings);

        match loaded {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                dependencies,
            } => {
                if self.config.target == ResolutionTarget::V2
                    && source_format == Some(SourceFormat::V2)
                {
                    let res = v2::resolve(self, repo, specs, dependencies).map(|resolved| {
                        let arc = Arc::new(WeaverResolvedSchema::V2(resolved));
                        if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                            self.record_cached_schema(url, arc.clone());
                        }
                        arc
                    });
                    return if !v2_warnings.is_empty() {
                        res.extend_non_fatal_errors(v2_warnings)
                    } else {
                        res
                    };
                }

                let (resolved, mut all_nfes) =
                    match v1::resolve_registry_internal(self, repo, specs, dependencies) {
                        WResult::Ok(resolved) => (resolved, Vec::new()),
                        WResult::OkWithNFEs(resolved, nfes) => (resolved, nfes),
                        WResult::FatalErr(e) => return WResult::FatalErr(e),
                    };

                let bundle = match self.finalize_v1_bundle(resolved, is_root) {
                    Ok(b) => b,
                    Err(e) => return WResult::FatalErr(e),
                };
                let arc = Arc::new(bundle);
                if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                    self.record_cached_schema(url, arc.clone());
                }
                all_nfes.extend(v2_warnings);
                WResult::with_non_fatal_errors(arc, all_nfes)
            }
            LoadedSemconvRegistry::Resolved {
                schema,
                direct_dependencies,
            } => {
                let bundle = match self.finalize_v1_bundle(schema, is_root) {
                    Ok(b) => b,
                    Err(e) => return WResult::FatalErr(e),
                };
                let arc = Arc::new(bundle);
                if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                    self.record_direct_dependencies(&url, direct_dependencies);
                    self.record_cached_schema(url, arc.clone());
                }
                WResult::Ok(arc)
            }
            LoadedSemconvRegistry::ResolvedV2 {
                schema,
                direct_dependencies,
            } => {
                let arc = Arc::new(WeaverResolvedSchema::V2(schema));
                if let Ok(url) = SchemaUrl::try_from(arc.schema_url_str()) {
                    self.record_direct_dependencies(&url, direct_dependencies);
                    self.record_cached_schema(url, arc.clone());
                }
                WResult::Ok(arc)
            }
        }
    }

    /// Builds an offline [`ResolvedSchemaCache`] snapshot for `schema` and its linearized dependencies.
    ///
    /// Evicted dependencies that were loaded in this resolver are re-resolved; transitive dependencies
    /// known only by URL through published packages are left absent without network fetching.
    pub fn schema_cache_for(
        &mut self,
        schema: &WeaverResolvedSchema,
    ) -> WResult<ResolvedSchemaCache, Error> {
        let mut schemas = HashMap::new();
        let mut nfes = vec![];

        if let Ok(root_url) = SchemaUrl::try_from(schema.schema_url_str()) {
            let root_arc = self
                .active_or_cached_schema(&root_url)
                .unwrap_or_else(|| Arc::new(schema.clone()));
            let _ = schemas.insert(root_url, root_arc);
        }

        for dep_url in schema.dependencies() {
            if let Some(cached) = self.active_or_cached_schema(dep_url) {
                let _ = schemas.insert(dep_url.clone(), cached);
            } else if self.direct_dependencies.contains_key(dep_url)
                || self.config.schema_url_overrides.contains_key(dep_url)
            {
                match self.resolve_dependency_schema(dep_url) {
                    WResult::Ok(resolved) => {
                        let _ = schemas.insert(dep_url.clone(), resolved);
                    }
                    WResult::OkWithNFEs(resolved, errs) => {
                        let _ = schemas.insert(dep_url.clone(), resolved);
                        nfes.extend(errs);
                    }
                    WResult::FatalErr(e) => return WResult::FatalErr(e),
                }
            }
        }

        let cache = ResolvedSchemaCache {
            schemas,
            direct_dependencies: self.direct_dependencies.clone(),
        };

        WResult::with_non_fatal_errors(cache, nfes)
    }

    /// Resolves and caches `schema_url` on demand, returning the cached entry if already present.
    pub fn resolve_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        self.resolve_schema_internal(schema_url, true)
    }

    /// Resolves and caches a dependency `schema_url` without clearing the active resolution graph.
    pub(crate) fn resolve_dependency_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        self.resolve_schema_internal(schema_url, false)
    }

    /// Resolves `schema_url` from cache or loads and resolves its repository, clearing active
    /// graph state when `is_root` is `true`.
    fn resolve_schema_internal(
        &mut self,
        schema_url: &SchemaUrl,
        is_root: bool,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        if is_root {
            self.active_graph_schemas.clear();
        } else if let Some(cached) = self.active_graph_schemas.get(schema_url).cloned() {
            return WResult::Ok(cached);
        }
        if let Some(cached) = self.cache.get(schema_url).cloned() {
            let arc = match &*cached {
                WeaverResolvedSchema::V1(v1)
                    if is_root && self.config.target == ResolutionTarget::V2 =>
                {
                    match self.finalize_v1_bundle(v1.clone(), true) {
                        Ok(bundle) => Arc::new(bundle),
                        Err(e) => return WResult::FatalErr(e),
                    }
                }
                _ => cached,
            };
            self.record_cached_schema(schema_url.clone(), arc.clone());
            return WResult::Ok(arc);
        }

        let mut nfes = vec![];
        let repo = if let Some(override_path) = self.config.schema_url_overrides.get(schema_url) {
            match graph::loader::open_overridden_repo(
                schema_url,
                override_path,
                &self.config.auth,
                &mut nfes,
            ) {
                Ok(r) => r,
                Err(e) => return WResult::FatalErr(e),
            }
        } else {
            let dep = Dependency {
                schema_url: schema_url.clone(),
                registry_path: None,
            };
            match RegistryRepo::try_new_dependency_with_auth(&dep, &mut nfes, &self.config.auth) {
                Ok(r) => r,
                Err(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
            }
        };

        let (loaded, mut load_nfes) = match self.load_repository(repo) {
            WResult::Ok(loaded) => (loaded, vec![]),
            WResult::OkWithNFEs(loaded, l_nfes) => (loaded, l_nfes),
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        };

        load_nfes.extend(nfes.into_iter().map(Error::from));

        let res = self.resolve_loaded_internal(loaded, is_root);
        match res {
            WResult::Ok(arc) => {
                if arc.schema_url_str() != schema_url.as_str() {
                    return WResult::FatalErr(Error::MismatchSchemaUrl {
                        expected: schema_url.as_str().to_owned(),
                        actual: arc.schema_url_str().to_owned(),
                    });
                }
                self.record_cached_schema(schema_url.clone(), arc.clone());
                WResult::with_non_fatal_errors(arc, load_nfes)
            }
            WResult::OkWithNFEs(arc, res_nfes) => {
                if arc.schema_url_str() != schema_url.as_str() {
                    return WResult::FatalErr(Error::MismatchSchemaUrl {
                        expected: schema_url.as_str().to_owned(),
                        actual: arc.schema_url_str().to_owned(),
                    });
                }
                self.record_cached_schema(schema_url.clone(), arc.clone());
                load_nfes.extend(res_nfes);
                WResult::OkWithNFEs(arc, load_nfes)
            }
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    /// Loads and resolves a semantic convention repository directly from disk or virtual directory.
    pub fn load_and_resolve_schema<V: SchemaLoadingVisitor>(
        &mut self,
        registry_repo: RegistryRepo,
        mut visitor: V,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let (loaded, mut load_nfes) = match self.load_repository(registry_repo) {
            WResult::Ok(loaded) => (loaded, vec![]),
            WResult::OkWithNFEs(loaded, nfes) => (loaded, nfes),
            WResult::FatalErr(e) => return WResult::FatalErr(e),
        };

        if !visitor.check_raw_loaded_schema_files(&loaded) {
            return WResult::FatalErr(Error::LoadingAbortedByVisitor);
        }

        match self.resolve_loaded(loaded) {
            WResult::Ok(arc) => {
                let owned = Arc::unwrap_or_clone(arc);
                WResult::with_non_fatal_errors(owned, load_nfes)
            }
            WResult::OkWithNFEs(arc, res_nfes) => {
                load_nfes.extend(res_nfes);
                WResult::OkWithNFEs(Arc::unwrap_or_clone(arc), load_nfes)
            }
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }
}

/// Trait for resolving schemas on demand from cache or remote repositories.
pub trait SchemaResolver {
    /// Resolves a schema by its SchemaUrl, returning the resolved bundle along with any non-fatal errors.
    fn resolve_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error>;

    /// Returns the direct dependencies of `schema_url`, or `None` when pre-resolved schemas do not
    /// distinguish direct from transitive dependencies.
    fn direct_dependencies(&self, schema_url: &SchemaUrl) -> Option<&[SchemaUrl]> {
        let _ = schema_url;
        None
    }
}

impl SchemaResolver for WeaverResolver {
    fn resolve_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        self.resolve_schema(schema_url)
    }

    fn direct_dependencies(&self, schema_url: &SchemaUrl) -> Option<&[SchemaUrl]> {
        self.direct_dependencies
            .get(schema_url)
            .map(|urls| urls.as_slice())
    }
}

/// A no-op schema resolver that returns a fatal error indicating schema URL resolution failure.
#[derive(Debug, Clone, Default)]
pub struct NullSchemaResolver;

impl SchemaResolver for NullSchemaResolver {
    fn resolve_schema(
        &mut self,
        _schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        WResult::FatalErr(Error::FailToResolveSchemaUrl {})
    }
}

/// A warm, read-only snapshot of resolved schemas produced during graph resolution.
///
/// Materializes dependencies on demand for consumers (such as forge template generation
/// or V1->V2 schema conversion) without performing network fetches.
#[derive(Debug, Clone)]
pub struct ResolvedSchemaCache {
    schemas: HashMap<SchemaUrl, Arc<WeaverResolvedSchema>>,
    direct_dependencies: HashMap<SchemaUrl, Vec<SchemaUrl>>,
}

impl ResolvedSchemaCache {
    /// Creates a new resolved schema cache from materialized schemas and direct dependency mappings.
    #[must_use]
    pub fn new(
        schemas: HashMap<SchemaUrl, Arc<WeaverResolvedSchema>>,
        direct_dependencies: HashMap<SchemaUrl, Vec<SchemaUrl>>,
    ) -> Self {
        Self {
            schemas,
            direct_dependencies,
        }
    }

    /// Returns a resolved schema reference if present in the cache.
    #[must_use]
    pub fn get(&self, schema_url: &SchemaUrl) -> Option<&Arc<WeaverResolvedSchema>> {
        self.schemas.get(schema_url)
    }

    /// Returns the direct dependencies of a schema, if known.
    #[must_use]
    pub fn direct_dependencies(&self, schema_url: &SchemaUrl) -> Option<&[SchemaUrl]> {
        self.direct_dependencies
            .get(schema_url)
            .map(|urls| urls.as_slice())
    }
}

impl SchemaResolver for ResolvedSchemaCache {
    fn resolve_schema(
        &mut self,
        schema_url: &SchemaUrl,
    ) -> WResult<Arc<WeaverResolvedSchema>, Error> {
        match self.schemas.get(schema_url) {
            Some(schema) => WResult::Ok(schema.clone()),
            None => WResult::FatalErr(Error::SchemaNotInCache {
                schema_url: schema_url.clone(),
            }),
        }
    }

    fn direct_dependencies(&self, schema_url: &SchemaUrl) -> Option<&[SchemaUrl]> {
        self.direct_dependencies
            .get(schema_url)
            .map(|urls| urls.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v1_target_on_mixed_registry_emits_no_warning() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-3-both-version/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V1,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(_) => {}
            WResult::OkWithNFEs(_, nfes) => {
                for nfe in &nfes {
                    assert!(!matches!(nfe, Error::MixedRegistryUsesV1Path { .. }));
                }
            }
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }
    }

    #[test]
    fn test_v2_target_on_mixed_registry_emits_warning() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-3-both-version/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(_) => panic!("Expected MixedRegistryUsesV1Path warning"),
            WResult::OkWithNFEs(resolved, nfes) => {
                assert!(matches!(resolved, WeaverResolvedSchema::V2(_)));
                let mixed_warns: Vec<_> = nfes
                    .iter()
                    .filter(|e| matches!(e, Error::MixedRegistryUsesV1Path { .. }))
                    .collect();
                assert_eq!(
                    mixed_warns.len(),
                    1,
                    "Expected 1 MixedRegistryUsesV1Path warning, got: {nfes:?}"
                );
            }
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }
    }

    #[test]
    fn test_v2_target_with_v1_dependency_emits_dep_warning_deduplicated() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/v1_dep_registry/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(_) => panic!("Expected V1DependencyUsesV1Path warning"),
            WResult::OkWithNFEs(_, nfes) => {
                let dep_warns: Vec<_> = nfes
                    .iter()
                    .filter_map(|e| match e {
                        Error::V1DependencyUsesV1Path { schema_url, format } => {
                            Some((schema_url.clone(), *format))
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    dep_warns.len(),
                    2,
                    "Expected 2 V1DependencyUsesV1Path warnings, got: {nfes:?}"
                );
                assert!(dep_warns
                    .iter()
                    .any(|(url, fmt)| url == "https://acme.com/schemas/0.1.0"
                        && *fmt == SourceFormat::V1));
                assert!(dep_warns.iter().any(|(url, fmt)| url
                    == "https://opentelemetry.io/schemas/1.30.0"
                    && *fmt == SourceFormat::V1));
            }
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }

        let dep_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let dep_repo = RegistryRepo::try_new(
            Some(SchemaUrl::try_from("https://acme.com/schemas/0.1.0").unwrap()),
            &dep_path,
            &mut vec![],
        )
        .expect("Failed to create dependency RegistryRepo");
        let loaded = resolver
            .load_repository(dep_repo)
            .into_result_failing_non_fatal()
            .unwrap();
        match resolver.resolve_dependency(loaded) {
            WResult::Ok(_) => {}
            WResult::OkWithNFEs(_, nfes) => {
                for nfe in &nfes {
                    assert!(
                        !matches!(nfe, Error::V1DependencyUsesV1Path { .. }),
                        "Unexpected duplicate V1DependencyUsesV1Path warning: {nfe:?}"
                    );
                }
            }
            WResult::FatalErr(e) => panic!("Dependency resolution failed: {e}"),
        }
    }

    #[test]
    fn test_v2_target_pure_v2_with_v2_dependencies_emits_no_v1_path_warnings() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/metric_registry/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(_) => {}
            WResult::OkWithNFEs(_, nfes) => {
                for nfe in &nfes {
                    assert!(
                        !matches!(
                            nfe,
                            Error::MixedRegistryUsesV1Path { .. }
                                | Error::V1DependencyUsesV1Path { .. }
                        ),
                        "Unexpected warning: {nfe:?}"
                    );
                }
            }
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }
    }

    #[test]
    fn test_v2_native_engine_resolves_group_import_registry() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/group_import_registry/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(resolved) => {
                assert_eq!(
                    resolved.schema_url_str(),
                    "https://group-import-consumer.example.com/schemas/1.0.0"
                );
            }
            WResult::OkWithNFEs(resolved, nfes) => {
                for nfe in &nfes {
                    assert!(
                        matches!(
                            nfe,
                            Error::FailToResolveDefinition(
                                weaver_semconv::Error::UnstableFileFormat { .. }
                            )
                        ),
                        "Unexpected NFE: {nfe:?}"
                    );
                }
                assert_eq!(
                    resolved.schema_url_str(),
                    "https://group-import-consumer.example.com/schemas/1.0.0"
                );
            }
            WResult::FatalErr(e) => {
                panic!(
                    "Expected native V2 engine to resolve group_import_registry, got FatalErr({e})"
                );
            }
        }
    }

    #[test]
    fn test_v2_native_engine_resolves_base_v2_registry() {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-1-everything/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(resolved) => {
                let v2 = resolved.as_v2().expect("Must resolve to V2Schema");
                assert_eq!(v2.file_format, "resolved/2.0");
                assert_eq!(v2.registry.metrics.len(), 1);
                assert_eq!(
                    &*v2.registry.metrics[0].name,
                    "http.server.request.duration"
                );
            }
            WResult::OkWithNFEs(resolved, nfes) => {
                let v2 = resolved.as_v2().expect("Must resolve to V2Schema");
                assert_eq!(v2.file_format, "resolved/2.0");
                assert_eq!(v2.registry.metrics.len(), 1);
                assert_eq!(
                    &*v2.registry.metrics[0].name,
                    "http.server.request.duration"
                );
                for nfe in &nfes {
                    assert!(
                        matches!(
                            nfe,
                            Error::FailToResolveDefinition(
                                weaver_semconv::Error::UnstableFileFormat { .. }
                            )
                        ),
                        "Unexpected NFE: {nfe:?}"
                    );
                }
            }
            WResult::FatalErr(e) => panic!("Native resolution failed: {e}"),
        }
    }

    #[test]
    fn test_pure_v2_never_calls_v1_adapter() {
        let _guard = v2::DisallowV1AdapterScope::new();
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-1-everything/registry".to_owned(),
        };
        let mut repo_nfes = vec![];
        let repo = RegistryRepo::try_new(None, &registry_path, &mut repo_nfes)
            .expect("Failed to create RegistryRepo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(resolved) | WResult::OkWithNFEs(resolved, _) => {
                let v2 = resolved.as_v2().expect("Must resolve to V2Schema");
                assert_eq!(v2.file_format, "resolved/2.0");
            }
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }
    }

    #[test]
    fn test_mixed_root_with_v1_dep_does_not_emit_v1_dependency_warning() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dep_dir = temp.path().join("dep");
        let root_dir = temp.path().join("root");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::create_dir_all(&root_dir).unwrap();

        std::fs::write(
            dep_dir.join("registry_manifest.yaml"),
            "name: dep\nversion: 1.0.0\nsemconv_version: 1.0.0\nschema_base_url: https://example.com/dep\n",
        )
        .unwrap();
        std::fs::write(
            dep_dir.join("attrs.yaml"),
            "groups:\n  - id: registry.dep\n    type: attribute_group\n    brief: dep attrs\n    attributes:\n      - id: dep.attr\n        type: string\n        brief: dep attr\n        stability: stable\n",
        )
        .unwrap();

        let dep_path_str = dep_dir.to_string_lossy().replace('\\', "/");
        std::fs::write(
            root_dir.join("manifest.yaml"),
            format!(
                "description: root\nschema_url: https://example.com/root/1.0.0\ndependencies:\n  - schema_url: https://example.com/dep/1.0.0\n    registry_path: {dep_path_str}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root_dir.join("v1.yaml"),
            "groups:\n  - id: registry.root.v1\n    type: attribute_group\n    brief: v1\n    attributes:\n      - id: root.v1.attr\n        type: string\n        brief: v1 attr\n        stability: stable\n",
        )
        .unwrap();
        std::fs::write(
            root_dir.join("v2.yaml"),
            "file_format: definition/2\nattributes:\n  - key: root.v2.attr\n    type: string\n    brief: v2 attr\n    stability: stable\n",
        )
        .unwrap();

        let repo = RegistryRepo::try_new(
            None,
            &VirtualDirectoryPath::LocalFolder {
                path: root_dir.to_string_lossy().into_owned(),
            },
            &mut vec![],
        )
        .unwrap();
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            ..Default::default()
        });
        match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::OkWithNFEs(_, nfes) => {
                assert!(
                    nfes.iter()
                        .any(|e| matches!(e, Error::MixedRegistryUsesV1Path { .. })),
                    "Expected MixedRegistryUsesV1Path warning, got: {nfes:?}"
                );
                assert!(
                    !nfes
                        .iter()
                        .any(|e| matches!(e, Error::V1DependencyUsesV1Path { .. })),
                    "Mixed root should not also emit V1DependencyUsesV1Path: {nfes:?}"
                );
            }
            WResult::Ok(_) => panic!("Expected OkWithNFEs with MixedRegistryUsesV1Path"),
            WResult::FatalErr(e) => panic!("Resolution failed: {e}"),
        }
    }

    #[test]
    fn test_schema_cache_for_keeps_published_leaf_and_resolves_overridden_transitive() {
        let temp = tempfile::tempdir().expect("tempdir");
        let leaf_dir = temp.path().join("leaf");
        let mid_dir = temp.path().join("mid");
        let root_dir = temp.path().join("root");
        std::fs::create_dir_all(&leaf_dir).unwrap();
        std::fs::create_dir_all(&mid_dir).unwrap();
        std::fs::create_dir_all(&root_dir).unwrap();

        let leaf_url = SchemaUrl::try_from("https://example.com/leaf/1.0.0").unwrap();
        let mid_url = SchemaUrl::try_from("https://example.com/mid/1.0.0").unwrap();

        std::fs::write(
            leaf_dir.join("manifest.yaml"),
            "file_format: manifest/2.0\nschema_url: https://example.com/leaf/1.0.0\ndescription: leaf\nstability: stable\nresolved_registry_uri: resolved.yaml\n",
        )
        .unwrap();
        std::fs::write(
            leaf_dir.join("resolved.yaml"),
            "file_format: resolved/2.0\nschema_url: https://example.com/leaf/1.0.0\nattribute_catalog: []\nregistry:\n  attributes: []\n  attribute_groups: []\n  entities: []\n  events: []\n  metrics: []\n  spans: []\nrefinements:\n  metrics: []\n  spans: []\n  events: []\ndependencies: []\n",
        )
        .unwrap();
        std::fs::write(
            mid_dir.join("manifest.yaml"),
            "file_format: manifest/2.0\nschema_url: https://example.com/mid/1.0.0\ndescription: mid\nstability: stable\nresolved_registry_uri: resolved.yaml\n",
        )
        .unwrap();
        std::fs::write(
            mid_dir.join("resolved.yaml"),
            "file_format: resolved/2.0\nschema_url: https://example.com/mid/1.0.0\nattribute_catalog: []\nregistry:\n  attributes: []\n  attribute_groups: []\n  entities: []\n  events: []\n  metrics: []\n  spans: []\nrefinements:\n  metrics: []\n  spans: []\n  events: []\ndependencies:\n  - https://example.com/leaf/1.0.0\n",
        )
        .unwrap();

        let mid_path_str = mid_dir.to_string_lossy().replace('\\', "/");
        std::fs::write(
            root_dir.join("manifest.yaml"),
            format!(
                "description: root\nschema_url: https://example.com/root/1.0.0\ndependencies:\n  - schema_url: https://example.com/mid/1.0.0\n    registry_path: {mid_path_str}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root_dir.join("attrs.yaml"),
            "file_format: definition/2\nattributes:\n  - key: root.attr\n    type: string\n    brief: root attr\n    stability: stable\n",
        )
        .unwrap();

        let mut overrides = BTreeMap::new();
        let _ = overrides.insert(
            leaf_url.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: leaf_dir.to_string_lossy().into_owned(),
            },
        );
        let repo = RegistryRepo::try_new(
            None,
            &VirtualDirectoryPath::LocalFolder {
                path: root_dir.to_string_lossy().into_owned(),
            },
            &mut vec![],
        )
        .unwrap();
        let mut resolver = WeaverResolver::new(WeaverResolverConfig {
            target: ResolutionTarget::V2,
            schema_url_overrides: overrides,
            ..Default::default()
        });
        let resolved = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("root resolution failed: {e}"),
        };

        let cache = resolver
            .schema_cache_for(&resolved)
            .into_result_failing_non_fatal()
            .expect("schema_cache_for succeeded");
        let root_url = SchemaUrl::try_from("https://example.com/root/1.0.0").unwrap();
        assert!(cache.get(&mid_url).is_some(), "mid should be in cache");
        assert!(
            cache.get(&leaf_url).is_some(),
            "overridden transitive leaf should be materialized in cache"
        );
        assert_eq!(
            cache.direct_dependencies(&root_url),
            Some(std::slice::from_ref(&mid_url)),
            "root should retain direct dependency edge to mid"
        );
    }
}
