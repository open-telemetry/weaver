// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.

use globset::GlobSet;
use serde::Deserialize;
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2Catalog;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::{attribute::UnresolvedAttribute, v2::Signal};
use weaver_semconv::attribute::{AttributeRole, RequirementLevel};
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::group::{GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::group::{GroupWildcard, ImportsWithProvenance};
use weaver_semconv::stability::Stability;

use crate::{attribute::AttributeCatalog, Error};

/// A summary of a group, used during refinement and extends resolution, along with its unresolved attributes.
#[derive(Debug, Clone)]
pub(crate) struct GroupSummary {
    /// The type of the semantic convention.
    pub r#type: GroupType,
    /// The brief description of the semantic convention.
    pub brief: String,
    /// The note of the semantic convention.
    pub note: String,
    /// Specifies the stability of the semantic convention.
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated.
    pub deprecated: Option<Deprecated>,
    /// The metric name.
    pub metric_name: Option<String>,
    /// The instrument type.
    pub instrument: Option<InstrumentSpec>,
    /// The unit.
    pub unit: Option<String>,
    /// Specifies the kind of the span.
    pub span_kind: Option<SpanKindSpec>,
    /// The attributes from this group before being completely resolved to a catalog.
    pub attributes: Vec<UnresolvedAttribute>,
    /// The annotations of the group.
    pub annotations: Option<std::collections::BTreeMap<String, weaver_semconv::YamlValue>>,
}

impl GroupSummary {
    /// Returns a group summary from this group.
    /// Does not include attributes because resolved Schema uses attribute refs,
    /// and this needs to fully resolve those attributes from the catalog.
    pub(crate) fn from_without_attributes(group: &Group) -> Self {
        GroupSummary {
            r#type: group.r#type.clone(),
            brief: group.brief.clone(),
            note: group.note.clone(),
            stability: group.stability.clone(),
            deprecated: group.deprecated.clone(),
            metric_name: group.metric_name.clone(),
            instrument: group.instrument.clone(),
            unit: group.unit.clone(),
            span_kind: group.span_kind.clone(),
            attributes: vec![], // Will be set during the dependency or registry loops.
            annotations: group.annotations.clone(),
        }
    }
}

/// A Resolved dependency, for which we can look up items.
#[derive(Debug, Deserialize)]
pub(crate) enum ResolvedDependency {
    /// A V1 Dependency
    V1(Box<V1Schema>),
    /// A V2 Dependency
    V2(Box<V2Schema>),
}

impl ResolvedDependency {
    /// Looks up a group summary on this dependency.
    pub(crate) fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_group_summary(id),
            ResolvedDependency::V2(schema) => schema.lookup_group_summary(id),
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
        let spans_imports_matcher = build_globset(
            current_registry_imports
                .iter()
                .find_map(|i| i.imports.spans.as_ref()),
        )?;
        let attribute_groups_imports_matcher = build_globset(
            current_registry_imports
                .iter()
                .find_map(|i| i.imports.attribute_groups.as_ref()),
        )?;

        let filter = move |g: &Group| {
            include_all
                || match g.r#type {
                    GroupType::AttributeGroup => attribute_groups_imports_matcher.is_match(&g.id),
                    GroupType::Span => spans_imports_matcher.is_match(&g.id),
                    GroupType::Event => g
                        .name
                        .as_ref()
                        .is_some_and(|name| events_imports_matcher.is_match(name.as_str())),
                    GroupType::Metric => g.metric_name.as_ref().is_some_and(|metric_name| {
                        metrics_imports_matcher.is_match(metric_name.as_str())
                    }),
                    GroupType::MetricGroup => false,
                    GroupType::Entity => g
                        .name
                        .as_ref()
                        .is_some_and(|name| entities_imports_matcher.is_match(name.as_str())),
                    GroupType::Scope => false,
                    GroupType::Undefined => false,
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
                // against the passed in attribute catalog.
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

/// Converts a V2 attribute (with no requirement level) to a v1 attribute.
fn convert_v2_attribute(
    attr: &weaver_resolved_schema::v2::attribute::Attribute,
    requirement_level: RequirementLevel,
    role: Option<AttributeRole>,
) -> Attribute {
    Attribute {
        name: attr.key.clone(),
        r#type: attr.r#type.clone(),
        brief: attr.common.brief.clone(),
        examples: attr.examples.clone(),
        tag: None,
        requirement_level,
        sampling_relevant: None,
        note: attr.common.note.clone(),
        stability: Some(attr.common.stability.clone()),
        deprecated: attr.common.deprecated.clone(),
        prefix: false,
        tags: None,
        annotations: Some(attr.common.annotations.clone()),
        value: None,
        role,
    }
}

impl ImportableDependency for V2Schema {
    fn import_groups(
        &self,
        imports: &[ImportsWithProvenance],
        include_all: bool,
        attribute_catalog: &mut AttributeCatalog,
    ) -> Result<Vec<Group>, Error> {
        let mut result = vec![];

        // First import metrics.  These are *by name* and come from the registry.
        // This is the closest to V1 ref syntax we have.
        let metrics_imports_matcher =
            build_globset(imports.iter().find_map(|i| i.imports.metrics.as_ref()))?;
        for m in self.registry.metrics.iter().filter(|m| {
            let metric_name: &str = &m.name;
            include_all || metrics_imports_matcher.is_match(metric_name)
        }) {
            let mut attributes = vec![];
            for ar in m.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    ar.requirement_level.clone(),
                    None,
                )));
            }
            result.push(Group {
                id: m.id().to_owned(),
                r#type: GroupType::Metric,
                brief: m.common.brief.clone(),
                note: m.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(m.common.stability.clone()),
                deprecated: m.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: Some(m.name.to_string()),
                instrument: Some(m.instrument.clone()),
                unit: Some(m.unit.clone()),
                name: None,
                // TODO - fill this out.
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(m.common.annotations.clone()),
                entity_associations: m.entity_associations.clone(),
                visibility: None,
                is_v2: true,
            });
        }

        // Now event imports.
        let events_imports_matcher =
            build_globset(imports.iter().find_map(|i| i.imports.events.as_ref()))?;
        for e in self.registry.events.iter().filter(|e| {
            let event_name: &str = &e.name;
            include_all || events_imports_matcher.is_match(event_name)
        }) {
            let mut attributes = vec![];
            for ar in e.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    ar.requirement_level.clone(),
                    None,
                )));
            }
            result.push(Group {
                id: e.id().to_owned(),
                r#type: GroupType::Event,
                brief: e.common.brief.clone(),
                note: e.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(e.common.stability.clone()),
                deprecated: e.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some(e.name.to_string()),
                // TODO - fill this out.
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(e.common.annotations.clone()),
                entity_associations: e.entity_associations.clone(),
                visibility: None,
                is_v2: true,
            });
        }

        // Now Entity imports.
        let entities_imports_matcher =
            build_globset(imports.iter().find_map(|i| i.imports.entities.as_ref()))?;
        for e in self.registry.entities.iter().filter(|e| {
            let entity_type: &str = &e.r#type;
            include_all || entities_imports_matcher.is_match(entity_type)
        }) {
            let mut attributes = vec![];
            for ar in e.identity.iter() {
                // TODO - this should be non-panic errors.
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    ar.requirement_level.clone(),
                    Some(AttributeRole::Identifying),
                )));
            }
            for ar in e.description.iter() {
                // TODO - this should be non-panic errors.
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    ar.requirement_level.clone(),
                    Some(AttributeRole::Descriptive),
                )));
            }
            result.push(Group {
                id: e.id().to_owned(),
                r#type: GroupType::Entity,
                brief: e.common.brief.clone(),
                note: e.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(e.common.stability.clone()),
                deprecated: e.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some(e.r#type.to_string()),
                // TODO - fill this out.
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(e.common.annotations.clone()),
                entity_associations: vec![],
                visibility: None,
                is_v2: true,
            });
        }

        // Now Span imports.
        let spans_imports_matcher =
            build_globset(imports.iter().find_map(|i| i.imports.spans.as_ref()))?;
        for s in self.registry.spans.iter().filter(|s| {
            let span_name: &str = &s.r#type;
            include_all || spans_imports_matcher.is_match(span_name)
        }) {
            let mut attributes = vec![];
            for ar in s.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    ar.requirement_level.clone(),
                    None,
                )));
            }
            result.push(Group {
                id: s.id().to_owned(),
                r#type: GroupType::Span,
                brief: s.common.brief.clone(),
                note: s.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(s.common.stability.clone()),
                deprecated: s.common.deprecated.clone(),
                attributes,
                span_kind: Some(s.kind.clone()),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some(s.r#type.to_string()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(s.common.annotations.clone()),
                entity_associations: s.entity_associations.clone(),
                visibility: None,
                is_v2: true,
            });
        }

        // Now AttributeGroup imports.
        let attribute_groups_imports_matcher = build_globset(
            imports
                .iter()
                .find_map(|i| i.imports.attribute_groups.as_ref()),
        )?;
        for ag in self.registry.attribute_groups.iter().filter(|ag| {
            let ag_id: &str = &ag.id;
            include_all || attribute_groups_imports_matcher.is_match(ag_id)
        }) {
            let mut attributes = vec![];
            for ar in ag.attributes.iter() {
                let attr = self.attribute_catalog.attribute(ar).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.0,
                    },
                )?;
                attributes.push(attribute_catalog.attribute_ref(convert_v2_attribute(
                    attr,
                    RequirementLevel::default(),
                    None,
                )));
            }
            result.push(Group {
                id: ag.id().to_owned(),
                r#type: GroupType::AttributeGroup,
                brief: ag.common.brief.clone(),
                note: ag.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(ag.common.stability.clone()),
                deprecated: ag.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(ag.common.annotations.clone()),
                entity_associations: vec![],
                visibility: None,
                is_v2: true,
            });
        }
        Ok(result)
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
pub(crate) trait GroupRefinementLookup {
    /// Looks up a group summary on this repo.
    /// id: The group id to find
    /// return: The summary of the group, or None if the group was not found.
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary>;
}

