// SPDX-License-Identifier: Apache-2.0

//! Dependency lookup types and traits for the V1 resolution algorithm.

use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::HashMap;
use std::sync::Arc;
use weaver_resolved_schema::v1::attribute::UnresolvedAttribute;
use weaver_resolved_schema::v1::registry::Group;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::entity_association::EntityAssociation;
use weaver_semconv::v1::group::{GroupType, InstrumentSpec, SpanKindSpec, SpanName};
use weaver_semconv::v1::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v1::stability::Stability;
use weaver_semconv::YamlValue;

#[cfg(test)]
use crate::graph::chosen_versions::collect_chosen_versions;
pub(crate) use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::WeaverResolvedSchema;

pub(crate) mod v1_dep;
pub(crate) mod v2_dep;

/// Origin of a looked-up group summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupSource {
    /// Defined in the registry currently being resolved.
    Local,
    /// Defined in a resolved dependency schema.
    Dependency,
}

/// Group metadata and unresolved attributes used during `extends` and `include_groups` resolution.
#[derive(Debug, Clone)]
pub(crate) struct GroupSummary {
    /// The type of the semantic convention.
    pub r#type: GroupType,
    /// The brief description of the semantic convention.
    pub brief: String,
    /// The note of the semantic convention.
    pub note: Option<String>,
    /// Specifies the stability of the semantic convention.
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated.
    pub deprecated: Option<Deprecated>,
    /// The metric name.
    pub metric_name: Option<String>,
    /// The event name or entity type name.
    pub name: Option<String>,
    /// The instrument type.
    pub instrument: Option<InstrumentSpec>,
    /// The unit.
    pub unit: Option<String>,
    /// The requirement level of the signal.
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Specifies the kind of the span.
    pub span_kind: Option<SpanKindSpec>,
    /// The v2 span name specification, inherited by refinements that do not
    /// override it.
    pub span_name: Option<SpanName>,
    /// The v2 span links, inherited by refinements that do not
    /// declare their own.
    pub span_links: Vec<weaver_semconv::v1::group::SpanLink>,
    /// The attributes from this group before being completely resolved to a catalog.
    pub attributes: Vec<UnresolvedAttribute>,
    /// The annotations of the group.
    pub annotations: Option<BTreeMap<String, YamlValue>>,
    /// The entity associations of the group.
    pub entity_associations: Vec<EntityAssociation>,
    /// The resolved origins of the entity associations when looked up from a dependency.
    pub association_origins: BTreeMap<String, SchemaUrl>,
    /// The origin schema URL of the group or signal.
    pub origin: Option<SchemaUrl>,
    /// Where this summary was looked up from.
    pub source: GroupSource,
}

impl GroupSummary {
    /// Builds a `GroupSummary` from `group` with an empty attribute list for callers to populate.
    pub(crate) fn from_without_attributes(group: &Group, source: GroupSource) -> Self {
        GroupSummary {
            r#type: group.r#type.clone(),
            brief: group.brief.clone(),
            note: group.note.clone(),
            stability: group.stability.clone(),
            deprecated: group.deprecated.clone(),
            metric_name: group.metric_name.clone(),
            name: group.name.clone(),
            instrument: group.instrument.clone(),
            unit: group.unit.clone(),
            requirement_level: group.requirement_level.clone(),
            span_kind: group.span_kind.clone(),
            span_name: group.span_name.clone(),
            span_links: group.span_links.clone(),
            attributes: vec![],
            annotations: group.annotations.clone(),
            entity_associations: group.entity_associations.clone(),
            association_origins: BTreeMap::new(),
            origin: group.provenance().map(|p| p.schema_url.clone()),
            source,
        }
    }
}

/// Resolved V1 or V2 dependency schema supporting group, attribute, and entity lookups without cloning.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedDependency(Arc<WeaverResolvedSchema>);

impl ResolvedDependency {
    /// Shares an existing `Arc<WeaverResolvedSchema>` as a `ResolvedDependency` without cloning.
    pub(crate) fn from_arc(arc: &Arc<WeaverResolvedSchema>) -> Self {
        Self(Arc::clone(arc))
    }

    /// Returns a reference to the underlying resolved schema bundle.
    pub(crate) fn as_schema(&self) -> &WeaverResolvedSchema {
        &self.0
    }

