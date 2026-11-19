// SPDX-License-Identifier: Apache-2.0

//! Reading V2 schemas as dependencies into the V1 resolution algorithm.

use std::collections::BTreeMap;
use weaver_resolved_schema::v1::attribute::UnresolvedAttribute;
use weaver_resolved_schema::v2::attribute::Attribute as V2Attribute;
use weaver_resolved_schema::v2::entity::Entity;
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::v2::Signal;
use weaver_semconv::convert::{
    v2_attribute_type_to_v1, v2_examples_to_v1, v2_instrument_to_v1, v2_requirement_level_to_v1,
    v2_span_kind_to_v1, v2_span_name_to_v1,
};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::{
    AttributeRole, AttributeSpec, BasicRequirementLevelSpec, RequirementLevel,
};
use weaver_semconv::v1::group::GroupType;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::CommonFields;

use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::graph::exclusion::is_excluded;
use crate::v1::dependency::{GroupRefinementLookup, GroupSource, GroupSummary};
use crate::v1::imports::{to_named_associations, v2_association_origins};

impl EntityLookup for V2Schema {
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        // Match base entities by `type` and entity refinements by `id`, as both share one namespace.
        let entity = self
            .registry
            .entities
            .iter()
            .find(|e| &*e.r#type == name)
            .or_else(|| {
                self.refinements
                    .entities
                    .iter()
                    .find(|r| &*r.id == name)
                    .map(|r| &r.entity)
            })?;
        let origin = entity
            .provenance
            .source
            .and_then(|dep_ref| self.dependencies.iter().nth(dep_ref.0 as usize).cloned())
            .unwrap_or_else(|| self.schema_url.clone());
        Some(EntityLocation {
            origin,
            excluded: is_excluded(&entity.common.annotations),
        })
    }
}

/// Resolves the origin `SchemaUrl` of a V2 item from `deps` (materialized from `schema.dependencies`)
/// when `source` is set, or `schema.schema_url` otherwise.
pub(crate) fn v2_source_url(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    source: Option<DependencyRef>,
) -> SchemaUrl {
    source
        .and_then(|dep_ref| deps.get(dep_ref.0 as usize).cloned())
        .unwrap_or_else(|| schema.schema_url.clone())
}

/// Converts a V2 catalog attribute and per-signal reference overrides into an `UnresolvedAttribute`.
fn attr_spec(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    a: &V2Attribute,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
) -> UnresolvedAttribute {
    UnresolvedAttribute {
        origin: Some(v2_source_url(schema, deps, a.provenance.source)),
        spec: AttributeSpec::Id {
            id: a.key.clone(),
            r#type: v2_attribute_type_to_v1(a.r#type.clone()),
            brief: Some(a.common.brief.clone()),
            examples: a.examples.clone().map(v2_examples_to_v1),
            tag: None,
            requirement_level,
            sampling_relevant,
            note: a.common.note.clone().unwrap_or_default(),
            stability: Some(a.common.stability.clone().into()),
            deprecated: a.common.deprecated.clone(),
            annotations: Some(a.common.annotations.clone()),
            role,
        },
    }
}

/// Builds a dependency `GroupSummary` initialized with fields common to all V2 signals.
fn signal_summary(
    r#type: GroupType,
    common: &CommonFields,
    requirement_level: Option<SignalRequirementLevel>,
    attributes: Vec<UnresolvedAttribute>,
) -> GroupSummary {
    GroupSummary {
        r#type,
        brief: common.brief.clone(),
        note: common.note.clone(),
        stability: Some(common.stability.clone().into()),
        deprecated: common.deprecated.clone(),
        metric_name: None,
        name: None,
        instrument: None,
        unit: None,
        requirement_level: requirement_level.map(Into::into),
        span_kind: None,
        span_name: None,
        attributes,
        annotations: Some(common.annotations.clone()),
        entity_associations: vec![],
        association_origins: BTreeMap::new(),
        origin: None,
        source: GroupSource::Dependency,
    }
}

