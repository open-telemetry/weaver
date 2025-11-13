//! Version 2 of semantic convention schema.

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    group::GroupType,
    v2::{
        attribute_group::AttributeGroupVisibilitySpec, signal_id::SignalId, span::SpanName,
        CommonFields,
    },
};

use crate::v2::{
    attribute_group::AttributeGroup,
    catalog::Catalog,
    entity::Entity,
    metric::Metric,
    refinements::Refinements,
    registry::Registry,
    span::{Span, SpanRefinement},
};

pub mod attribute;
pub mod attribute_group;
pub mod catalog;
pub mod entity;
pub mod event;
pub mod metric;
pub mod refinements;
pub mod registry;
pub mod span;

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: String,
    /// The ID of the registry that this schema belongs to.
    pub registry_id: String,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Refinements for the registry
    pub refinements: Refinements,
    // TODO - versions, dependencies and other options.
}

/// Easy conversion from v1 to v2.
impl TryFrom<crate::ResolvedTelemetrySchema> for ResolvedTelemetrySchema {
    type Error = crate::error::Error;
    fn try_from(value: crate::ResolvedTelemetrySchema) -> Result<Self, Self::Error> {
        let (registry, refinements) = convert_v1_to_v2(value.catalog, value.registry)?;
        Ok(ResolvedTelemetrySchema {
            // TODO - bump file format?
            file_format: value.file_format,
            schema_url: value.schema_url,
            registry_id: value.registry_id,
            registry,
            refinements,
        })
    }
}

fn fix_group_id(prefix: &'static str, group_id: &str) -> SignalId {
    if group_id.starts_with(prefix) {
        group_id.trim_start_matches(prefix).to_owned().into()
    } else {
        group_id.to_owned().into()
    }
}

fn fix_span_group_id(group_id: &str) -> SignalId {
    fix_group_id("span.", group_id)
}

