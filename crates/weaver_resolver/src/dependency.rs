// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.

use globset::GlobSet;
use serde::Deserialize;
use weaver_resolved_schema::attribute::UnresolvedAttribute;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::group::{GroupWildcard, ImportsWithProvenance};

use crate::{attribute::AttributeCatalog, Error};

/// A Resolved dependency, for which we can look up items.
#[derive(Debug, Deserialize)]
pub(crate) enum ResolvedDependency {
    /// A V1 Dependency
    V1(V1Schema),
    // A V2 Dependency
    V2(V2Schema),
}

impl ResolvedDependency {
    /// Creates unresolved attributes to fill out "ref" attributes when resolving a repository.
    pub(crate) fn lookup_group_attributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_group_attributes(id),
            ResolvedDependency::V2(schema) => schema.lookup_group_attributes(id),
        }
    }
}

/// Allows importing dependencies
pub(crate) trait ImportableDependency {
    /// Imports groups from the given dependency using the flags provided.
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
        attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error>;
}

impl ImportableDependency for V1Schema {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
        attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error> {
        let build_globset = |wildcards: Option<&Vec<GroupWildcard>>| {
            let mut builder = GlobSet::builder();
            if let Some(wildcards_vec) = wildcards {
                for wildcard in wildcards_vec.iter() {
                    _ = builder.add(wildcard.0.clone());
                }
            }
            builder.build().map_err(|e| Error::InvalidWildcard {
                error: e.to_string(),
            })
        };

        // Filter imports to only include those from the current registry
        let current_registry_imports: Vec<_> = imports.iter().collect();

        let metrics_imports_matcher = build_globset(
            current_registry_imports
                .iter()
                .find_map(|i| i.imports.metrics.as_ref()),
        )?;
        let events_imports_matcher = build_globset(
            current_registry_imports
                .iter()
                .find_map(|i| i.imports.events.as_ref()),
        )?;
        let entities_imports_matcher = build_globset(
            current_registry_imports
                .iter()
                .find_map(|i| i.imports.entities.as_ref()),
        )?;

        let filter = move |g: &Group| {
            include_all
                || match g.r#type {
                    // TODO - support importing attribute groups.
                    weaver_semconv::group::GroupType::AttributeGroup => false,
                    // TODO - support importing spans.
                    weaver_semconv::group::GroupType::Span => false,
                    weaver_semconv::group::GroupType::Event => g
                        .name
                        .as_ref()
                        .is_some_and(|name| events_imports_matcher.is_match(name.as_str())),
                    weaver_semconv::group::GroupType::Metric => {
                        g.metric_name.as_ref().is_some_and(|metric_name| {
                            metrics_imports_matcher.is_match(metric_name.as_str())
                        })
                    }
                    weaver_semconv::group::GroupType::MetricGroup => false,
                    weaver_semconv::group::GroupType::Entity => g
                        .name
                        .as_ref()
                        .is_some_and(|name| entities_imports_matcher.is_match(name.as_str())),
                    weaver_semconv::group::GroupType::Scope => false,
                    weaver_semconv::group::GroupType::Undefined => false,
                }
        };
        Ok(self
            .registry
            .groups
            .iter()
            .filter(|g| filter(g))
            .cloned()
            .map(|mut g| {
                // We need to fix all the attribute references in this group to be
                // against hte passed in attribute catalog.
                let mut attributes = vec![];
                for a in g
                    .attributes
                    .iter()
                    .filter_map(|ar| self.catalog().attribute(ar))
                {
                    let ar = attribute_catalog.attribute_ref(a.clone());
                    attributes.push(ar);
                }
                g.attributes = attributes;
                g
            })
            .collect())
    }
}

impl ImportableDependency for V2Schema {
    fn import_groups(
        &self,
        _imports: &[ImportsWithProvenance],
        _include_all: bool,
        _attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error> {
        todo!("Support V2 schema dependency resolution.")
    }
}

impl ImportableDependency for ResolvedDependency {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
        attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error> {
        match self {
            ResolvedDependency::V1(schema) => {
                schema.import_groups(imports, include_all, attribute_catalog)
            }
            ResolvedDependency::V2(schema) => {
                schema.import_groups(imports, include_all, attribute_catalog)
            }
        }
    }
}

// Allows importing across all dependencies.
impl ImportableDependency for Vec<ResolvedDependency> {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
        attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error> {
        self.iter()
            .map(|d| d.import_groups(imports, include_all, attribute_catalog))
            .try_fold(vec![], |mut result, next| {
                result.extend(next?);
                Ok(result)
            })
    }
}

/// Helper trait for abstracting over V1 and V2 schema.
trait UnresolvedAttributeLookup {
    /// Looks up group attributes on this repo.
    fn lookup_group_attributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>>;
}

impl UnresolvedAttributeLookup for V1Schema {
    fn lookup_group_attributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>> {
        self.group(id).map(|g| {
            let attributes: Vec<UnresolvedAttribute> = g
                .attributes
                .iter()
                .filter_map(|ar| self.catalog.attribute(ar))
                .map(|a| {
                    // TODO - we should include *chained* provenance from dependencies here.
                    UnresolvedAttribute {
                        spec: weaver_semconv::attribute::AttributeSpec::Id {
                            id: a.name.clone(),
                            r#type: a.r#type.clone(),
                            brief: Some(a.brief.clone()),
                            examples: a.examples.clone(),
                            tag: a.tag.clone(),
                            requirement_level: a.requirement_level.clone(),
                            sampling_relevant: a.sampling_relevant,
                            note: a.note.clone(),
                            stability: a.stability.clone(),
                            deprecated: a.deprecated.clone(),
                            annotations: a.annotations.clone(),
                            role: a.role.clone(),
                        },
                    }
                })
                .collect();
            attributes
        })
    }
}

impl UnresolvedAttributeLookup for V2Schema {
    fn lookup_group_attributes(&self, _id: &str) -> Option<Vec<UnresolvedAttribute>> {
        // TODO - we need to lookup on all possible groups.
        todo!("Support V2 in resolution")
    }
}

impl From<V1Schema> for ResolvedDependency {
    fn from(value: V1Schema) -> Self {
        ResolvedDependency::V1(value)
    }
}

impl From<V2Schema> for ResolvedDependency {
    fn from(value: V2Schema) -> Self {
        ResolvedDependency::V2(value)
    }
}