/// Builds a `GroupSummary` for a V2 `Entity`, tagging identity and description attributes with their roles.
fn entity_group_summary(schema: &V2Schema, deps: &[SchemaUrl], e: &Entity) -> GroupSummary {
    let attributes = e
        .identity
        .iter()
        .map(|ar| {
            (
                ar,
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                AttributeRole::Identifying,
            )
        })
        .chain(e.description.iter().map(|ar| {
            (
                &ar.base,
                v2_requirement_level_to_v1(ar.requirement_level.clone()),
                AttributeRole::Descriptive,
            )
        }))
        .filter_map(|(ar, req_level, role)| {
            schema
                .attribute_catalog
                .get(ar.0 as usize)
                .map(|a| attr_spec(schema, deps, a, req_level, None, Some(role)))
        })
        .collect();
    let mut summary = signal_summary(
        GroupType::Entity,
        &e.common,
        e.requirement_level.clone(),
        attributes,
    );
    summary.name = Some(e.r#type.to_string());
    summary.origin = Some(v2_source_url(schema, deps, e.provenance.source));
    summary
}

/// Finds a signal matching `id` with `prefix` stripped first, falling back to raw `id`.
fn find<'a, S: Signal>(signals: &'a [S], id: &str, prefix: &str) -> Option<&'a S> {
    let by_id = |n: &str| signals.iter().find(|s| s.id() == n);
    id.strip_prefix(prefix)
        .and_then(by_id)
        .or_else(|| by_id(id))
}

impl GroupRefinementLookup for V2Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        let deps: Vec<_> = self.dependencies.iter().cloned().collect();

        if let Some(e) = find(&self.registry.entities, id, "entity.") {
            return Some(entity_group_summary(self, &deps, e));
        }
        if let Some(m) = find(&self.registry.metrics, id, "metric.") {
            let attributes = m
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            v2_requirement_level_to_v1(ar.requirement_level.clone()),
                            None,
                            None,
                        )
                    })
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Metric,
                &m.common,
                m.requirement_level.clone(),
                attributes,
            );
            summary.metric_name = Some(m.name.to_string());
            summary.instrument = Some(v2_instrument_to_v1(m.instrument));
            summary.unit = Some(m.unit.clone());
            summary.entity_associations = to_named_associations(&m.entity_associations);
            summary.association_origins =
                v2_association_origins(self, &deps, &m.entity_associations);
            summary.origin = Some(v2_source_url(self, &deps, m.provenance.source));
            return Some(summary);
        }
        if let Some(e) = find(&self.registry.events, id, "event.") {
            let attributes = e
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            v2_requirement_level_to_v1(ar.requirement_level.clone()),
                            None,
                            None,
                        )
                    })
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Event,
                &e.common,
                e.requirement_level.clone(),
                attributes,
            );
            summary.name = Some(e.name.to_string());
            summary.entity_associations = to_named_associations(&e.entity_associations);
            summary.association_origins =
                v2_association_origins(self, &deps, &e.entity_associations);
            summary.origin = Some(v2_source_url(self, &deps, e.provenance.source));
            return Some(summary);
        }
        if let Some(s) = find(&self.registry.spans, id, "span.") {
            let attributes = s
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            v2_requirement_level_to_v1(ar.requirement_level.clone()),
                            ar.sampling_relevant,
                            None,
                        )
                    })
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Span,
                &s.common,
                s.requirement_level.clone(),
                attributes,
            );
            summary.span_kind = Some(v2_span_kind_to_v1(s.kind));
            summary.span_name = Some(v2_span_name_to_v1(s.name.clone()));
            summary.entity_associations = to_named_associations(&s.entity_associations);
            summary.association_origins =
                v2_association_origins(self, &deps, &s.entity_associations);
            summary.origin = Some(v2_source_url(self, &deps, s.provenance.source));
            return Some(summary);
        }
        None
    }
}