impl GroupRefinementLookup for V1Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.group(id).map(|g| {
            let attributes: Vec<UnresolvedAttribute> = g
                .attributes
                .iter()
                .filter_map(|ar| self.catalog.attribute(ar))
                .map(|a| UnresolvedAttribute {
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
                })
                .collect();
            let mut summary = GroupSummary::from_without_attributes(g);
            summary.attributes = attributes;
            summary
        })
    }
}

impl GroupRefinementLookup for V2Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        let lookup_group = self
            .registry
            .metrics
            .iter()
            .find(|m| m.id() == id)
            .map(|m| Group {
                id: m.id().to_owned(),
                r#type: GroupType::Metric,
                brief: m.common.brief.clone(),
                note: m.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(m.common.stability.clone()),
                deprecated: m.common.deprecated.clone(),
                attributes: vec![],
                span_kind: None,
                events: vec![],
                metric_name: Some(m.name.to_string()),
                instrument: Some(m.instrument.clone()),
                unit: Some(m.unit.clone()),
                name: None,
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(m.common.annotations.clone()),
                entity_associations: m.entity_associations.clone(),
                visibility: None,
                is_v2: true,
            })
            .or_else(|| {
                self.registry
                    .events
                    .iter()
                    .find(|e| e.id() == id)
                    .map(|e| Group {
                        id: e.id().to_owned(),
                        r#type: GroupType::Event,
                        brief: e.common.brief.clone(),
                        note: e.common.note.clone(),
                        prefix: "".to_owned(),
                        extends: None,
                        stability: Some(e.common.stability.clone()),
                        deprecated: e.common.deprecated.clone(),
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        name: Some(e.name.to_string()),
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: Some(e.common.annotations.clone()),
                        entity_associations: e.entity_associations.clone(),
                        visibility: None,
                        is_v2: true,
                    })
            })
            .or_else(|| {
                self.registry
                    .entities
                    .iter()
                    .find(|e| e.id() == id)
                    .map(|e| Group {
                        id: e.id().to_owned(),
                        r#type: GroupType::Entity,
                        brief: e.common.brief.clone(),
                        note: e.common.note.clone(),
                        prefix: "".to_owned(),
                        extends: None,
                        stability: Some(e.common.stability.clone()),
                        deprecated: e.common.deprecated.clone(),
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        name: Some(e.r#type.to_string()),
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: Some(e.common.annotations.clone()),
                        entity_associations: vec![],
                        visibility: None,
                        is_v2: true,
                    })
            });

        // Now fill out all the attributes we need for `extends` and refinements.
        lookup_group.map(|g| {
            let mut summary = GroupSummary::from_without_attributes(&g);
            summary.attributes = g
                .attributes
                .iter()
                .filter_map(|ar| self.attribute_catalog.get(ar.0 as usize))
                .map(|a| UnresolvedAttribute {
                    spec: weaver_semconv::attribute::AttributeSpec::Id {
                        id: a.key.clone(),
                        r#type: a.r#type.clone(),
                        brief: Some(a.common.brief.clone()),
                        examples: a.examples.clone(),
                        tag: None,
                        requirement_level: RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                        ),
                        sampling_relevant: None,
                        note: a.common.note.clone(),
                        stability: Some(a.common.stability.clone()),
                        deprecated: a.common.deprecated.clone(),
                        annotations: Some(a.common.annotations.clone()),
                        role: None,
                    },
                })
                .collect();
            summary
        })
    }
}

