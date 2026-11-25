// SPDX-License-Identifier: Apache-2.0

//! Semconv registry loader and cached repository access.

use itertools::Itertools;
use lru::LruCache;
use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Display};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use walkdir::{DirEntry, WalkDir};
use weaver_common::http_auth::HttpAuthResolver;
use weaver_common::result::WResult;
use weaver_common::vdir::{VirtualDirectory, VirtualDirectoryPath};
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::registry_repo::{is_semantic_convention_file, RegistryRepo};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::semconv::{SemConvSpecWithProvenance, Versioned};
use weaver_semconv::v2::manifest::Dependency;

use crate::graph::conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion};
use crate::{Error, WeaverResolvedSchema};

/// Maximum allowed depth for registry dependency chains.
const MAX_DEPENDENCY_DEPTH: u32 = 10;

/// The semantic convention source format of a registry specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SourceFormat {
    /// Pure V1 semantic convention specifications (`definition/1.x`).
    V1,
    /// Pure V2 semantic convention specifications (`definition/2.x`).
    V2,
    /// Mixed specifications containing both V1 and V2 files.
    Mixed,
}

impl Display for SourceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V1 => write!(f, "definition/1"),
            Self::V2 => write!(f, "definition/2"),
            Self::Mixed => write!(f, "mixed"),
        }
    }
}

/// The result of loading a semantic convention URL prior to resolution.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum LoadedSemconvRegistry {
    /// The semconv repository was unresolved and needs to be run through resolution.
    Unresolved {
        /// The specification of this raw repository.
        repo: RegistryRepo,
        /// The raw definition schema for this repository.
        specs: Vec<SemConvSpecWithProvenance>,
        /// The dependencies of this repository.
        dependencies: Vec<LoadedSemconvRegistry>,
    },
    /// A pre-resolved OpenTelemetry V1 schema (`ResolvedTelemetrySchema`) and its direct dependencies.
    Resolved {
        /// The resolved schema.
        schema: V1Schema,
        /// The direct dependencies its manifest declares, empty when it has no
        /// manifest.
        direct_dependencies: Vec<SchemaUrl>,
    },
    /// A pre-resolved OpenTelemetry V2 schema (`v2::ResolvedTelemetrySchema`) and its direct dependencies (e.g. from published `resolved/2.0` artifacts).
    ResolvedV2 {
        /// The resolved schema.
        schema: V2Schema,
        /// The direct dependencies its manifest declares, empty when it has no
        /// manifest.
        direct_dependencies: Vec<SchemaUrl>,
    },
}