#[cfg(test)]
pub(crate) fn example_v2_schema() -> V2Schema {
    use std::collections::BTreeSet;
    use weaver_resolved_schema::v2::attribute::AttributeRef as V2AttributeRef;
    use weaver_resolved_schema::v2::attribute_group::{AttributeGroup, AttributeGroupAttributeRef};
    use weaver_resolved_schema::v2::entity::{
        EntityAssociation as V2EntityAssociation, EntityAttributeRef, EntityRef,
    };
    use weaver_resolved_schema::v2::event::Event;
    use weaver_resolved_schema::v2::metric::Metric;
    use weaver_resolved_schema::v2::refinements::Refinements;
    use weaver_resolved_schema::v2::registry::Registry as V2Registry;
    use weaver_resolved_schema::v2::span::Span;
    use weaver_semconv::v2::attribute::{
        AttributeType as V2AttributeType, BasicRequirementLevelSpec as V2BasicRequirementLevelSpec,
        PrimitiveOrArrayTypeSpec as V2PrimitiveOrArrayTypeSpec,
        RequirementLevel as V2RequirementLevel,
    };
    use weaver_semconv::v2::metric::InstrumentSpec as V2InstrumentSpec;
    use weaver_semconv::v2::span::{SpanKindSpec as V2SpanKindSpec, SpanName as V2SpanName};

    V2Schema {
        file_format: "resolved/2.0".to_owned(),
        schema_url: "http://test/schemas/2.0.0".try_into().unwrap(),
        registry: V2Registry {
            attribute_groups: vec![AttributeGroup {
                id: "attribute_group.e".to_owned().into(),
                // A public group whose attribute ref carries a
                // non-default requirement level; importing the group
                // must preserve it.
                attributes: vec![AttributeGroupAttributeRef {
                    base: V2AttributeRef(0),
                    requirement_level: V2RequirementLevel::Basic(
                        V2BasicRequirementLevelSpec::Required,
                    ),
                }],
                common: Default::default(),
                provenance: Default::default(),
            }],
            metrics: vec![Metric {
                name: "metric.a".to_owned().into(),
                instrument: V2InstrumentSpec::Counter,
                unit: "1".to_owned(),
                attributes: vec![],
                entity_associations: vec![V2EntityAssociation::Ref(EntityRef::local(
                    "entity.c".to_owned().into(),
                ))],
                requirement_level: None,
                common: Default::default(),
                provenance: Default::default(),
            }],
            events: vec![Event {
                name: "event.b".to_owned().into(),
                attributes: vec![],
                entity_associations: vec![],
                requirement_level: None,
                common: Default::default(),
                provenance: Default::default(),
            }],
            spans: vec![Span {
                r#type: "span.d".to_owned().into(),
                kind: V2SpanKindSpec::Client,
                name: V2SpanName {
                    templates: Vec::new(),
                    note: Some("test".to_owned()),
                },
                attributes: vec![],
                entity_associations: vec![],
                requirement_level: None,
                common: Default::default(),
                provenance: Default::default(),
            }],
            entities: vec![Entity {
                r#type: "entity.c".to_owned().into(),
                // An identity and a description attribute, so importing
                // the entity has to tag each one with its role.
                identity: vec![V2AttributeRef(1)],
                description: vec![EntityAttributeRef {
                    base: V2AttributeRef(2),
                    requirement_level: Default::default(),
                }],
                requirement_level: None,
                common: Default::default(),
                provenance: Default::default(),
            }],
            attributes: vec![],
        },
        attribute_catalog: vec![
            V2Attribute {
                key: "attr.in.group".to_owned(),
                r#type: V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: Default::default(),
                provenance: Default::default(),
            },
            V2Attribute {
                key: "entity.c.id".to_owned(),
                r#type: V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: Default::default(),
                provenance: Default::default(),
            },
            V2Attribute {
                key: "entity.c.label".to_owned(),
                r#type: V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: Default::default(),
                provenance: Default::default(),
            },
        ],
        refinements: Refinements {
            spans: vec![],
            metrics: vec![],
            events: vec![],
            entities: vec![],
        },
        dependencies: BTreeSet::new(),
    }
}
