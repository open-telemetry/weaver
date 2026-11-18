// SPDX-License-Identifier: Apache-2.0

//! Assembly and emission of the final V2 `ResolvedTelemetrySchema`.

use std::collections::BTreeSet;
use weaver_resolved_schema::v2::attribute_group::{
    AttributeGroup as V2AttributeGroup, AttributeGroupAttributeRef,
};
use weaver_resolved_schema::v2::entity::{
    Entity as V2Entity, EntityAttributeRef, EntityRefinement,
};
use weaver_resolved_schema::v2::event::{Event as V2Event, EventAttributeRef, EventRefinement};
use weaver_resolved_schema::v2::metric::{
    Metric as V2Metric, MetricAttributeRef, MetricRefinement,
};
use weaver_resolved_schema::v2::refinements::Refinements;
use weaver_resolved_schema::v2::registry::Registry as V2Registry;
use weaver_resolved_schema::v2::span::{Span as V2Span, SpanAttributeRef, SpanRefinement};
use weaver_resolved_schema::v2::{ResolvedTelemetrySchema as V2Schema, V2_RESOLVED_FILE_FORMAT};
use weaver_semconv::schema_url::SchemaUrl;

use super::attributes::FinalCatalog;
use super::groups::ExpandedGroups;
use super::imports::ImportedItems;
use super::index::Index;
use super::refinements::ResolvedRefinements;
use super::signals::{EntityAssociations, ResolvedSignals};