impl LoadedSemconvRegistry {
    /// Creates a loaded semconv registry from a single string.
    #[cfg(test)]
    pub fn create_from_string(spec: &str) -> Result<LoadedSemconvRegistry, Error> {
        use std::io::Write;
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_semconv::schema_url::SchemaUrl;
        let path: VirtualDirectoryPath = "data".try_into().expect("Bad fake path for test");
        let repo =
            RegistryRepo::try_new(None, &path, &mut vec![]).map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("{e}"),
            })?;
        let mut temp_file =
            tempfile::NamedTempFile::with_suffix(".yaml").map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("Failed to create temp file: {e}"),
            })?;
        temp_file
            .write_all(spec.as_bytes())
            .map_err(|e| Error::InvalidUrl {
                url: "test string".to_owned(),
                error: format!("Failed to write to temp file: {e}"),
            })?;
        let spec_with_provenance =
            SemConvSpecWithProvenance::from_file(SchemaUrl::new_unknown(), temp_file.path())
                .ignore(|e| matches!(e, weaver_semconv::Error::UnstableFileFormat { .. }))
                .into_result_failing_non_fatal()
                .map_err(|e| Error::InvalidUrl {
                    url: "test string".to_owned(),
                    error: format!("{e}"),
                })?;
        Ok(LoadedSemconvRegistry::Unresolved {
            repo,
            specs: vec![spec_with_provenance],
            dependencies: vec![],
        })
    }

    /// Returns the semantic convention source format of this registry.
    ///
    /// For unresolved registries, this is determined by inspecting the format of each spec file:
    /// - `Some(SourceFormat::V1)` if all files are `definition/1.x`
    /// - `Some(SourceFormat::V2)` if all files are `definition/2.x`
    /// - `Some(SourceFormat::Mixed)` if both V1 and V2 files are present
    /// - `None` if there are 0 spec files
    ///
    /// For pre-resolved registries, `Resolved` is `Some(SourceFormat::V1)` and `ResolvedV2` is `Some(SourceFormat::V2)`.
    #[must_use]
    pub fn source_format(&self) -> Option<SourceFormat> {
        match self {
            Self::Unresolved { specs, .. } => {
                if specs.is_empty() {
                    return None;
                }
                let mut has_v1 = false;
                let mut has_v2 = false;
                for spec in specs {
                    match spec.spec {
                        Versioned::V1(_) => has_v1 = true,
                        Versioned::V2(_) => has_v2 = true,
                    }
                }
                match (has_v1, has_v2) {
                    (true, true) => Some(SourceFormat::Mixed),
                    (false, true) => Some(SourceFormat::V2),
                    (true, false) => Some(SourceFormat::V1),
                    (false, false) => None,
                }
            }
            Self::Resolved { .. } => Some(SourceFormat::V1),
            Self::ResolvedV2 { .. } => Some(SourceFormat::V2),
        }
    }

    /// Returns `true` if the repository has not yet been resolved.
    #[must_use]
    pub fn is_unresolved(&self) -> bool {
        matches!(self, LoadedSemconvRegistry::Unresolved { .. })
    }

    /// Returns the path or schema URL string identifying this registry.
    #[must_use]
    pub fn registry_path_repr(&self) -> &str {
        match self {
            LoadedSemconvRegistry::Unresolved { repo, .. } => repo.registry_path_repr(),
            // TODO - are these correct?
            LoadedSemconvRegistry::Resolved { schema, .. } => &schema.schema_url,
            LoadedSemconvRegistry::ResolvedV2 { schema, .. } => schema.schema_url.as_str(),
        }
    }

    /// Returns the depth of the dependency chain for this loaded repository.
    #[cfg(test)]
    #[must_use]
    pub fn dependency_depth(&self) -> u32 {
        match self {
            LoadedSemconvRegistry::Unresolved { dependencies, .. } => {
                1 + dependencies
                    .iter()
                    .map(|d| d.dependency_depth())
                    .max()
                    .unwrap_or_default()
            }
            LoadedSemconvRegistry::Resolved { .. } => 1,
            LoadedSemconvRegistry::ResolvedV2 { .. } => 1,
        }
    }

    /// Returns all the registry ids in this loaded registry and its dependencies.
    #[cfg(test)]
    #[must_use]
    pub fn registry_names(&self) -> Vec<String> {
        match self {
            LoadedSemconvRegistry::Unresolved {
                repo, dependencies, ..
            } => {
                let mut result = vec![repo.name().to_owned()];
                for d in dependencies {
                    result.extend(d.registry_names());
                }
                result
            }
            LoadedSemconvRegistry::Resolved { schema, .. } => vec![schema.registry_id.to_owned()],
            LoadedSemconvRegistry::ResolvedV2 { schema, .. } => {
                vec![schema.schema_url.name().to_owned()]
            }
        }
    }
}

impl Display for LoadedSemconvRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadedSemconvRegistry::Unresolved {
                repo,
                specs: _,
                dependencies,
            } => write!(
                f,
                "{} - [{}]",
                repo.schema_url(),
                dependencies.iter().map(|d| format!("{d}")).join(",")
            ),
            LoadedSemconvRegistry::Resolved { schema, .. } => write!(f, "{}", schema.schema_url),
            LoadedSemconvRegistry::ResolvedV2 { schema, .. } => write!(f, "{}", schema.schema_url),
        }
    }
}

/// Loads a semantic convention repository and its dependency graph without caching.
#[cfg(test)]
pub(crate) fn load_semconv_repository(
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    auth: &HttpAuthResolver,
) -> WResult<LoadedSemconvRegistry, Error> {
    load_semconv_repository_with_cache(None, registry_repo, follow_symlinks, auth, &BTreeMap::new())
}

/// Opens a dependency repository from an explicit override path and verifies its `SchemaUrl`.
pub(crate) fn open_overridden_repo(
    schema_url: &SchemaUrl,
    override_path: &VirtualDirectoryPath,
    auth: &HttpAuthResolver,
    nfes: &mut Vec<weaver_semconv::Error>,
) -> Result<RegistryRepo, Error> {
    let repo = RegistryRepo::try_new_with_auth(Some(schema_url.clone()), override_path, nfes, auth)
        .map_err(Error::FailToResolveDefinition)?;
    if repo.schema_url() != schema_url {
        return Err(Error::MismatchSchemaUrl {
            expected: schema_url.as_str().to_owned(),
            actual: repo.schema_url().as_str().to_owned(),
        });
    }
    Ok(repo)
}