/// Converts a V1 registry + catalog to V2.
pub fn convert_v1_to_v2(
    c: crate::catalog::Catalog,
    r: crate::registry::Registry,
) -> Result<(Registry, Refinements), crate::error::Error> {
    // When pulling attributes, as we collapse things, we need to filter
    // to just unique.
    let attributes: HashSet<attribute::Attribute> = c
        .attributes
        .iter()
        .cloned()
        .map(|a| {
            attribute::Attribute {
                key: a.name,
                r#type: a.r#type,
                examples: a.examples,
                common: CommonFields {
                    brief: a.brief,
                    note: a.note,
                    // TODO - Check this assumption.
                    stability: a
                        .stability
                        .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                    deprecated: a.deprecated,
                    annotations: a.annotations.unwrap_or_default(),
                },
            }
        })
        .collect();

    let v2_catalog = Catalog::from_attributes(attributes.into_iter().collect());

    // Create a lookup so we can check inheritance.
    let mut group_type_lookup = HashMap::new();
    for g in r.groups.iter() {
        let _ = group_type_lookup.insert(g.id.clone(), g.r#type.clone());
    }
    // Pull signals from the registry and create a new span-focused registry.
    let mut spans = Vec::new();
    let mut span_refinements = Vec::new();
    let mut metrics = Vec::new();
    let mut metric_refinements = Vec::new();
    let mut events = Vec::new();
    let mut event_refinements = Vec::new();
    let mut entities = Vec::new();
    let mut attribute_groups = Vec::new();
    for g in r.groups.iter() {
        match g.r#type {
            GroupType::Span => {
                // Check if we extend another span.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Span)
                    .unwrap_or(false);
                // Pull all the attribute references.
                let mut span_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        span_attributes.push(span::SpanAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                            sampling_relevant: attr.sampling_relevant,
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                if !is_refinement {
                    let span = Span {
                        r#type: fix_span_group_id(&g.id),
                        kind: g
                            .span_kind
                            .clone()
                            .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                        // TODO - Pass advanced name controls through V1 groups.
                        name: SpanName {
                            note: g.name.clone().unwrap_or_default(),
                        },
                        entity_associations: g.entity_associations.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        attributes: span_attributes,
                    };
                    spans.push(span.clone());
                    span_refinements.push(SpanRefinement {
                        id: span.r#type.clone(),
                        span,
                    });
                } else {
                    // unwrap should be safe because we verified this is a refinement earlier.
                    let span_type = g
                        .lineage
                        .as_ref()
                        .and_then(|l| l.extends_group.as_ref())
                        .map(|id| fix_span_group_id(id))
                        .expect("Refinement extraction issue - this is a logic bug");
                    span_refinements.push(SpanRefinement {
                        id: fix_span_group_id(&g.id),
                        span: Span {
                            r#type: span_type,
                            kind: g
                                .span_kind
                                .clone()
                                .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                            // TODO - Pass advanced name controls through V1 groups.
                            name: SpanName {
                                note: g.name.clone().unwrap_or_default(),
                            },
                            entity_associations: g.entity_associations.clone(),
                            common: CommonFields {
                                brief: g.brief.clone(),
                                note: g.note.clone(),
                                stability: g
                                    .stability
                                    .clone()
                                    .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                                deprecated: g.deprecated.clone(),
                                annotations: g.annotations.clone().unwrap_or_default(),
                            },
                            attributes: span_attributes,
                        },
                    });
                }
            }
            GroupType::Event => {
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Event)
                    .unwrap_or(false);
                let mut event_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        event_attributes.push(event::EventAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                let event = event::Event {
                    name: g
                        .name
                        .clone()
                        .expect("Name must exist on events prior to translation to v2")
                        .into(),
                    attributes: event_attributes,
                    entity_associations: g.entity_associations.clone(),
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                };
                if !is_refinement {
                    events.push(event.clone());
                    event_refinements.push(event::EventRefinement {
                        id: event.name.clone(),
                        event,
                    });
                } else {
                    event_refinements.push(event::EventRefinement {
                        id: fix_group_id("event.", &g.id),
                        event,
                    });
                }
            }
            GroupType::Metric => {
                // Check if we extend another metric.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Metric)
                    .unwrap_or(false);
                let mut metric_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        metric_attributes.push(metric::MetricAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                // TODO - deal with unwrap errors.
                let metric = Metric {
                    name: g
                        .metric_name
                        .clone()
                        .expect("metric_name must exist on metrics prior to translation to v2")
                        .into(),
                    instrument: g
                        .instrument
                        .clone()
                        .expect("instrument must exist on metrics prior to translation to v2"),
                    unit: g
                        .unit
                        .clone()
                        .expect("unit must exist on metrics prior to translation to v2"),
                    attributes: metric_attributes,
                    entity_associations: g.entity_associations.clone(),
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                };
                if is_refinement {
                    metric_refinements.push(metric::MetricRefinement {
                        id: fix_group_id("metric.", &g.id),
                        metric,
                    });
                } else {
                    metrics.push(metric.clone());
                    metric_refinements.push(metric::MetricRefinement {
                        id: metric.name.clone(),
                        metric,
                    });
                }
            }
            GroupType::Entity => {
                let mut id_attrs = Vec::new();
                let mut desc_attrs = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        match attr.role {
                            Some(weaver_semconv::attribute::AttributeRole::Identifying) => {
                                id_attrs.push(entity::EntityAttributeRef {
                                    base: a,
                                    requirement_level: attr.requirement_level.clone(),
                                });
                            }
                            _ => {
                                desc_attrs.push(entity::EntityAttributeRef {
                                    base: a,
                                    requirement_level: attr.requirement_level.clone(),
                                });
                            }
                        }
                    } else {
                        // TODO logic error!
                    }
                }
                entities.push(Entity {
                    r#type: fix_group_id("entity.", &g.id),
                    identity: id_attrs,
                    description: desc_attrs,
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                });
            }
            GroupType::AttributeGroup => {
                if g.visibility
                    .as_ref()
                    .is_some_and(|v| AttributeGroupVisibilitySpec::Public == *v)
                {
                    // Now we need to convert the group.
                    let mut attributes = Vec::new();
                    // TODO - we need to check lineage and remove parent groups.
                    for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                        if let Some(a) = v2_catalog.convert_ref(attr) {
                            attributes.push(a);
                        } else {
                            // TODO logic error!
                        }
                    }
                    attribute_groups.push(AttributeGroup {
                        id: fix_group_id("attribute_group.", &g.id),
                        attributes,
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                    });
                }
            }
            GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => {
                // Ignored for now, we should probably issue warnings.
            }
        }
    }

    let v2_registry = Registry {
        registry_url: r.registry_url,
        attributes: v2_catalog.into(),
        spans,
        metrics,
        events,
        entities,
        attribute_groups,
    };
    let v2_refinements = Refinements {
        spans: span_refinements,
        metrics: metric_refinements,
        events: event_refinements,
    };
    Ok((v2_registry, v2_refinements))
}

