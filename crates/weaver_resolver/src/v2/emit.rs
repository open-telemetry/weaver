// SPDX-License-Identifier: Apache-2.0

//! Assembly and emission of the final V2 `ResolvedTelemetrySchema`.

use std::collections::BTreeSet;
use weaver_resolved_schema::v2::attribute::{Attribute, AttributeRef};
use weaver_resolved_schema::v2::attribute_group::{
    AttributeGroup as V2AttributeGroup, AttributeGroupAttributeRef,
};
use weaver_resolved_schema::v2::entity::{
    Entity as V2Entity, EntityAssociation as ResolvedEntityAssociation, EntityAttributeRef,
    EntityRefinement,
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
use weaver_semconv::v2::attribute::RequirementLevel;
use weaver_semconv::v2::signal_id::SignalId;

use super::attributes::FinalCatalog;
use super::groups::{ExpandedGroups, ResolvedPublicGroup};
use super::imports::ImportedItems;
use super::refinements::ResolvedRefinements;
use super::signals::{
    EntityAssociations, ResolvedEntity, ResolvedEvent, ResolvedMetric, ResolvedSignals,
    ResolvedSpan,
};

/// Maps `(attribute, requirement_level)` pairs to catalog-indexed attribute references.
fn emit_attribute_refs<'a, R>(
    cat: &FinalCatalog,
    items: impl IntoIterator<Item = (&'a Attribute, &'a RequirementLevel)>,
    build: impl Fn(AttributeRef, RequirementLevel) -> R,
) -> Vec<R> {
    items
        .into_iter()
        .map(|(attr, req_level)| build(cat.attribute_ref(attr), req_level.clone()))
        .collect()
}

/// Maps span `(attribute, requirement_level, sampling_relevant)` tuples to `SpanAttributeRef`s.
fn emit_span_attribute_refs<'a>(
    cat: &FinalCatalog,
    items: impl IntoIterator<Item = (&'a Attribute, &'a RequirementLevel, Option<bool>)>,
) -> Vec<SpanAttributeRef> {
    items
        .into_iter()
        .map(|(attr, req_level, sampling_relevant)| SpanAttributeRef {
            base: cat.attribute_ref(attr),
            requirement_level: req_level.clone(),
            sampling_relevant,
        })
        .collect()
}

/// Extracts resolved entity associations, returning an empty vector when none were resolved.
fn emit_resolved_associations(assocs: &EntityAssociations) -> Vec<ResolvedEntityAssociation> {
    match assocs {
        EntityAssociations::Resolved(resolved) => resolved.clone(),
        EntityAssociations::Raw(_) => Vec::new(),
    }
}

/// Converts a `ResolvedPublicGroup` into a schema-ready `V2AttributeGroup`.
fn emit_attribute_group(cat: &FinalCatalog, g: &ResolvedPublicGroup) -> V2AttributeGroup {
    let attributes = emit_attribute_refs(
        cat,
        g.attributes.iter().map(|(a, req)| (a, req)),
        |base, requirement_level| AttributeGroupAttributeRef {
            base,
            requirement_level,
        },
    );
    V2AttributeGroup {
        id: g.id.clone(),
        attributes,
        common: g.common.clone(),
        provenance: g.provenance.clone(),
    }
}

/// Converts a `ResolvedMetric` into a schema-ready `V2Metric`.
fn emit_metric(cat: &FinalCatalog, m: ResolvedMetric) -> V2Metric {
    let attributes = emit_attribute_refs(
        cat,
        m.attributes.iter().map(|(a, req)| (a, req)),
        |base, requirement_level| MetricAttributeRef {
            base,
            requirement_level,
        },
    );
    let entity_associations = emit_resolved_associations(&m.entity_associations);
    V2Metric {
        name: m.name,
        instrument: m.instrument,
        unit: m.unit,
        attributes,
        entity_associations,
        requirement_level: m.requirement_level,
        common: m.common,
        provenance: m.provenance,
    }
}

/// Converts a `ResolvedSpan` into a schema-ready `V2Span`.
fn emit_span(cat: &FinalCatalog, s: ResolvedSpan) -> V2Span {
    let attributes = emit_span_attribute_refs(
        cat,
        s.attributes.iter().map(|(a, req, samp)| (a, req, *samp)),
    );
    let entity_associations = emit_resolved_associations(&s.entity_associations);
    V2Span {
        r#type: s.r#type,
        kind: s.kind,
        name: s.name,
        attributes,
        entity_associations,
        requirement_level: s.requirement_level,
        common: s.common,
        provenance: s.provenance,
    }
}