/// Loads a semantic convention repository and its dependency graph, serving cached schemas and
/// applying URL overrides.
pub(crate) fn load_semconv_repository_with_cache(
    cache: Option<&LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>>,
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    auth: &HttpAuthResolver,
    schema_url_overrides: &BTreeMap<SchemaUrl, VirtualDirectoryPath>,
) -> WResult<LoadedSemconvRegistry, Error> {
    let mut visited_registries = HashSet::new();
    let mut chosen_versions = HashMap::new();
    let mut dependency_chain = Vec::new();

    let mut main_dependencies = HashMap::new();
    if let Some(manifest) = registry_repo.manifest() {
        for dep in manifest.dependencies().iter() {
            let _ =
                main_dependencies.insert(dep.schema_url.name().to_owned(), dep.schema_url.clone());
        }
    }

    let mut result = load_semconv_repository_recursive(
        cache,
        registry_repo,
        follow_symlinks,
        MAX_DEPENDENCY_DEPTH,
        &mut visited_registries,
        &mut chosen_versions,
        &mut dependency_chain,
        auth,
        schema_url_overrides,
    );

    if let WResult::Ok(_) | WResult::OkWithNFEs(_, _) = result {
        let mut warnings = vec![];
        for (name, main_url) in main_dependencies.iter() {
            if let Some(selected_url) = chosen_versions.get(name) {
                if selected_url != main_url {
                    warnings.push(Error::DependencyVersionUpgradedWarning {
                        dependency: name.clone(),
                        requested_version: main_url.version().to_owned(),
                        selected_version: selected_url.version().to_owned(),
                    });
                }
            }
        }
        if !warnings.is_empty() {
            result = result.extend_non_fatal_errors(warnings);
        }
    }
    result
}

/// Peeks the LRU cache for a pre-resolved schema matching `schema_url` without updating recency.
pub(crate) fn find_pre_resolved(
    cache: &LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>,
    schema_url: &SchemaUrl,
) -> Option<Arc<WeaverResolvedSchema>> {
    cache.peek(schema_url).cloned()
}

/// Recursively loads a repository and its dependencies while enforcing depth limits and cycle
/// detection.
fn load_semconv_repository_recursive(
    cache: Option<&LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>>,
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    max_dependency_depth: u32,
    visited_registries: &mut HashSet<String>,
    chosen_versions: &mut HashMap<String, SchemaUrl>,
    dependency_chain: &mut Vec<String>,
    auth: &HttpAuthResolver,
    schema_url_overrides: &BTreeMap<SchemaUrl, VirtualDirectoryPath>,
) -> WResult<LoadedSemconvRegistry, Error> {
    // Make sure we don't go past our max dependency depth.
    if max_dependency_depth == 0 {
        return WResult::FatalErr(Error::MaximumDependencyDepth {
            registry_name: registry_repo.registry_path_repr().to_owned(),
        });
    }
    let registry_name = registry_repo.name().to_owned();
    let schema_url = registry_repo.schema_url().clone();

    // Check for circular dependency in the current path
    if dependency_chain.contains(&registry_name) {
        dependency_chain.push(registry_name.clone());
        let chain_str = dependency_chain.join(" → ");
        return WResult::FatalErr(Error::CircularDependency {
            registry_name: registry_name.clone(),
            chain: chain_str,
        });
    }

    // Check for conflict across the graph
    if let Some(prev_schema_url) = chosen_versions.get(&registry_name) {
        if prev_schema_url != &schema_url {
            match UseLatestMajorVersion.resolve_conflict(prev_schema_url, &schema_url) {
                Ok(chosen) => {
                    let _ = chosen_versions.insert(registry_name.clone(), chosen);
                }
                Err(e) => return WResult::FatalErr(e),
            }
        }
    } else {
        let _ = chosen_versions.insert(registry_name.clone(), schema_url.clone());
    }

    // Also mark as visited for loading purposes.
    let _ = visited_registries.insert(registry_name.clone());

    // Add current registry to dependency chain
    dependency_chain.push(registry_name.clone());

    // The manifest is the only record of the direct dependencies of a schema that
    // arrives already resolved.
    let declared_dependencies: Vec<SchemaUrl> = registry_repo
        .manifest()
        .map(|m| {
            m.dependencies()
                .iter()
                .map(|d| d.schema_url.clone())
                .collect()
        })
        .unwrap_or_default();

    // Check for pre-resolved schema in cache.
    if let Some(cache) = cache {
        if let Some(arc) = find_pre_resolved(cache, &schema_url) {
            let loaded = match &*arc {
                WeaverResolvedSchema::V1(s) => LoadedSemconvRegistry::Resolved {
                    schema: s.clone(),
                    direct_dependencies: declared_dependencies,
                },
                WeaverResolvedSchema::V2(s) => LoadedSemconvRegistry::ResolvedV2 {
                    schema: s.clone(),
                    direct_dependencies: declared_dependencies,
                },
            };
            let _ = dependency_chain.pop();
            return WResult::Ok(loaded);
        }
    }

    // Either load a fully resolved repository, or read in raw files.
    if let Some(manifest) = registry_repo.manifest() {
        if let Some(resolved_url) = registry_repo.resolved_registry_uri() {
            let res = load_resolved_repository(&resolved_url, auth, declared_dependencies);

            // Register dependencies of the resolved schema for conflict resolution.
            if let WResult::Ok(LoadedSemconvRegistry::ResolvedV2 { ref schema, .. })
            | WResult::OkWithNFEs(LoadedSemconvRegistry::ResolvedV2 { ref schema, .. }, _) = res
            {
                for dep in schema.dependencies.iter() {
                    let dep_name = dep.name().to_owned();
                    if let Some(prev_schema_url) = chosen_versions.get(&dep_name) {
                        if prev_schema_url != dep {
                            match UseLatestMajorVersion.resolve_conflict(prev_schema_url, dep) {
                                Ok(chosen) => {
                                    let _ = chosen_versions.insert(dep_name, chosen);
                                }
                                Err(e) => return WResult::FatalErr(e),
                            }
                        }
                    } else {
                        let _ = chosen_versions.insert(dep_name, dep.clone());
                    }
                }
            }
            let _ = dependency_chain.pop();
            res
        } else {
            // Load dependencies.
            let mut loaded_dependencies = vec![];
            let mut non_fatal_errors: Vec<Error> = vec![];
            let mut seen_dependencies: HashMap<String, &Dependency> = HashMap::new();

            let dependencies = manifest.dependencies();
            for d in dependencies.iter() {
                let dep_name = d.schema_url.name().to_owned();
                if let Some(prev) = seen_dependencies.get(&dep_name) {
                    if let Err(e) = check_version_compatibility(prev, d) {
                        // Clean up the state of dependency_chain before erroring.
                        let _ = dependency_chain.pop();
                        return WResult::FatalErr(e);
                    }
                } else {
                    let _ = seen_dependencies.insert(dep_name, d);
                }
                let mut semconv_nfes: Vec<weaver_semconv::Error> = vec![];
                let d_repo_res =
                    if let Some(override_path) = schema_url_overrides.get(&d.schema_url) {
                        open_overridden_repo(&d.schema_url, override_path, auth, &mut semconv_nfes)
                    } else {
                        RegistryRepo::try_new_dependency_with_auth(d, &mut semconv_nfes, auth)
                            .map_err(Error::from)
                    };

                match d_repo_res {
                    Ok(d_repo) => {
                        non_fatal_errors
                            .extend(semconv_nfes.into_iter().map(Error::FailToResolveDefinition));
                        // so we need to make sure the dependency chain only include direct dependencies of each other.
                        match load_semconv_repository_recursive(
                            cache,
                            d_repo,
                            follow_symlinks,
                            max_dependency_depth - 1,
                            visited_registries,
                            chosen_versions,
                            dependency_chain,
                            auth,
                            schema_url_overrides,
                        ) {
                            WResult::Ok(d) => loaded_dependencies.push(d),
                            WResult::OkWithNFEs(d, nfes) => {
                                loaded_dependencies.push(d);
                                non_fatal_errors.extend(nfes);
                            }
                            WResult::FatalErr(err) => {
                                let _ = dependency_chain.pop();
                                return WResult::FatalErr(err);
                            }
                        }
                    }
                    Err(err) => {
                        let _ = dependency_chain.pop();
                        return WResult::FatalErr(err);
                    }
                }
            }
            // Now load the raw repository.
            // TODO - Allow ignoring dependency warnings - https://github.com/open-telemetry/weaver/issues/1126.
            let res =
                load_definition_repository(registry_repo, follow_symlinks, loaded_dependencies)
                    .extend_non_fatal_errors(non_fatal_errors);
            let _ = dependency_chain.pop();
            res
        }
    } else {
        // This is a raw repository with *no* manifest.
        // TODO - issue a warning that manifest will be required w/ 2.0 to allow publishing.
        let res = load_definition_repository(registry_repo, follow_symlinks, vec![]);
        let _ = dependency_chain.pop();
        res
    }
}