impl GroupRefinementLookup for Vec<ResolvedDependency> {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.iter().find_map(|d| d.lookup_group_summary(id))
    }
}

impl From<V1Schema> for ResolvedDependency {
    fn from(value: V1Schema) -> Self {
        ResolvedDependency::V1(Box::new(value))
    }
}

impl From<V2Schema> for ResolvedDependency {
    fn from(value: V2Schema) -> Self {
        ResolvedDependency::V2(Box::new(value))
    }
}

// Constructs a globset from a set of wildcards.
fn build_globset(wildcards: Option<&Vec<GroupWildcard>>) -> Result<GlobSet, Error> {
    let mut builder = GlobSet::builder();
    if let Some(wildcards_vec) = wildcards {
        for wildcard in wildcards_vec.iter() {
            _ = builder.add(wildcard.0.clone());
        }
    }
    builder.build().map_err(|e| Error::InvalidWildcard {
        error: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use std::error::Error;
    use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;

    use crate::dependency::{GroupRefinementLookup, ResolvedDependency};

    #[test]
    fn test_lookup_group_summary() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::V1(Box::new(example_v1_schema()));
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

    fn example_v1_schema() -> V1Schema {
        V1Schema {
            file_format: "resolved/1.0.0".to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "test-registry".to_owned(),
            registry: weaver_resolved_schema::registry::Registry {
                registry_url: "v1-example".to_owned(),
                groups: vec![
                    weaver_resolved_schema::registry::Group {
                        id: "a".to_owned(),
                        r#type: weaver_semconv::group::GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![weaver_resolved_schema::attribute::AttributeRef(0)],
                        span_kind: Default::default(),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                        is_v2: Default::default(),
                    },
                    weaver_resolved_schema::registry::Group {
                        id: "span.v1".to_owned(),
                        r#type: weaver_semconv::group::GroupType::Span,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![],
                        span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                        is_v2: Default::default(),
                    },
                ],
            },
            catalog: weaver_resolved_schema::catalog::Catalog::from_attributes(vec![
                weaver_resolved_schema::attribute::Attribute {
                    name: "a.test".to_owned(),
                    r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    brief: Default::default(),
                    examples: Default::default(),
                    tag: Default::default(),
                    requirement_level: Default::default(),
                    sampling_relevant: Default::default(),
                    note: Default::default(),
                    stability: Default::default(),
                    deprecated: Default::default(),
                    prefix: Default::default(),
                    tags: None,
                    annotations: Default::default(),
                    value: Default::default(),
                    role: Default::default(),
                },
            ]),
            resource: None,
            instrumentation_library: None,
            dependencies: vec![],
            versions: None,
            registry_manifest: None,
        }
    }

    fn example_v2_schema() -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0.0".to_owned(),
            schema_url: "http://test/schemas/2.0.0".try_into().unwrap(),
            registry: weaver_resolved_schema::v2::registry::Registry {
                attribute_groups: vec![
                    weaver_resolved_schema::v2::attribute_group::AttributeGroup {
                        id: "attribute_group.e".to_owned().into(),
                        attributes: vec![],
                        common: Default::default(),
                    },
                ],
                metrics: vec![weaver_resolved_schema::v2::metric::Metric {
                    name: "metric.a".to_owned().into(),
                    instrument: weaver_semconv::group::InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: Default::default(),
                }],
                events: vec![weaver_resolved_schema::v2::event::Event {
                    name: "event.b".to_owned().into(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: Default::default(),
                }],
                spans: vec![weaver_resolved_schema::v2::span::Span {
                    r#type: "span.d".to_owned().into(),
                    kind: weaver_semconv::group::SpanKindSpec::Client,
                    name: weaver_semconv::v2::span::SpanName {
                        note: "test".to_owned(),
                    },
                    attributes: vec![],
                    entity_associations: vec![],
                    common: Default::default(),
                }],
                entities: vec![weaver_resolved_schema::v2::entity::Entity {
                    r#type: "entity.c".to_owned().into(),
                    identity: vec![],
                    description: vec![],
                    common: Default::default(),
                }],
                attributes: vec![],
            },
            attribute_catalog: vec![],
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
            },
        }
    }

    #[test]
    fn test_lookup_group_summary_v2() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::V2(Box::new(example_v2_schema()));

        let result_metric = d.lookup_group_summary("metric.a");
        assert!(result_metric.is_some(), "Should find metric.a");
        assert_eq!(
            result_metric.unwrap().r#type,
            weaver_semconv::group::GroupType::Metric
        );

        let result_event = d.lookup_group_summary("event.b");
        assert!(result_event.is_some(), "Should find event.b");
        assert_eq!(
            result_event.unwrap().r#type,
            weaver_semconv::group::GroupType::Event
        );

        let result_entity = d.lookup_group_summary("entity.c");
        assert!(result_entity.is_some(), "Should find entity.c");
        assert_eq!(
            result_entity.unwrap().r#type,
            weaver_semconv::group::GroupType::Entity
        );

        Ok(())
    }

    #[test]
    fn test_import_groups_v1() -> Result<(), Box<dyn Error>> {
        use crate::dependency::ImportableDependency;
        let d = example_v1_schema();
        let mut catalog = crate::attribute::AttributeCatalog::default();

        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new("test", "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: None,
                events: None,
                entities: None,
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.v1").unwrap(),
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("a").unwrap(),
                )]),
            },
        }];

        // By default V1 example schema has an AttributeGroup and a Span.
        let result = d.import_groups(&imports, false, &mut catalog)?;
        assert_eq!(result.len(), 2, "Attribute group and span should be imported");

        let result_all = d.import_groups(&imports, true, &mut catalog)?;
        assert_eq!(result_all.len(), 2, "Include all should also import both");

        Ok(())
    }

    #[test]
    fn test_import_groups_v2() -> Result<(), Box<dyn Error>> {
        use crate::dependency::ImportableDependency;
        let d = example_v2_schema();
        let mut catalog = crate::attribute::AttributeCatalog::default();

        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new("test", "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("metric.a").unwrap(),
                )]),
                events: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("event.b").unwrap(),
                )]),
                entities: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("entity.c").unwrap(),
                )]),
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.d").unwrap(),
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e").unwrap(),
                )]),
            },
        }];

        let result = d.import_groups(&imports, false, &mut catalog)?;
        assert_eq!(
            result.len(),
            5,
            "Should import metric, event, entity, span and attribute_group"
        );

        let result_all = d.import_groups(&imports, true, &mut catalog)?;
        assert_eq!(result_all.len(), 5, "Include all should also import all 5");

        Ok(())
    }

    #[test]
    fn test_import_groups_vec() -> Result<(), Box<dyn Error>> {
        use crate::dependency::ImportableDependency;
        let deps = vec![
            ResolvedDependency::V1(Box::new(example_v1_schema())),
            ResolvedDependency::V2(Box::new(example_v2_schema())),
        ];
        let mut catalog = crate::attribute::AttributeCatalog::default();

        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new("test", "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("metric.a").unwrap(),
                )]),
                events: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("event.b").unwrap(),
                )]),
                entities: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("entity.c").unwrap(),
                )]),
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.d").unwrap(),
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e").unwrap(),
                )]),
            },
        }];

        let result = deps.import_groups(&imports, false, &mut catalog)?;
        // V1 schema has AttributeGroup, which returns false unless include_all.
        // V2 schema has metric, event, entity, span, and attribute_group that match.
        assert_eq!(result.len(), 5);

        let result_all = deps.import_groups(&imports, true, &mut catalog)?;
        // V1 (1 group) + V2 (5 groups) = 6 groups.
        assert_eq!(result_all.len(), 6);

        Ok(())
    }
}