/// Converts a `ResolvedEvent` into a schema-ready `V2Event`.
fn emit_event(cat: &FinalCatalog, e: ResolvedEvent) -> V2Event {
    let attributes = emit_attribute_refs(
        cat,
        e.attributes.iter().map(|(a, req)| (a, req)),
        |base, requirement_level| EventAttributeRef {
            base,
            requirement_level,
        },
    );
    let entity_associations = emit_resolved_associations(&e.entity_associations);
    V2Event {
        name: e.name,
        attributes,
        entity_associations,
        requirement_level: e.requirement_level,
        common: e.common,
        provenance: e.provenance,
    }
}

/// Converts a `ResolvedEntity` into a schema-ready `V2Entity`.
fn emit_entity(cat: &FinalCatalog, en: ResolvedEntity, sort_identity: bool) -> V2Entity {
    let mut identity: Vec<AttributeRef> = en
        .identity
        .iter()
        .map(|attr| cat.attribute_ref(attr))
        .collect();
    if sort_identity {
        identity.sort();
    }
    let description = emit_attribute_refs(
        cat,
        en.description.iter().map(|(a, req)| (a, req)),
        |base, requirement_level| EntityAttributeRef {
            base,
            requirement_level,
        },
    );
    V2Entity {
        r#type: en.r#type,
        identity,
        description,
        requirement_level: en.requirement_level,
        common: en.common,
        provenance: en.provenance,
    }
}

/// Combines base signals and explicit refinements into a refinement vector sorted by `id`.
fn emit_refinement_list<B: Clone, R, Out>(
    bases: &[B],
    base_id: impl Fn(&B) -> &SignalId,
    refined: Vec<(SignalId, R)>,
    wrap_base: impl Fn(SignalId, B) -> Out,
    wrap_refined: impl Fn(SignalId, R) -> Out,
) -> Vec<Out> {
    let mut keyed = Vec::with_capacity(bases.len() + refined.len());
    keyed.extend(bases.iter().map(|b| {
        let id = base_id(b).clone();
        (id.clone(), wrap_base(id, b.clone()))
    }));
    for (id, r) in refined {
        keyed.push((id.clone(), wrap_refined(id, r)));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, item)| item).collect()
}

/// Assembles the final `ResolvedTelemetrySchema` directly in V2 format.
#[allow(clippy::too_many_arguments)]
pub(crate) fn schema(
    schema_url: SchemaUrl,
    cat: FinalCatalog,
    groups: ExpandedGroups,
    signals: ResolvedSignals,
    refinements: ResolvedRefinements,
    imported: ImportedItems,
    dependencies: &BTreeSet<SchemaUrl>,
) -> V2Schema {
    let registry_attributes = cat.local_def_attribute_refs();

    let mut attribute_groups: Vec<V2AttributeGroup> = groups
        .public_groups()
        .iter()
        .chain(imported.attribute_groups.iter())
        .map(|g| emit_attribute_group(&cat, g))
        .collect();
    attribute_groups.sort_by(|a, b| a.id.cmp(&b.id));

    let mut metrics: Vec<V2Metric> = signals
        .metrics
        .into_iter()
        .chain(imported.metrics.into_iter().map(|i| i.item))
        .map(|m| emit_metric(&cat, m))
        .collect();
    metrics.sort_by(|a, b| a.name.cmp(&b.name));

    let mut spans: Vec<V2Span> = signals
        .spans
        .into_iter()
        .chain(imported.spans.into_iter().map(|i| i.item))
        .map(|s| emit_span(&cat, s))
        .collect();
    spans.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    let mut events: Vec<V2Event> = signals
        .events
        .into_iter()
        .chain(imported.events.into_iter().map(|i| i.item))
        .map(|e| emit_event(&cat, e))
        .collect();
    events.sort_by(|a, b| a.name.cmp(&b.name));

    let mut entities: Vec<V2Entity> = signals
        .entities
        .into_iter()
        .chain(imported.entities.into_iter().map(|i| i.item))
        .map(|en| emit_entity(&cat, en, true))
        .collect();
    entities.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    let metric_refinements = emit_refinement_list(
        &metrics,
        |m| &m.name,
        refinements.metrics,
        |id, metric| MetricRefinement { id, metric },
        |id, m| MetricRefinement {
            id,
            metric: emit_metric(&cat, m),
        },
    );
    let span_refinements = emit_refinement_list(
        &spans,
        |s| &s.r#type,
        refinements.spans,
        |id, span| SpanRefinement { id, span },
        |id, s| SpanRefinement {
            id,
            span: emit_span(&cat, s),
        },
    );
    let event_refinements = emit_refinement_list(
        &events,
        |e| &e.name,
        refinements.events,
        |id, event| EventRefinement { id, event },
        |id, e| EventRefinement {
            id,
            event: emit_event(&cat, e),
        },
    );
    let entity_refinements = emit_refinement_list(
        &entities,
        |en| &en.r#type,
        refinements.entities,
        |id, entity| EntityRefinement { id, entity },
        |id, en| EntityRefinement {
            id,
            entity: emit_entity(&cat, en, false),
        },
    );

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