/// Loads a published V2 resolved schema and validates catalog and dependency indices.
fn load_resolved_repository(
    path: &VirtualDirectoryPath,
    auth: &HttpAuthResolver,
    direct_dependencies: Vec<SchemaUrl>,
) -> WResult<LoadedSemconvRegistry, Error> {
    let schema: V2Schema = match from_vdir(path, auth) {
        Ok(resolved) => resolved,
        Err(err) => return WResult::FatalErr(err),
    };

    let catalog_len = schema.attribute_catalog.len();
    let deps_len = schema.dependencies.len();

    let check_attr_ref = |idx: u32| -> Result<(), Error> {
        if idx as usize >= catalog_len {
            Err(Error::InvalidRegistryAttributeRef {
                registry_name: schema.schema_url.name().to_owned(),
                attribute_ref: idx,
            })
        } else {
            Ok(())
        }
    };

    let check_dep_ref = |dep_ref: DependencyRef| -> Result<(), Error> {
        if dep_ref.0 as usize >= deps_len {
            Err(Error::ConversionError {
                message: format!(
                    "Invalid registry: {}. DependencyRef({}) is out of bounds (dependencies count: {deps_len})",
                    schema.schema_url.name(),
                    dep_ref.0,
                ),
            })
        } else {
            Ok(())
        }
    };

    for aref in &schema.registry.attributes {
        if let Err(e) = check_attr_ref(aref.0) {
            return WResult::FatalErr(e);
        }
    }

    for group in &schema.registry.attribute_groups {
        if let Some(src) = group.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &group.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
    }

    for span in &schema.registry.spans {
        if let Some(src) = span.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &span.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for link in &span.links {
            if let Some(src) = link.provenance.source {
                if let Err(e) = check_dep_ref(src) {
                    return WResult::FatalErr(e);
                }
            }
            for attr in &link.attributes {
                if let Err(e) = check_attr_ref(attr.base.0) {
                    return WResult::FatalErr(e);
                }
            }
        }
        for assoc in &span.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for metric in &schema.registry.metrics {
        if let Some(src) = metric.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &metric.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for assoc in &metric.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for event in &schema.registry.events {
        if let Some(src) = event.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &event.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for assoc in &event.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for entity in &schema.registry.entities {
        if let Some(src) = entity.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for aref in &entity.identity {
            if let Err(e) = check_attr_ref(aref.0) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &entity.description {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
    }

    for span_ref in &schema.refinements.spans {
        if let Some(src) = span_ref.span.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &span_ref.span.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for link in &span_ref.span.links {
            if let Some(src) = link.provenance.source {
                if let Err(e) = check_dep_ref(src) {
                    return WResult::FatalErr(e);
                }
            }
            for attr in &link.attributes {
                if let Err(e) = check_attr_ref(attr.base.0) {
                    return WResult::FatalErr(e);
                }
            }
        }
        for assoc in &span_ref.span.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for metric_ref in &schema.refinements.metrics {
        if let Some(src) = metric_ref.metric.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &metric_ref.metric.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for assoc in &metric_ref.metric.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for event_ref in &schema.refinements.events {
        if let Some(src) = event_ref.event.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &event_ref.event.attributes {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
        for assoc in &event_ref.event.entity_associations {
            for eref in assoc.refs() {
                if let Some(src) = eref.provenance.source {
                    if let Err(e) = check_dep_ref(src) {
                        return WResult::FatalErr(e);
                    }
                }
            }
        }
    }

    for entity_ref in &schema.refinements.entities {
        if let Some(src) = entity_ref.entity.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
        for aref in &entity_ref.entity.identity {
            if let Err(e) = check_attr_ref(aref.0) {
                return WResult::FatalErr(e);
            }
        }
        for attr in &entity_ref.entity.description {
            if let Err(e) = check_attr_ref(attr.base.0) {
                return WResult::FatalErr(e);
            }
        }
    }

    for attr in &schema.attribute_catalog {
        if let Some(src) = attr.provenance.source {
            if let Err(e) = check_dep_ref(src) {
                return WResult::FatalErr(e);
            }
        }
    }

    WResult::Ok(LoadedSemconvRegistry::ResolvedV2 {
        schema,
        direct_dependencies,
    })
}

/// Deserializes a value from a file in the given virtual directory path.
fn from_vdir<T: DeserializeOwned>(
    f: &VirtualDirectoryPath,
    auth: &HttpAuthResolver,
) -> Result<T, Error> {
    let path = VirtualDirectory::try_new_with_auth(f, auth).map_err(|e| Error::InvalidUrl {
        url: f.to_string(),
        error: format!("Invalid weaver path reference: {e}"),
    })?;
    let file = File::open(path.path()).map_err(|_| Error::InvalidSchemaPath {
        path: path.path().to_path_buf(),
    })?;
    let reader = BufReader::new(file);
    serde_saphyr::from_reader(reader).map_err(|e| Error::ConversionError {
        message: format!("Unable to read resolved schema: {e}"),
    })
}

/// Loads raw semantic convention specification files in parallel from `registry_repo`.
fn load_definition_repository(
    registry_repo: RegistryRepo,
    follow_symlinks: bool,
    dependencies: Vec<LoadedSemconvRegistry>,
) -> WResult<LoadedSemconvRegistry, Error> {
    /// Returns `true` if `entry` has a dot-prefixed file name.
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with('.'))
    }
    let local_path = registry_repo.path().to_path_buf();

    let result = WalkDir::new(local_path)
        .follow_links(follow_symlinks)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .par_bridge()
        .flat_map(|entry| match entry {
            Ok(entry) => {
                if !is_semantic_convention_file(entry.path()) {
                    return vec![].into_par_iter();
                }

                let mapped_path = registry_repo.map_entry_path(entry.path());
                vec![SemConvSpecWithProvenance::from_file_with_mapped_path(
                    registry_repo.schema_url().clone(),
                    entry.path(),
                    move |_| mapped_path.clone(),
                )]
                .into_par_iter()
            }
            Err(e) => vec![WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                error: e.to_string(),
            })]
            .into_par_iter(),
        })
        .collect::<Vec<_>>();

    let mut non_fatal_errors = vec![];
    let mut specs = vec![];
    for r in result {
        match r {
            WResult::Ok(t) => specs.push(t),
            WResult::OkWithNFEs(t, nfes) => {
                specs.push(t);
                non_fatal_errors.extend(nfes);
            }
            WResult::FatalErr(e) => return WResult::FatalErr(Error::FailToResolveDefinition(e)),
        }
    }
    specs.sort_by(|a, b| a.provenance.path.cmp(&b.provenance.path));

    WResult::OkWithNFEs(
        LoadedSemconvRegistry::Unresolved {
            repo: registry_repo,
            specs,
            dependencies,
        },
        non_fatal_errors
            .into_iter()
            .map(Error::FailToResolveDefinition)
            .collect(),
    )
}

/// Verifies that two declarations of the same registry are both versioned and major-version
/// compatible.
fn check_version_compatibility(prev: &Dependency, dep: &Dependency) -> Result<(), Error> {
    if !prev.is_versioned() || !dep.is_versioned() {
        return Err(Error::UnversionedDependencyConflict {
            name: dep.schema_url.name().to_owned(),
        });
    }
    let _ = UseLatestMajorVersion.resolve_conflict(&prev.schema_url, &dep.schema_url)?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use weaver_common::{
        diagnostic::DiagnosticMessages, http_auth::HttpAuthResolver, result::WResult,
        vdir::VirtualDirectoryPath,
    };
    use weaver_semconv::registry_repo::RegistryRepo;

    #[test]
    fn test_load_unresolved_registry_with_dependencies() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut diag_msgs = DiagnosticMessages::empty();
        let loaded = load_semconv_repository(
            registry_repo,
            false,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        )
        .capture_non_fatal_errors(&mut diag_msgs)?;
        // Assert that we've loaded the ACME repository and the dependency of OTEL.
        if let LoadedSemconvRegistry::Unresolved {
            repo,
            specs,
            dependencies,
        } = loaded
        {
            assert_eq!("acme.com/schemas", repo.name());
            assert_eq!(dependencies.len(), 1);
            assert_eq!(specs.len(), 1);
            if let &[LoadedSemconvRegistry::Unresolved {
                repo: ref dep_repo,
                specs: ref dep_specs,
                dependencies: ref dep_deps,
            }] = &dependencies.as_slice()
            {
                assert_eq!("opentelemetry.io/schemas", dep_repo.name());
                assert_eq!(dep_deps.len(), 0);
                assert_eq!(dep_specs.len(), 1);
            } else {
                panic!("Failed to load unresolved registry dependency")
            }
        } else {
            panic!("Failed to load unresolved registry")
        }
        Ok(())
    }

    #[test]
    fn test_load_registry_from_relative_paths() -> Result<(), Error> {
        let registry = "data/registry-test-1-single-attr-ref/registry";
        for (path, dir) in [
            (format!("./{registry}/."), format!("./{registry}/./")),
            (
                format!("./{registry}/../registry"),
                format!("./{registry}/../registry/"),
            ),
            (format!("{registry}/"), format!("{registry}/")),
        ] {
            let registry_repo = RegistryRepo::try_new(
                None,
                &VirtualDirectoryPath::LocalFolder { path },
                &mut vec![],
            )?;
            let loaded = load_semconv_repository(
                registry_repo,
                false,
                &weaver_common::http_auth::HttpAuthResolver::empty(),
            )
            .into_result_failing_non_fatal()?;
            let LoadedSemconvRegistry::Unresolved { specs, .. } = loaded else {
                panic!("Failed to load unresolved registry")
            };
            let mut files: Vec<_> = specs.into_iter().map(|s| s.provenance.path).collect();
            files.sort_unstable();
            assert_eq!(
                files,
                ["metrics-messaging.yaml", "registry-messaging.yaml"].map(|f| format!("{dir}{f}"))
            );
        }
        Ok(())
    }

    #[test]
    fn test_depth_limit_enforcement() -> Result<(), weaver_semconv::Error> {
        // Test that depth limit is properly enforced by using internal method
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;

        // Try with depth limit of 1 - should fail at acme->otel transition
        let mut visited_registries = HashSet::new();
        let mut chosen_versions = HashMap::new();
        let mut dependency_chain = Vec::new();
        let result = load_semconv_repository_recursive(
            None,
            registry_repo,
            true,
            1,
            &mut visited_registries,
            &mut chosen_versions,
            &mut dependency_chain,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
            &BTreeMap::new(),
        );

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Maximum dependency depth reached"),
                    "Expected depth limit error, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to depth limit, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_two_unversioned_dependencies_under_the_same_name_are_fatal(
    ) -> Result<(), weaver_semconv::Error> {
        // Both dependencies are declared by `name: dep` and neither registry carries a
        // version, so they collapse onto the same placeholder schema URL. There is nothing
        // to reconcile them against, so this must fail rather than silently pick one.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/unversioned-conflict/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(
            registry_repo,
            true,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        );

        let WResult::FatalErr(fatal) = result else {
            panic!("Expected a fatal error for two unversioned declarations of 'dep'");
        };
        assert!(
            matches!(fatal, Error::UnversionedDependencyConflict { ref name, .. } if name == "dep"),
            "Expected an unversioned dependency conflict on 'dep', got: {fatal}"
        );

        Ok(())
    }

    #[test]
    fn test_circular_dependency_detection() -> Result<(), weaver_semconv::Error> {
        // Test circular dependency: registry_a -> registry_b -> registry_a
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/circular-registry-test/registry_a".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(
            registry_repo,
            true,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        );

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    error_msg.contains("Circular dependency detected") &&
                    error_msg.contains("registry_a") &&
                    error_msg.contains("registry_b"),
                    "Expected circular dependency error mentioning both registries, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to circular dependency, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_incompatible_version_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/incompatible-version-conflict/main".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(
            registry_repo,
            true,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        );

        match result {
            WResult::FatalErr(fatal) => {
                let error_msg = fatal.to_string();
                assert!(
                    (error_msg.contains("Incompatible dependency versions")
                        || error_msg.contains("Duplicate dependency"))
                        && error_msg.contains("example.com/c")
                        && error_msg.contains("1.0.0")
                        && error_msg.contains("2.0.0"),
                    "Expected incompatible dependency error mentioning both versions, got: {error_msg}"
                );
            }
            _ => {
                panic!("Expected fatal error due to duplicate dependency, but got success");
            }
        }

        Ok(())
    }

    #[test]
    fn test_compatible_version_conflict() -> Result<(), Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let result = load_semconv_repository(
            registry_repo,
            true,
            &weaver_common::http_auth::HttpAuthResolver::empty(),
        );

        let loaded = match result {
            WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
            WResult::FatalErr(fatal) => {
                panic!("Expected success, but got fatal error: {fatal}");
            }
        };

        // Resolve the registry
        let mut resolver = crate::WeaverResolver::new(crate::WeaverResolverConfig::default());
        let resolved_result = resolver.resolve_loaded(loaded);
        let resolved_schema = match resolved_result {
            WResult::Ok(s) | WResult::OkWithNFEs(s, _) => {
                Arc::unwrap_or_clone(s).into_v1().unwrap()
            }
            WResult::FatalErr(fatal) => {
                panic!("Failed to resolve registry: {fatal}");
            }
        };

        // Verify that attributes in the catalog use the V1.2 definitions.
        // c.attr1 should be from V1.2 (updated)
        let (attr1, _) = resolved_schema
            .catalog
            .root_attribute("c.attr1")
            .expect("c.attr1 not found in catalog");
        assert_eq!(attr1.brief, "Attribute 1 from C v1.2 (updated)");

        // c.attr2 should be from V1.2 (new)
        let (attr2, _) = resolved_schema
            .catalog
            .root_attribute("c.attr2")
            .expect("c.attr2 not found in catalog");
        assert_eq!(attr2.brief, "Attribute 2 from C v1.2 (new)");

        Ok(())
    }

    #[test]
    fn test_source_format() {
        use crate::graph::loader::SourceFormat;
        use weaver_common::vdir::VirtualDirectoryPath;

        let v1_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-1-single-attr-ref/registry".to_owned(),
        };
        let v1_repo = RegistryRepo::try_new(None, &v1_path, &mut vec![]).unwrap();
        let loaded_v1 =
            match load_semconv_repository(v1_repo.clone(), false, &HttpAuthResolver::empty()) {
                WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
                WResult::FatalErr(e) => panic!("Failed to load v1 repo: {e}"),
            };
        assert_eq!(loaded_v1.source_format(), Some(SourceFormat::V1));

        let v2_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-1-everything/registry".to_owned(),
        };
        let v2_repo = RegistryRepo::try_new(None, &v2_path, &mut vec![]).unwrap();
        let loaded_v2 = match load_semconv_repository(v2_repo, false, &HttpAuthResolver::empty()) {
            WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
            WResult::FatalErr(e) => panic!("Failed to load v2 repo: {e}"),
        };
        assert_eq!(loaded_v2.source_format(), Some(SourceFormat::V2));

        let mixed_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-3-both-version/registry".to_owned(),
        };
        let mixed_repo = RegistryRepo::try_new(None, &mixed_path, &mut vec![]).unwrap();
        let loaded_mixed =
            match load_semconv_repository(mixed_repo, false, &HttpAuthResolver::empty()) {
                WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
                WResult::FatalErr(e) => panic!("Failed to load mixed repo: {e}"),
            };
        assert_eq!(loaded_mixed.source_format(), Some(SourceFormat::Mixed));

        let resolved_v1 = LoadedSemconvRegistry::Resolved {
            schema: V1Schema {
                file_format: "resolved/1.0.0".to_owned(),
                schema_url: "http://example.com/v1".to_owned(),
                registry_id: "example".to_owned(),
                registry: weaver_resolved_schema::v1::registry::Registry {
                    registry_url: "http://example.com/v1".to_owned(),
                    entity_association_origins: Default::default(),
                    groups: vec![],
                },
                catalog: weaver_resolved_schema::v1::catalog::Catalog::new(
                    vec![],
                    Default::default(),
                ),
                resource: None,
                instrumentation_library: None,
                dependencies: Default::default(),
                versions: None,
                registry_manifest: None,
            },
            direct_dependencies: vec![],
        };
        assert_eq!(resolved_v1.source_format(), Some(SourceFormat::V1));

        let resolved_v2 = LoadedSemconvRegistry::ResolvedV2 {
            schema: V2Schema {
                file_format: "resolved/2.0".to_owned(),
                schema_url: "http://example.com/v2".try_into().unwrap(),
                registry: weaver_resolved_schema::v2::registry::Registry {
                    attributes: vec![],
                    attribute_groups: vec![],
                    metrics: vec![],
                    events: vec![],
                    spans: vec![],
                    entities: vec![],
                },
                attribute_catalog: vec![],
                refinements: weaver_resolved_schema::v2::refinements::Refinements {
                    spans: vec![],
                    metrics: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
            },
            direct_dependencies: vec![],
        };
        assert_eq!(resolved_v2.source_format(), Some(SourceFormat::V2));

        let empty_unresolved = LoadedSemconvRegistry::Unresolved {
            repo: v1_repo,
            specs: vec![],
            dependencies: vec![],
        };
        assert_eq!(empty_unresolved.source_format(), None);
    }

    #[test]
    fn test_load_resolved_repository_oob_link_attribute_ref_is_fatal() {
        let span = r#"{"type":"s","kind":"internal","name":{"note":"s"},"brief":"s","stability":"stable",
            "links":[{"ref":"s","attributes":[{"base":999,"requirement_level":"required"}]}]"#;
        for (spans, refinements) in [
            (format!("[{span}}}]"), "[]".to_owned()),
            ("[]".to_owned(), format!(r#"[{{"id":"r",{}}}]"#, &span[1..])),
        ] {
            let schema = format!(
                r#"{{"file_format":"resolved/2.0","schema_url":"https://example.com/v2","attribute_catalog":[],
                "registry":{{"attributes":[],"attribute_groups":[],"spans":{spans},"metrics":[],"events":[],"entities":[]}},
                "refinements":{{"spans":{refinements},"metrics":[],"events":[],"entities":[]}}}}"#
            );
            let temp_dir = tempfile::tempdir().unwrap();
            let file_path = temp_dir.path().join("schema.json");
            std::fs::write(&file_path, schema).unwrap();
            let path = VirtualDirectoryPath::LocalFolder {
                path: file_path.to_str().unwrap().to_owned(),
            };
            let auth = weaver_common::http_auth::HttpAuthResolver::empty();
            match load_resolved_repository(&path, &auth, vec![]) {
                WResult::FatalErr(Error::InvalidRegistryAttributeRef { attribute_ref, .. }) => {
                    assert_eq!(attribute_ref, 999);
                }
                WResult::FatalErr(e) => panic!("Expected InvalidRegistryAttributeRef, got {e:?}"),
                _ => panic!("Expected FatalErr, got Ok"),
            }
        }
    }

    #[test]
    fn test_load_resolved_repository_oob_attribute_ref_is_fatal() {
        let schema_with_oob_ref = V2Schema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: "https://example.com/v2".try_into().unwrap(),
            registry: weaver_resolved_schema::v2::registry::Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(999)],
                attribute_groups: vec![],
                metrics: vec![],
                events: vec![],
                spans: vec![],
                entities: vec![],
            },
            attribute_catalog: vec![],
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("schema.json");
        std::fs::write(
            &file_path,
            serde_json::to_string(&schema_with_oob_ref).unwrap(),
        )
        .unwrap();

        let path = VirtualDirectoryPath::LocalFolder {
            path: file_path.to_str().unwrap().to_owned(),
        };
        let auth = weaver_common::http_auth::HttpAuthResolver::empty();

        match load_resolved_repository(&path, &auth, vec![]) {
            WResult::FatalErr(Error::InvalidRegistryAttributeRef {
                registry_name,
                attribute_ref,
            }) => {
                assert_eq!(attribute_ref, 999);
                assert_eq!(registry_name, "example.com");
            }
            WResult::FatalErr(e) => panic!("Expected InvalidRegistryAttributeRef, got {e:?}"),
            _ => panic!("Expected FatalErr, got Ok"),
        }

        let dep = crate::v2::dependency::v2_dep::V2Dependency::new(Arc::new(schema_with_oob_ref));
        assert!(dep.lookup_attribute("nonexistent").unwrap().is_none());
        assert!(dep
            .lookup_signal(crate::v2::signals::SignalKind::Metric, "nonexistent")
            .is_none());
        assert!(dep.importable_spans().is_empty());
        assert!(dep.importable_metrics().is_empty());
        assert!(dep.importable_events().is_empty());
        assert!(dep.importable_entities().is_empty());
        assert!(dep.importable_attribute_groups().is_empty());
    }
}