    /// Looks up a `GroupSummary` by group or signal `id`.
    pub(crate) fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.0.lookup_group_summary(id)
    }

    /// Looks up an entity definition or refinement matching `name`.
    pub(crate) fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        self.0.lookup_entity(name)
    }

    /// Records the latest compatible `SchemaUrl` per registry name from this dependency and its transitive dependencies.
    #[cfg(test)]
    pub(crate) fn collect_chosen_versions(&self, chosen: &mut HashMap<String, SchemaUrl>) {
        match self.0.as_ref() {
            WeaverResolvedSchema::V1(s) => {
                collect_chosen_versions(
                    SchemaUrl::try_from(s.schema_url.as_str()).ok().as_ref(),
                    &s.dependencies,
                    chosen,
                );
            }
            WeaverResolvedSchema::V2(s) => {
                collect_chosen_versions(Some(&s.schema_url), &s.dependencies, chosen);
            }
        }
    }
}

impl EntityLookup for ResolvedDependency {
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        self.lookup_entity(name)
    }
}

/// Looks up group summaries for `extends` refinement resolution across V1 and V2 schemas.
pub(crate) trait GroupRefinementLookup {
    /// Returns the `GroupSummary` for `id`, or `None` if not found.
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary>;
}

impl GroupRefinementLookup for WeaverResolvedSchema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        match self {
            WeaverResolvedSchema::V1(s) => s.lookup_group_summary(id),
            WeaverResolvedSchema::V2(s) => s.lookup_group_summary(id),
        }
    }
}

impl GroupRefinementLookup for Vec<ResolvedDependency> {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.iter().find_map(|d| d.lookup_group_summary(id))
    }
}

impl From<V1Schema> for ResolvedDependency {
    fn from(value: V1Schema) -> Self {
        Self(Arc::new(WeaverResolvedSchema::V1(value)))
    }
}

impl From<V2Schema> for ResolvedDependency {
    fn from(value: V2Schema) -> Self {
        Self(Arc::new(WeaverResolvedSchema::V2(value)))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use itertools::Itertools;
    use std::error::Error;

    use super::v1_dep::example_v1_schema;
    use super::v2_dep::example_v2_schema;
    use super::{GroupRefinementLookup, ResolvedDependency};

    #[test]
    fn test_lookup_group_summary() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::from(example_v1_schema());
        let result = d.lookup_group_summary("a");
        assert!(
            result.is_some(),
            "Should find group summary for `a` on {d:?}"
        );
        if let Some(summary) = result.as_ref() {
            assert!(
                !summary.attributes.is_empty(),
                "Should find attributes for group `a`, found none."
            );
            assert_eq!(summary.attributes[0].spec.id(), "a.test");
        }
        let ds = vec![d];
        let result2 = ds.lookup_group_summary("a");
        // Assert we get the same if we look across a vector vs. raw.
        assert_eq!(
            result.map(|a| a.attributes.iter().map(|a| a.spec.id()).collect_vec()),
            result2.map(|a| a.attributes.iter().map(|a| a.spec.id()).collect_vec())
        );
        Ok(())
    }

    #[test]
    fn test_lookup_group_summary_v2() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::from(example_v2_schema());

        let result_metric = d.lookup_group_summary("metric.a");
        assert!(result_metric.is_some(), "Should find metric.a");
        assert_eq!(
            result_metric.unwrap().r#type,
            weaver_semconv::v1::group::GroupType::Metric
        );

        let result_event = d.lookup_group_summary("event.b");
        assert!(result_event.is_some(), "Should find event.b");
        assert_eq!(
            result_event.unwrap().r#type,
            weaver_semconv::v1::group::GroupType::Event
        );

        let result_entity = d.lookup_group_summary("entity.c");
        assert!(result_entity.is_some(), "Should find entity.c");
        assert_eq!(
            result_entity.unwrap().r#type,
            weaver_semconv::v1::group::GroupType::Entity
        );

        let result_span = d.lookup_group_summary("span.d");
        assert!(result_span.is_some(), "Should find span.d");
        let span_summary = result_span.unwrap();
        assert_eq!(
            span_summary.r#type,
            weaver_semconv::v1::group::GroupType::Span
        );
        assert_eq!(
            span_summary.span_kind,
            Some(weaver_semconv::v1::group::SpanKindSpec::Client)
        );
        // The span name (with its note) is carried over so refinements that do
        // not override it inherit the dependency's definition.
        assert_eq!(
            span_summary.span_name,
            Some(weaver_semconv::v1::group::SpanName {
                templates: Vec::new(),
                note: Some("test".to_owned()),
            })
        );

        // Unknown ids resolve to nothing.
        assert!(
            d.lookup_group_summary("does.not.exist").is_none(),
            "Should not find an unknown group id"
        );

        Ok(())
    }
}
