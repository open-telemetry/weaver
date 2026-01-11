// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.

use std::borrow::Cow;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;

/// A Resolved dependency, for which we can look up items.
pub(crate) enum ResolvedDependency {
    /// A V1 Dependency
    V1(V1Schema),
    // A V2 Dependency
    V2(V2Schema),
}

impl ResolvedDependency {
    /// Looks for a group on the resolved dependency.
    pub(crate) fn lookup_group<'a>(&'a self, id: &str) -> Option<Cow<'a, Group>> {
        match self {
            ResolvedDependency::V1(schema) => schema.group(id).map(Cow::Borrowed),
            ResolvedDependency::V2(schema) => todo!(),
        }
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