#[cfg(test)]
mod tests {

    use weaver_semconv::{provenance::Provenance, stability::Stability};

    use crate::{attribute::Attribute, lineage::GroupLineage, registry::Group};

    use super::*;

    #[test]
    fn test_convert_span_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_span_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_span_lineage.extends("span.my-span");
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![
                Group {
                    id: "span.my-span".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
                Group {
                    id: "span.custom".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: Some(refinement_span_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
            ],
        };

        let (v2_registry, v2_refinements) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.spans.len(), 1);
        if let Some(span) = v2_registry.spans.first() {
            assert_eq!(span.r#type, "my-span".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.spans.len(), 2);
        let span_ref_ids: Vec<String> = v2_refinements
            .spans
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            span_ref_ids,
            vec!["my-span".to_owned(), "custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_metric_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_metric_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_metric_lineage.extends("metric.http");
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![
                Group {
                    id: "metric.http".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[0]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
                Group {
                    id: "metric.http.custom".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: Some(refinement_metric_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
            ],
        };

        let (v2_registry, v2_refinements) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.metrics.len(), 1);
        if let Some(metric) = v2_registry.metrics.first() {
            assert_eq!(metric.name, "http".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.metrics.len(), 2);
        let metric_ref_ids: Vec<String> = v2_refinements
            .metrics
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            metric_ref_ids,
            vec!["http".to_owned(), "http.custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_event_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([Attribute {
            name: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "".to_owned(),
            examples: None,
            tag: None,
            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
            ),
            sampling_relevant: None,
            note: "".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: None,
        }]);
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![Group {
                id: "event.my-event".to_owned(),
                r#type: GroupType::Event,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some("my-event".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
            }],
        };

        let (v2_registry, _) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        assert_eq!(v2_registry.events.len(), 1);
        if let Some(event) = v2_registry.events.first() {
            assert_eq!(event.name, "my-event".to_owned().into());
        }
    }

    #[test]
    fn test_convert_entity_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([Attribute {
            name: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "".to_owned(),
            examples: None,
            tag: None,
            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
            ),
            sampling_relevant: None,
            note: "".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: Some(weaver_semconv::attribute::AttributeRole::Identifying),
        }]);
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![Group {
                id: "entity.my-entity".to_owned(),
                r#type: GroupType::Entity,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some("my-entity".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
            }],
        };

        let (v2_registry, _) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        assert_eq!(v2_registry.entities.len(), 1);
        if let Some(entity) = v2_registry.entities.first() {
            assert_eq!(entity.r#type, "my-entity".to_owned().into());
            assert_eq!(entity.identity.len(), 1);
        }
    }

    #[test]
    fn test_try_from_v1_to_v2() {
        let v1_schema = crate::ResolvedTelemetrySchema {
            file_format: "1.0.0".to_owned(),
            schema_url: "my.schema.url".to_owned(),
            registry_id: "my-registry".to_owned(),
            catalog: crate::catalog::Catalog::from_attributes(vec![]),
            registry: crate::registry::Registry {
                registry_url: "my.schema.url".to_owned(),
                groups: vec![],
            },
            instrumentation_library: None,
            resource: None,
            dependencies: vec![],
            versions: None,
            registry_manifest: None,
        };

        let v2_schema: Result<ResolvedTelemetrySchema, _> = v1_schema.try_into();
        assert!(v2_schema.is_ok());
        let v2_schema = v2_schema.unwrap();
        assert_eq!(v2_schema.file_format, "1.0.0");
        assert_eq!(v2_schema.schema_url, "my.schema.url");
        assert_eq!(v2_schema.registry_id, "my-registry");
    }
}
