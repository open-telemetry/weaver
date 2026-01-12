// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.

use serde::Deserialize;
use weaver_resolved_schema::attribute::UnresolvedAttribute;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::group::ImportsWithProvenance;

use crate::Error;

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
    pub(crate) fn lookup_group_atributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_group_atributes(id),
            ResolvedDependency::V2(schema) => schema.lookup_group_atributes(id),
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
    ) -> Result<Vec<Group>, Error>;
}

impl ImportableDependency for V1Schema {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
    ) -> Result<Vec<Group>, Error> {
        todo!()
    }
}

impl ImportableDependency for V2Schema {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
    ) -> Result<Vec<Group>, Error> {
        todo!()
    }
}

impl ImportableDependency for ResolvedDependency {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
    ) -> Result<Vec<Group>, Error> {
        match self {
            ResolvedDependency::V1(schema) => schema.import_groups(imports, include_all),
            ResolvedDependency::V2(schema) => schema.import_groups(imports, include_all),
        }
    }
}

// Allows importing across all dependencies.
impl ImportableDependency for Vec<ResolvedDependency> {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
    ) -> Result<Vec<Group>, Error> {
        self.iter()
            .map(|d| d.import_groups(imports, include_all))
            .try_fold(vec![], |mut result, next| {
                result.extend(next?);
                Ok(result)
            })
    }
}

/// Helper trait for abstracting over V1 and V2 schema.
trait UnresolvedAttributeLookup {
    /// Looks up group attributes on this repo.
    fn lookup_group_atributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>>;
}

impl UnresolvedAttributeLookup for V1Schema {
    fn lookup_group_atributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>> {
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
                            sampling_relevant: a.sampling_relevant.clone(),
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
    fn lookup_group_atributes(&self, id: &str) -> Option<Vec<UnresolvedAttribute>> {
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