/// Assembles the final `ResolvedTelemetrySchema` directly in V2 format.
pub(crate) fn schema(
    schema_url: SchemaUrl,
    cat: FinalCatalog,
    groups: ExpandedGroups,
    signals: ResolvedSignals,
    refinements: ResolvedRefinements,
    imported: ImportedItems,
    dependencies: &BTreeSet<SchemaUrl>,
    index: &Index,
) -> V2Schema {
    let registry_attributes = cat.local_def_attribute_refs();

    let mut attribute_groups = Vec::with_capacity(
        groups.public_groups().len()
            + imported
                .attribute_groups
                .iter()
                .filter(|ag| ag.is_public)
                .count(),
    );
    for g in groups.public_groups() {
        let attributes = g
            .attributes
            .iter()
            .map(|(attr, req_level)| AttributeGroupAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        attribute_groups.push(V2AttributeGroup {
            id: g.id.clone(),
            attributes,
            common: g.common.clone(),
            provenance: g.provenance.clone(),
        });
    }
    for ag in &imported.attribute_groups {
        if ag.is_public {
            let attributes = ag
                .attributes
                .iter()
                .map(|(attr, req_level, _, _)| AttributeGroupAttributeRef {
                    base: cat.attribute_ref(attr),
                    requirement_level: req_level.clone(),
                })
                .collect();
            attribute_groups.push(V2AttributeGroup {
                id: ag.id.clone(),
                attributes,
                common: ag.common.clone(),
                provenance: ag.provenance.clone(),
            });
        }
    }
    attribute_groups.sort_by(|a, b| a.id.cmp(&b.id));

    let mut metrics = Vec::with_capacity(signals.metrics.len() + imported.metrics.len());
    for m in &signals.metrics {
        let attributes = m
            .attributes
            .iter()
            .map(|(attr, req_level)| MetricAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &m.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        metrics.push(V2Metric {
            name: m.name.clone(),
            instrument: m.instrument,
            unit: m.unit.clone(),
            attributes,
            entity_associations,
            requirement_level: m.requirement_level.clone(),
            common: m.common.clone(),
            provenance: m.provenance.clone(),
        });
    }
    for m in &imported.metrics {
        let attributes = m
            .attributes
            .iter()
            .map(|(attr, req_level, _, _)| MetricAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &m.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        metrics.push(V2Metric {
            name: m.name.clone(),
            instrument: m.instrument,
            unit: m.unit.clone(),
            attributes,
            entity_associations,
            requirement_level: m.requirement_level.clone(),
            common: m.common.clone(),
            provenance: m.provenance.clone(),
        });
    }

    metrics.sort_by(|a, b| a.name.cmp(&b.name));

    let mut spans = Vec::with_capacity(signals.spans.len() + imported.spans.len());
    for s in &signals.spans {
        let attributes = s
            .attributes
            .iter()
            .map(|(attr, req_level, sampling_relevant)| SpanAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
                sampling_relevant: *sampling_relevant,
            })
            .collect();
        let entity_associations = match &s.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        spans.push(V2Span {
            r#type: s.r#type.clone(),
            kind: s.kind,
            name: s.name.clone(),
            attributes,
            entity_associations,
            requirement_level: s.requirement_level.clone(),
            common: s.common.clone(),
            provenance: s.provenance.clone(),
        });
    }
    for s in &imported.spans {
        let attributes = s
            .attributes
            .iter()
            .map(
                |(attr, req_level, sampling_relevant, _, _)| SpanAttributeRef {
                    base: cat.attribute_ref(attr),
                    requirement_level: req_level.clone(),
                    sampling_relevant: *sampling_relevant,
                },
            )
            .collect();
        let entity_associations = match &s.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        spans.push(V2Span {
            r#type: s.r#type.clone(),
            kind: s.kind,
            name: s.name.clone(),
            attributes,
            entity_associations,
            requirement_level: s.requirement_level.clone(),
            common: s.common.clone(),
            provenance: s.provenance.clone(),
        });
    }
    spans.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    let mut events = Vec::with_capacity(signals.events.len() + imported.events.len());
    for e in &signals.events {
        let attributes = e
            .attributes
            .iter()
            .map(|(attr, req_level)| EventAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &e.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        events.push(V2Event {
            name: e.name.clone(),
            attributes,
            entity_associations,
            requirement_level: e.requirement_level.clone(),
            common: e.common.clone(),
            provenance: e.provenance.clone(),
        });
    }
    for e in &imported.events {
        let attributes = e
            .attributes
            .iter()
            .map(|(attr, req_level, _, _)| EventAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &e.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        events.push(V2Event {
            name: e.name.clone(),
            attributes,
            entity_associations,
            requirement_level: e.requirement_level.clone(),
            common: e.common.clone(),
            provenance: e.provenance.clone(),
        });
    }
    events.sort_by(|a, b| a.name.cmp(&b.name));

    let mut entities = Vec::with_capacity(signals.entities.len() + imported.entities.len());
    for en in &signals.entities {
        let mut identity = en
            .identity
            .iter()
            .map(|attr| cat.attribute_ref(attr))
            .collect::<Vec<_>>();
        identity.sort();
        let description = en
            .description
            .iter()
            .map(|(attr, req_level)| EntityAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        entities.push(V2Entity {
            r#type: en.r#type.clone(),
            identity,
            description,
            requirement_level: en.requirement_level.clone(),
            common: en.common.clone(),
            provenance: en.provenance.clone(),
        });
    }
    for en in &imported.entities {
        let mut identity = en
            .identity
            .iter()
            .map(|(attr, _, _)| cat.attribute_ref(attr))
            .collect::<Vec<_>>();
        identity.sort();
        let description = en
            .description
            .iter()
            .map(|(attr, req_level, _, _)| EntityAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        entities.push(V2Entity {
            r#type: en.r#type.clone(),
            identity,
            description,
            requirement_level: en.requirement_level.clone(),
            common: en.common.clone(),
            provenance: en.provenance.clone(),
        });
    }
    entities.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    // Include a base entry per signal alongside explicit refinements, sorted by authored group ID.
    let mut metric_refinements_with_key =
        Vec::with_capacity(metrics.len() + refinements.metrics.len());
    metric_refinements_with_key.extend(metrics.iter().map(|m| {
        let key = format!("metric.{}", m.name);
        (
            key,
            MetricRefinement {
                id: m.name.clone(),
                metric: m.clone(),
            },
        )
    }));
    for (id, m) in refinements.metrics {
        let attributes = m
            .attributes
            .iter()
            .map(|(attr, req_level)| MetricAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &m.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        let key = index
            .metric_refinements()
            .get(&id)
            .map(|ir| ir.refinement.id.to_string())
            .unwrap_or_else(|| id.to_string());
        metric_refinements_with_key.push((
            key,
            MetricRefinement {
                id,
                metric: V2Metric {
                    name: m.name,
                    instrument: m.instrument,
                    unit: m.unit,
                    attributes,
                    entity_associations,
                    requirement_level: m.requirement_level,
                    common: m.common,
                    provenance: m.provenance,
                },
            },
        ));
    }
    metric_refinements_with_key.sort_by(|a, b| a.0.cmp(&b.0));
    let metric_refinements: Vec<MetricRefinement> = metric_refinements_with_key
        .into_iter()
        .map(|(_, r)| r)
        .collect();

    let mut span_refinements_with_key = Vec::with_capacity(spans.len() + refinements.spans.len());
    span_refinements_with_key.extend(spans.iter().map(|s| {
        let key = format!("span.{}", s.r#type);
        (
            key,
            SpanRefinement {
                id: s.r#type.clone(),
                span: s.clone(),
            },
        )
    }));
    for (id, s) in refinements.spans {
        let attributes = s
            .attributes
            .iter()
            .map(|(attr, req_level, sampling_relevant)| SpanAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
                sampling_relevant: *sampling_relevant,
            })
            .collect();
        let entity_associations = match &s.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        let key = index
            .span_refinements()
            .get(&id)
            .map(|ir| ir.refinement.id.to_string())
            .unwrap_or_else(|| id.to_string());
        span_refinements_with_key.push((
            key,
            SpanRefinement {
                id,
                span: V2Span {
                    r#type: s.r#type,
                    kind: s.kind,
                    name: s.name,
                    attributes,
                    entity_associations,
                    requirement_level: s.requirement_level,
                    common: s.common,
                    provenance: s.provenance,
                },
            },
        ));
    }
    span_refinements_with_key.sort_by(|a, b| a.0.cmp(&b.0));
    let span_refinements: Vec<SpanRefinement> = span_refinements_with_key
        .into_iter()
        .map(|(_, r)| r)
        .collect();

    let mut event_refinements_with_key =
        Vec::with_capacity(events.len() + refinements.events.len());
    event_refinements_with_key.extend(events.iter().map(|e| {
        let key = format!("event.{}", e.name);
        (
            key,
            EventRefinement {
                id: e.name.clone(),
                event: e.clone(),
            },
        )
    }));
    for (id, e) in refinements.events {
        let attributes = e
            .attributes
            .iter()
            .map(|(attr, req_level)| EventAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let entity_associations = match &e.entity_associations {
            EntityAssociations::Resolved(assocs) => assocs.clone(),
            EntityAssociations::Raw(_) => vec![],
        };
        let key = index
            .event_refinements()
            .get(&id)
            .map(|ir| ir.refinement.id.to_string())
            .unwrap_or_else(|| id.to_string());
        event_refinements_with_key.push((
            key,
            EventRefinement {
                id,
                event: V2Event {
                    name: e.name,
                    attributes,
                    entity_associations,
                    requirement_level: e.requirement_level,
                    common: e.common,
                    provenance: e.provenance,
                },
            },
        ));
    }
    event_refinements_with_key.sort_by(|a, b| a.0.cmp(&b.0));
    let event_refinements: Vec<EventRefinement> = event_refinements_with_key
        .into_iter()
        .map(|(_, r)| r)
        .collect();

    let mut entity_refinements_with_key =
        Vec::with_capacity(entities.len() + refinements.entities.len());
    entity_refinements_with_key.extend(entities.iter().map(|en| {
        let key = format!("entity.{}", en.r#type);
        (
            key,
            EntityRefinement {
                id: en.r#type.clone(),
                entity: en.clone(),
            },
        )
    }));
    for (id, en) in refinements.entities {
        let identity = en
            .identity
            .iter()
            .map(|attr| cat.attribute_ref(attr))
            .collect();
        let description = en
            .description
            .iter()
            .map(|(attr, req_level)| EntityAttributeRef {
                base: cat.attribute_ref(attr),
                requirement_level: req_level.clone(),
            })
            .collect();
        let key = index
            .entity_refinements()
            .get(&id)
            .map(|ir| ir.refinement.id.to_string())
            .unwrap_or_else(|| format!("entity.{id}"));
        entity_refinements_with_key.push((
            key,
            EntityRefinement {
                id,
                entity: V2Entity {
                    r#type: en.r#type,
                    identity,
                    description,
                    requirement_level: en.requirement_level,
                    common: en.common,
                    provenance: en.provenance,
                },
            },
        ));
    }
    entity_refinements_with_key.sort_by(|a, b| a.0.cmp(&b.0));
    let entity_refinements: Vec<EntityRefinement> = entity_refinements_with_key
        .into_iter()
        .map(|(_, r)| r)
        .collect();

    let refinements = Refinements {
        spans: span_refinements,
        metrics: metric_refinements,
        events: event_refinements,
        entities: entity_refinements,
    };

    let registry = V2Registry {
        attributes: registry_attributes,
        attribute_groups,
        spans,
        metrics,
        events,
        entities,
    };

    V2Schema {
        file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
        schema_url,
        attribute_catalog: cat.into_attributes(),
        registry,
        refinements,
        dependencies: dependencies.clone(),
    }
}
