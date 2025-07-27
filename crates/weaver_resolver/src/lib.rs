// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use miette::Diagnostic;
use std::collections::HashSet;
use std::path::{PathBuf, MAIN_SEPARATOR};
use weaver_common::log_error;

use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge};
use serde::Serialize;
use walkdir::DirEntry;

use crate::attribute::AttributeCatalog;
use crate::registry::resolve_semconv_registry;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_common::result::WResult;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::json_schema::JsonSchemaValidator;
use weaver_semconv::provenance::Provenance;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::registry_repo::{RegistryRepo, REGISTRY_MANIFEST};
use weaver_semconv::semconv::SemConvSpecWithProvenance;

pub mod attribute;
pub mod registry;

/// Maximum allowed depth for registry dependency chains.
const MAX_DEPENDENCY_DEPTH: u32 = 3;

/// A resolver that can be used to resolve telemetry schemas.
/// All references to semantic conventions will be resolved.
pub struct SchemaResolver {}

/// An error that can occur while resolving a telemetry schema.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    #[diagnostic(help("Check the URL and try again."))]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a set of attributes.
    #[error("Failed to resolve a set of attributes {ids:?}: {error}")]
    FailToResolveAttributes {
        /// The ids of the attributes.
        ids: Vec<String>,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a metric.
    #[error("Failed to resolve the metric '{ref}'")]
    FailToResolveMetric {
        /// The reference to the metric.
        r#ref: String,
    },

    /// Metric attributes are incompatible within the metric group.
    #[error("Metric attributes are incompatible within the metric group '{metric_group_ref}' for metric '{metric_ref}' (error: {error})")]
    IncompatibleMetricAttributes {
        /// The metric group reference.
        metric_group_ref: String,
        /// The reference to the metric.
        metric_ref: String,
        /// The error that occurred.
        error: String,
    },

    /// A generic conversion error.
    #[error("Conversion error: {message}")]
    ConversionError {
        /// The error that occurred.
        message: String,
    },

    /// An unresolved attribute reference.
    #[error("The following attribute reference is not resolved for the group '{group_id}'.\nAttribute reference: {attribute_ref}\nProvenance: {provenance}")]
    UnresolvedAttributeRef {
        /// The id of the group containing the attribute reference.
        group_id: String,
        /// The unresolved attribute reference.
        attribute_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An unresolved `extends` clause reference.
    #[error("The following `extends` clause reference is not resolved for the group '{group_id}'.\n`extends` clause reference: {extends_ref}\nProvenance: {provenance}")]
    UnresolvedExtendsRef {
        /// The id of the group containing the `extends` clause reference.
        group_id: String,
        /// The unresolved `extends` clause reference.
        extends_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An unresolved `include` reference.
    #[error("The following `include` reference is not resolved for the group '{group_id}'.\n`include` reference: {include_ref}\nProvenance: {provenance}")]
    UnresolvedIncludeRef {
        /// The id of the group containing the `include` reference.
        group_id: String,
        /// The unresolved `include` reference.
        include_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Provenance,
    },

    /// An invalid Schema path.
    #[error("Invalid Schema path: {path}")]
    InvalidSchemaPath {
        /// The schema path.
        path: PathBuf,
    },

    /// A duplicate group id error.
    #[error("The group id `{group_id}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupId {
        /// The group id.
        group_id: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate group id error.
    #[error("The group name `{group_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupName {
        /// The group name.
        group_name: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate group id error.
    #[error("The metric name `{metric_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateMetricName {
        /// The metric name.
        metric_name: String,
        /// The provenances where this metric name is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate attribute id error.
    #[error("The attribute id `{attribute_id}` is declared multiple times in the following groups:\n{group_ids:?}")]
    DuplicateAttributeId {
        /// The groups where this attribute is duplicated.
        group_ids: Vec<String>,
        /// The attribute id.
        attribute_id: String,
    },

    /// Invalid import wildcard.
    #[error("Invalid import wildcard: {error:?}")]
    #[diagnostic(help(
        "Check the wildcard syntax supported here: https://crates.io/crates/globset"
    ))]
    InvalidWildcard {
        /// The error that occurred.
        error: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            Error::CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

impl Error {
    /// Logs one or multiple errors (if current error is a 1CompoundError`)
    /// using the given logger.
    pub fn log(&self) {
        match self {
            Error::CompoundError(errors) => {
                for error in errors {
                    error.log();
                }
            }
            _ => log_error(self),
        }
    }
}

impl SchemaResolver {
    /// Resolves the given semantic convention registry and returns the
    /// corresponding resolved telemetry schema.
    pub fn resolve_semantic_convention_registry(
        registry: &mut SemConvRegistry,
        include_unreferenced: bool,
    ) -> WResult<ResolvedTelemetrySchema, Error> {
        let mut attr_catalog = AttributeCatalog::default();
        resolve_semconv_registry(&mut attr_catalog, "", registry, include_unreferenced).map(
            move |resolved_registry| {
                let catalog = Catalog::from_attributes(attr_catalog.drain_attributes());

                let resolved_schema = ResolvedTelemetrySchema {
                    file_format: "1.0.0".to_owned(),
                    schema_url: "".to_owned(),
                    registry_id: registry.id().into(),
                    registry: resolved_registry,
                    catalog,
                    resource: None,
                    instrumentation_library: None,
                    dependencies: vec![],
                    versions: None, // ToDo LQ: Implement this!
                    registry_manifest: registry.manifest().cloned(),
                };

                resolved_schema
            },
        )
    }

    /// Loads the semantic convention specifications from the given registry path.
    /// Implementation note: semconv files are read and parsed in parallel and
    /// all errors are collected and returned as a compound error.
    ///
    /// # Arguments
    /// * `registry_repo` - The registry repository containing the semantic convention files.
    /// * `allow_registry_deps` - Whether to allow registry dependencies.
    /// * `follow_symlinks` - Whether to follow symbolic links.
    pub fn load_semconv_specs(
        registry_repo: &RegistryRepo,
        allow_registry_deps: bool,
        follow_symlinks: bool,
    ) -> WResult<Vec<SemConvSpecWithProvenance>, weaver_semconv::Error> {
        let mut visited_registries = HashSet::new();
        let mut dependency_chain = Vec::new();
        Self::load_semconv_specs_with_depth(
            registry_repo, 
            allow_registry_deps, 
            follow_symlinks, 
            MAX_DEPENDENCY_DEPTH,
            &mut visited_registries,
            &mut dependency_chain
        )
    }

    fn load_semconv_specs_with_depth(
        registry_repo: &RegistryRepo,
        allow_registry_deps: bool,
        follow_symlinks: bool,
        max_dependency_depth: u32,
        visited_registries: &mut HashSet<String>,
        dependency_chain: &mut Vec<String>,
    ) -> WResult<Vec<SemConvSpecWithProvenance>, weaver_semconv::Error> {
        let registry_id = registry_repo.id().to_string();
        
        // Check for circular dependency
        if visited_registries.contains(&registry_id) {
            dependency_chain.push(registry_id.clone());
            let chain_str = dependency_chain.join(" → ");
            return WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                error: format!(
                    "Circular dependency detected: registry '{registry_id}' depends on itself through the chain: {chain_str}"
                ),
            });
        }
        
        // Add current registry to visited set and dependency chain
        let _ = visited_registries.insert(registry_id.clone());
        dependency_chain.push(registry_id.clone());
        // Define helper functions for filtering files.
        fn is_hidden(entry: &DirEntry) -> bool {
            entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        }
        fn is_semantic_convention_file(entry: &DirEntry) -> bool {
            let path = entry.path();
            let extension = path.extension().unwrap_or_else(|| std::ffi::OsStr::new(""));
            let file_name = path.file_name().unwrap_or_else(|| std::ffi::OsStr::new(""));
            path.is_file()
                && (extension == "yaml" || extension == "yml")
                && file_name != "schema-next.yaml"
                && file_name != REGISTRY_MANIFEST
        }

        let local_path = registry_repo.path().to_path_buf();
        let registry_path_repr = registry_repo.registry_path_repr();
        let validator = JsonSchemaValidator::new();

        // Loads the semantic convention specifications from the git repo.
        // All yaml files are recursively loaded and parsed in parallel from
        // the given path.
        let result = walkdir::WalkDir::new(local_path.clone())
            .follow_links(follow_symlinks)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .par_bridge()
            .flat_map(|entry| {
                match entry {
                    Ok(entry) => {
                        if !is_semantic_convention_file(&entry) {
                            return vec![].into_par_iter();
                        }

                        vec![SemConvRegistry::semconv_spec_from_file(
                            &registry_repo.id(),
                            entry.path(),
                            &validator,
                            |path| {
                                // Replace the local path with the git URL combined with the relative path
                                // of the semantic convention file.
                                let prefix = local_path
                                    .to_str()
                                    .map(|s| s.to_owned())
                                    .unwrap_or_default();
                                if registry_path_repr.ends_with(MAIN_SEPARATOR) {
                                    let relative_path = &path[prefix.len()..];
                                    format!("{registry_path_repr}{relative_path}")
                                } else {
                                    let relative_path = &path[prefix.len() + 1..];
                                    format!("{registry_path_repr}/{relative_path}")
                                }
                            },
                        )]
                        .into_par_iter()
                    }
                    Err(e) => vec![WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                        error: e.to_string(),
                    })]
                    .into_par_iter(),
                }
            })
            .collect::<Vec<_>>();

        let mut non_fatal_errors = vec![];
        let mut specs = vec![];

        // Process the registry dependencies (if any).
        if let Some(dep_result) =
            Self::process_registry_dependencies(registry_repo, allow_registry_deps, follow_symlinks, max_dependency_depth, visited_registries, dependency_chain)
        {
            match dep_result {
                WResult::Ok(t) => specs.extend(t),
                WResult::OkWithNFEs(t, nfes) => {
                    specs.extend(t);
                    non_fatal_errors.extend(nfes);
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        // Process all the results of the previous parallel processing.
        // The first fatal error will stop the processing and return the error.
        // Otherwise, all non-fatal errors will be collected and returned along
        // with the result.
        for r in result {
            match r {
                WResult::Ok(t) => specs.push(t),
                WResult::OkWithNFEs(t, nfes) => {
                    specs.push(t);
                    non_fatal_errors.extend(nfes);
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        WResult::OkWithNFEs(specs, non_fatal_errors)
    }

    fn process_registry_dependencies(
        registry_repo: &RegistryRepo,
        allow_registry_deps: bool,
        follow_symlinks: bool,
        max_dependency_depth: u32,
        visited_registries: &mut HashSet<String>,
        dependency_chain: &mut Vec<String>,
    ) -> Option<WResult<Vec<SemConvSpecWithProvenance>, weaver_semconv::Error>> {
        match registry_repo.manifest() {
            Some(manifest) => {
                if let Some(dependencies) = manifest
                    .dependencies
                    .as_ref()
                    .filter(|deps| !deps.is_empty())
                {
                    if !allow_registry_deps {
                        Some(WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                            error: format!(
                                "Registry dependencies are not allowed for the `{}` registry.",
                                registry_repo.registry_path_repr()
                            ),
                        }))
                    } else if max_dependency_depth == 0 {
                        Some(WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                            error: format!(
                                "Maximum dependency depth reached for registry `{}`. Cannot load further dependencies.",
                                registry_repo.registry_path_repr()
                            ),
                        }))
                    } else if dependencies.len() > 1 {
                        Some(WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                            error: format!(
                                "Currently, Weaver supports only a single dependency per registry. Multiple dependencies have been found in the `{}` registry.",
                                registry_repo.registry_path_repr()
                            ),
                        }))
                    } else {
                        let dependency = &dependencies[0];
                        match RegistryRepo::try_new(&dependency.name, &dependency.registry_path) {
                            Ok(registry_repo_dep) => Some(Self::load_semconv_specs_with_depth(
                                &registry_repo_dep,
                                true,
                                follow_symlinks,
                                max_dependency_depth - 1,
                                visited_registries,
                                dependency_chain,
                            )),
                            Err(e) => {
                                Some(WResult::FatalErr(weaver_semconv::Error::SemConvSpecError {
                                    error: format!(
                                        "Failed to load the registry dependency `{}`: {}",
                                        dependency.name, e
                                    ),
                                }))
                            }
                        }
                    }
                } else {
                    // Manifest has no dependencies or dependencies are empty
                    None
                }
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SchemaResolver;
    use std::collections::HashSet;
    use weaver_common::result::WResult;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::group::GroupType;
    use weaver_semconv::provenance::Provenance;
    use weaver_semconv::registry::SemConvRegistry;
    use weaver_semconv::registry_repo::RegistryRepo;
    use weaver_semconv::semconv::SemConvSpecWithProvenance;

    #[test]
    fn test_multi_registry() -> Result<(), weaver_semconv::Error> {
        fn check_semconv_specs(
            registry_repo: &RegistryRepo,
            semconv_specs: Vec<SemConvSpecWithProvenance>,
            include_unreferenced: bool,
        ) {
            assert_eq!(semconv_specs.len(), 2);
            for SemConvSpecWithProvenance {
                spec: semconv_spec,
                provenance: Provenance { registry_id, path },
            } in semconv_specs.iter()
            {
                match registry_id.as_ref() {
                    "acme" => {
                        assert_eq!(
                            path,
                            "data/multi-registry/custom_registry/custom_registry.yaml"
                        );
                        assert_eq!(semconv_spec.groups().len(), 2);
                        assert_eq!(&semconv_spec.groups()[0].id, "shared.attributes");
                        assert_eq!(&semconv_spec.groups()[1].id, "metric.auction.bid.count");
                        assert_eq!(
                            semconv_spec
                                .imports()
                                .unwrap()
                                .metrics
                                .as_ref()
                                .unwrap()
                                .len(),
                            1
                        );
                        assert_eq!(
                            semconv_spec
                                .imports()
                                .unwrap()
                                .events
                                .as_ref()
                                .unwrap()
                                .len(),
                            1
                        );
                        assert_eq!(
                            semconv_spec
                                .imports()
                                .unwrap()
                                .entities
                                .as_ref()
                                .unwrap()
                                .len(),
                            1
                        );
                    }
                    "otel" => {
                        assert_eq!(path, "data/multi-registry/otel_registry/otel_registry.yaml");
                        assert_eq!(semconv_spec.groups().len(), 7);
                        assert_eq!(&semconv_spec.groups()[0].id, "otel.registry");
                        assert_eq!(&semconv_spec.groups()[1].id, "otel.unused");
                        assert_eq!(&semconv_spec.groups()[2].id, "metric.example.counter");
                        assert_eq!(
                            &semconv_spec.groups()[3].id,
                            "entity.gcp.apphub.application"
                        );
                        assert_eq!(&semconv_spec.groups()[4].id, "entity.gcp.apphub.service");
                        assert_eq!(&semconv_spec.groups()[5].id, "event.session.start");
                        assert_eq!(&semconv_spec.groups()[6].id, "event.session.end");
                    }
                    _ => panic!("Unexpected registry id: {registry_id}"),
                }
            }

            let mut registry = SemConvRegistry::from_semconv_specs(registry_repo, semconv_specs)
                .expect("Failed to create the registry");
            match SchemaResolver::resolve_semantic_convention_registry(
                &mut registry,
                include_unreferenced,
            ) {
                WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                    if include_unreferenced {
                        // The group `otel.unused` shouldn't be garbage collected
                        let group = resolved_registry.group("otel.unused");
                        assert!(group.is_some());

                        // These groups are referenced in the `imports` and should not be garbage
                        // collected
                        let group = resolved_registry.group("metric.example.counter");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.application");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.service");
                        assert!(group.is_some());
                        let group = resolved_registry.group("event.session.start");
                        assert!(group.is_some());
                        let group = resolved_registry.group("event.session.end");
                        assert!(group.is_some());
                    } else {
                        // These groups should be garbage collected because they are not referenced
                        // anywhere (in ref or imports)
                        let group = resolved_registry.group("otel.unused");
                        assert!(group.is_none());
                        let group = resolved_registry.group("event.session.end");
                        assert!(group.is_none());

                        // These groups are referenced in the `imports` and should not be garbage
                        // collected
                        let group = resolved_registry.group("metric.example.counter");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.application");
                        assert!(group.is_some());
                        let group = resolved_registry.group("entity.gcp.apphub.service");
                        assert!(group.is_some());
                        let group = resolved_registry.group("event.session.start");
                        assert!(group.is_some());
                    }

                    let metrics = resolved_registry.groups(GroupType::Metric);
                    let metric = metrics
                        .get("metric.auction.bid.count")
                        .expect("Metric not found");
                    let attributes = &metric.attributes;
                    assert_eq!(attributes.len(), 3);
                    let mut attr_names = HashSet::new();
                    for attr_ref in attributes {
                        let attr = resolved_registry
                            .catalog
                            .attribute(attr_ref)
                            .expect("Failed to resolve attribute");
                        _ = attr_names.insert(attr.name.clone());
                        match attr.name.as_str() {
                            "auction.name" => {}
                            "auction.id" => {}
                            "error.type" => {
                                // Check requirement level is properly overridden.
                                // Initially, it was set to `recommended` in the otel registry.
                                // It should be overridden to `required` in the custom registry.
                                assert_eq!(
                                    attr.requirement_level,
                                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                                );
                            }
                            _ => {
                                panic!("Unexpected attribute name: {}", attr.name);
                            }
                        }
                    }
                    assert_eq!(metric.attributes.len(), 3);
                    assert!(attr_names.contains("auction.name"));
                    assert!(attr_names.contains("auction.id"));
                    assert!(attr_names.contains("error.type"));
                }
                WResult::FatalErr(fatal) => {
                    panic!("Fatal error: {fatal}");
                }
            }
        }

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("main", &registry_path)?;
        let result = SchemaResolver::load_semconv_specs(&registry_repo, true, true);
        match result {
            WResult::Ok(semconv_specs) => {
                // test with the `include_unreferenced` flag set to false
                check_semconv_specs(&registry_repo, semconv_specs.clone(), false);
                // test with the `include_unreferenced` flag set to true
                check_semconv_specs(&registry_repo, semconv_specs, true);
            }
            WResult::OkWithNFEs(semconv_specs, nfe) => {
                // test with the `include_unreferenced` flag set to false
                check_semconv_specs(&registry_repo, semconv_specs.clone(), false);
                // test with the `include_unreferenced` flag set to true
                check_semconv_specs(&registry_repo, semconv_specs, true);
                if !nfe.is_empty() {
                    panic!("Non-fatal errors: {nfe:?}");
                }
            }
            WResult::FatalErr(fatal) => {
                panic!("Fatal error: {fatal}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_three_registry_chain_works() -> Result<(), weaver_semconv::Error> {
        // Test the three-registry chain: app -> acme -> otel
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("app", &registry_path)?;
        let result = SchemaResolver::load_semconv_specs(&registry_repo, true, true);
        
        match result {
            WResult::Ok(semconv_specs) | WResult::OkWithNFEs(semconv_specs, _) => {
                // Should successfully load specs from all three registries
                assert!(semconv_specs.len() >= 3, "Expected specs from at least 3 registries, got {}", semconv_specs.len());
                
                // Verify we have specs from all three registries
                let registry_ids: Vec<&str> = semconv_specs.iter()
                    .map(|spec| spec.provenance.registry_id.as_ref())
                    .collect();
                
                assert!(registry_ids.contains(&"app"), "Missing app registry specs");
                assert!(registry_ids.contains(&"acme"), "Missing acme registry specs");
                assert!(registry_ids.contains(&"otel"), "Missing otel registry specs");

                // Now test the resolved registry content
                let mut registry = SemConvRegistry::from_semconv_specs(&registry_repo, semconv_specs)
                    .expect("Failed to create the registry");
                let resolved_result = SchemaResolver::resolve_semantic_convention_registry(&mut registry, false);
                
                match resolved_result {
                    WResult::Ok(resolved_registry) | WResult::OkWithNFEs(resolved_registry, _) => {
                        // Check that ONLY the app.example group exists (no imported groups should be in the resolved registry)
                        use weaver_semconv::group::GroupType;
                        let all_groups: Vec<String> = [GroupType::AttributeGroup, GroupType::Metric, GroupType::Event, GroupType::Span]
                            .iter()
                            .flat_map(|group_type| {
                                resolved_registry.groups(group_type.clone()).keys().map(|k| k.to_string()).collect::<Vec<_>>()
                            })
                            .collect();
                        
                        
                        // Should only have the app.example group, not any imported groups
                        assert_eq!(all_groups.len(), 1, 
                            "Expected only 1 group (app.example), but found {}: {:?}", all_groups.len(), all_groups);
                        assert!(all_groups.contains(&"app.example".to_owned()), 
                            "Missing app.example group, found: {all_groups:?}");
                        
                        // Check that app.example group exists and has exactly the expected attributes
                        let app_group = resolved_registry.group("app.example")
                            .expect("app.example group should exist");
                        
                        // Should have exactly 2 attributes: app.name (local) and error.type (from otel)
                        println!("app.example group has {} attributes", app_group.attributes.len());
                        
                        // Collect attribute names for verification
                        let mut attr_names = HashSet::new();
                        for attr_ref in &app_group.attributes {
                            let attr = resolved_registry.catalog.attribute(attr_ref)
                                .expect("Failed to resolve attribute");
                            println!("app.example attribute: {}", attr.name);
                            let _ = attr_names.insert(attr.name.clone());
                        }
                        
                        // Verify we have exactly the expected attributes
                        assert!(attr_names.contains("app.name"), "Missing app.name attribute");
                        assert!(attr_names.contains("error.type"), "Missing error.type attribute");
                        assert_eq!(attr_names.len(), 2, 
                            "Expected exactly 2 attributes (app.name, error.type), got: {attr_names:?}");
                    }
                    WResult::FatalErr(fatal) => {
                        panic!("Failed to resolve registry: {fatal}");
                    }
                }
            }
            WResult::FatalErr(fatal) => {
                panic!("Unexpected fatal error in three-registry chain: {fatal}");
            }
        }
        
        Ok(())
    }

    #[test]
    fn test_depth_limit_enforcement() -> Result<(), weaver_semconv::Error> {
        // Test that depth limit is properly enforced by using internal method
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("app", &registry_path)?;
        
        // Try with depth limit of 1 - should fail at acme->otel transition
        let mut visited_registries = HashSet::new();
        let mut dependency_chain = Vec::new();
        let result = SchemaResolver::load_semconv_specs_with_depth(&registry_repo, true, true, 1, &mut visited_registries, &mut dependency_chain);
        
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
    fn test_circular_dependency_detection() -> Result<(), weaver_semconv::Error> {
        // Test circular dependency: registry_a -> registry_b -> registry_a
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/circular-registry-test/registry_a".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new("registry_a", &registry_path)?;
        let result = SchemaResolver::load_semconv_specs(&registry_repo, true, true);
        
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
}
