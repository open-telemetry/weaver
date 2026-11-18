// SPDX-License-Identifier: Apache-2.0

//! Local symbol tables and duplicate checks across V2 specification files.

use std::collections::BTreeMap;
use weaver_semconv::provenance::Provenance;
use weaver_semconv::semconv::{SemConvSpecWithProvenance, Versioned};
use weaver_semconv::v2::attribute::AttributeDef;
use weaver_semconv::v2::attribute_group::AttributeGroup;
use weaver_semconv::v2::entity::{Entity, EntityRefinement};
use weaver_semconv::v2::event::{Event, EventRefinement};
use weaver_semconv::v2::metric::{Metric, MetricRefinement};
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::{Span, SpanRefinement};
use weaver_semconv::v2::Imports;
use weaver_semconv::Error as SemconvError;

use super::rules::normalize_refinement_id;
use crate::error::Error;

/// Local attribute definition paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedAttributeDef {
    pub def: AttributeDef,
    pub provenance: Provenance,
}

/// Local attribute group paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedAttributeGroup {
    pub group: AttributeGroup,
    pub provenance: Provenance,
}

/// Local base metric paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedMetric {
    pub metric: Metric,
    pub provenance: Provenance,
}

/// Local base span paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedSpan {
    pub span: Span,
    pub provenance: Provenance,
}

/// Local base event paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedEvent {
    pub event: Event,
    pub provenance: Provenance,
}

/// Local base entity paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedEntity {
    pub entity: Entity,
    pub provenance: Provenance,
}

/// Local metric refinement paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedMetricRefinement {
    pub refinement: MetricRefinement,
    pub provenance: Provenance,
}

/// Local span refinement paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedSpanRefinement {
    pub refinement: SpanRefinement,
    pub provenance: Provenance,
}

/// Local event refinement paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedEventRefinement {
    pub refinement: EventRefinement,
    pub provenance: Provenance,
}

/// Local entity refinement paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct IndexedEntityRefinement {
    pub refinement: EntityRefinement,
    pub provenance: Provenance,
}

/// Origin of an import block (explicitly authored vs synthesized by `--include-unreferenced`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportOrigin {
    Explicit(Provenance),
    IncludeUnreferenced,
}

impl ImportOrigin {
    /// Returns `true` if this import block was explicitly authored in a specification file.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        matches!(self, ImportOrigin::Explicit(_))
    }
}

/// An `imports:` block paired with its explicit or synthesized origin.
#[derive(Debug, Clone)]
pub(crate) struct IndexedImports {
    pub imports: Imports,
    pub origin: ImportOrigin,
}

/// Per-namespace symbol tables for a local V2 registry.
#[derive(Debug, Default)]
pub(crate) struct Index {
    attributes: BTreeMap<String, IndexedAttributeDef>,
    attribute_groups: BTreeMap<SignalId, IndexedAttributeGroup>,
    metrics: BTreeMap<SignalId, IndexedMetric>,
    spans: BTreeMap<SignalId, IndexedSpan>,
    events: BTreeMap<SignalId, IndexedEvent>,
    entities: BTreeMap<SignalId, IndexedEntity>,
    metric_refinements: BTreeMap<SignalId, IndexedMetricRefinement>,
    span_refinements: BTreeMap<SignalId, IndexedSpanRefinement>,
    event_refinements: BTreeMap<SignalId, IndexedEventRefinement>,
    entity_refinements: BTreeMap<SignalId, IndexedEntityRefinement>,
    imports: Vec<IndexedImports>,
}

impl Index {
    /// Returns the indexed local attribute definitions keyed by attribute name.
    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<String, IndexedAttributeDef> {
        &self.attributes
    }

    /// Returns the indexed local attribute groups keyed by group ID.
    #[must_use]
    pub fn attribute_groups(&self) -> &BTreeMap<SignalId, IndexedAttributeGroup> {
        &self.attribute_groups
    }

    /// Returns the indexed local base metrics keyed by metric name.
    #[must_use]
    pub fn metrics(&self) -> &BTreeMap<SignalId, IndexedMetric> {
        &self.metrics
    }

    /// Returns the indexed local base spans keyed by span type.
    #[must_use]
    pub fn spans(&self) -> &BTreeMap<SignalId, IndexedSpan> {
        &self.spans
    }

    /// Returns the indexed local base events keyed by event name.
    #[must_use]
    pub fn events(&self) -> &BTreeMap<SignalId, IndexedEvent> {
        &self.events
    }

    /// Returns the indexed local base entities keyed by entity type.
    #[must_use]
    pub fn entities(&self) -> &BTreeMap<SignalId, IndexedEntity> {
        &self.entities
    }

    /// Returns the indexed metric refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn metric_refinements(&self) -> &BTreeMap<SignalId, IndexedMetricRefinement> {
        &self.metric_refinements
    }

    /// Returns the indexed span refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn span_refinements(&self) -> &BTreeMap<SignalId, IndexedSpanRefinement> {
        &self.span_refinements
    }

    /// Returns the indexed event refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn event_refinements(&self) -> &BTreeMap<SignalId, IndexedEventRefinement> {
        &self.event_refinements
    }

    /// Returns the indexed entity refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn entity_refinements(&self) -> &BTreeMap<SignalId, IndexedEntityRefinement> {
        &self.entity_refinements
    }

    /// Returns all explicit and synthesized `imports:` blocks in declaration order.
    #[must_use]
    pub fn imports(&self) -> &[IndexedImports] {
        &self.imports
    }

    /// Appends an `IndexedImports` entry to the registry's import list.
    pub fn append_import(&mut self, imports: IndexedImports) {
        self.imports.push(imports);
    }
}

/// Builds the local symbol tables from specs, detecting duplicates.
pub(crate) fn build(specs: &[SemConvSpecWithProvenance]) -> Result<(Index, Vec<Error>), Error> {
    let mut raw_attributes: BTreeMap<String, Vec<(AttributeDef, Provenance)>> = BTreeMap::new();
    let mut raw_groups: BTreeMap<SignalId, Vec<(AttributeGroup, Provenance)>> = BTreeMap::new();
    let mut raw_metrics: BTreeMap<SignalId, Vec<(Metric, Provenance)>> = BTreeMap::new();
    let mut raw_spans: BTreeMap<SignalId, Vec<(Span, Provenance)>> = BTreeMap::new();
    let mut raw_events: BTreeMap<SignalId, Vec<(Event, Provenance)>> = BTreeMap::new();
    let mut raw_entities: BTreeMap<SignalId, Vec<(Entity, Provenance)>> = BTreeMap::new();
    let mut raw_metric_refinements: BTreeMap<SignalId, Vec<(MetricRefinement, Provenance)>> =
        BTreeMap::new();
    let mut raw_span_refinements: BTreeMap<SignalId, Vec<(SpanRefinement, Provenance)>> =
        BTreeMap::new();
    let mut raw_event_refinements: BTreeMap<SignalId, Vec<(EventRefinement, Provenance)>> =
        BTreeMap::new();
    let mut raw_entity_refinements: BTreeMap<SignalId, Vec<(EntityRefinement, Provenance)>> =
        BTreeMap::new();
    let mut indexed_imports = Vec::new();

    for spec_with_prov in specs {
        let v2_spec = match &spec_with_prov.spec {
            Versioned::V2(v2) => v2,
            Versioned::V1(_) => {
                return Err(Error::FailToResolveDefinition(
                    SemconvError::InvalidFileFormat {
                        field_key: "file_format".to_owned(),
                        field_value: "definition/1".to_owned(),
                    },
                ));
            }
        };

        if let Some(imports) = v2_spec.imports() {
            indexed_imports.push(IndexedImports {
                imports: imports.clone(),
                origin: ImportOrigin::Explicit(spec_with_prov.provenance.clone()),
            });
        }

        for attr in v2_spec.attributes() {
            raw_attributes
                .entry(attr.key.clone())
                .or_default()
                .push((attr.clone(), spec_with_prov.provenance.clone()));
        }

        for group in v2_spec.attribute_groups() {
            let id = match group {
                AttributeGroup::Internal(i) => i.id.clone(),
                AttributeGroup::Public(p) => p.id.clone(),
            };
            raw_groups
                .entry(id)
                .or_default()
                .push((group.clone(), spec_with_prov.provenance.clone()));
        }

        for metric in v2_spec.metrics() {
            raw_metrics
                .entry(metric.name.clone())
                .or_default()
                .push((metric.clone(), spec_with_prov.provenance.clone()));
        }

        for span in v2_spec.spans() {
            raw_spans
                .entry(span.r#type.clone())
                .or_default()
                .push((span.clone(), spec_with_prov.provenance.clone()));
        }

        for event in v2_spec.events() {
            raw_events
                .entry(event.name.clone())
                .or_default()
                .push((event.clone(), spec_with_prov.provenance.clone()));
        }

        for entity in v2_spec.entities() {
            raw_entities
                .entry(entity.r#type.clone())
                .or_default()
                .push((entity.clone(), spec_with_prov.provenance.clone()));
        }

        for metric_ref in v2_spec.metric_refinements() {
            let norm_id = normalize_refinement_id("metric", &metric_ref.id);
            raw_metric_refinements
                .entry(norm_id)
                .or_default()
                .push((metric_ref.clone(), spec_with_prov.provenance.clone()));
        }

        for span_ref in v2_spec.span_refinements() {
            let norm_id = normalize_refinement_id("span", &span_ref.id);
            raw_span_refinements
                .entry(norm_id)
                .or_default()
                .push((span_ref.clone(), spec_with_prov.provenance.clone()));
        }

        for event_ref in v2_spec.event_refinements() {
            let norm_id = normalize_refinement_id("event", &event_ref.id);
            raw_event_refinements
                .entry(norm_id)
                .or_default()
                .push((event_ref.clone(), spec_with_prov.provenance.clone()));
        }

        for entity_ref in v2_spec.entity_refinements() {
            let norm_id = normalize_refinement_id("entity", &entity_ref.id);
            raw_entity_refinements
                .entry(norm_id)
                .or_default()
                .push((entity_ref.clone(), spec_with_prov.provenance.clone()));
        }
    }

    let mut warnings = Vec::new();

    let mut attributes = BTreeMap::new();
    for (key, defs) in raw_attributes {
        if defs.len() > 1 {
            warnings.push(Error::DuplicateAttributeId {
                attribute_id: key.clone(),
                group_ids: defs.iter().map(|(_, p)| p.path.clone()).collect(),
            });
        }
        if let Some((def, provenance)) = defs.into_iter().next() {
            _ = attributes.insert(key, IndexedAttributeDef { def, provenance });
        }
    }

    let mut attribute_groups = BTreeMap::new();
    for (id, groups) in raw_groups {
        if groups.len() > 1 {
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: groups.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some((group, provenance)) = groups.into_iter().next() {
            _ = attribute_groups.insert(id, IndexedAttributeGroup { group, provenance });
        }
    }

    let mut metrics = BTreeMap::new();
    for (name, items) in raw_metrics {
        if items.len() > 1 {
            warnings.push(Error::DuplicateMetricName {
                metric_name: name.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some((metric, provenance)) = items.into_iter().next() {
            _ = metrics.insert(name, IndexedMetric { metric, provenance });
        }
    }

    let mut spans = BTreeMap::new();
    for (type_id, items) in raw_spans {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupName {
                group_name: type_id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some((span, provenance)) = items.into_iter().next() {
            _ = spans.insert(type_id, IndexedSpan { span, provenance });
        }
    }

    let mut events = BTreeMap::new();
    for (name, items) in raw_events {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupName {
                group_name: name.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some((event, provenance)) = items.into_iter().next() {
            _ = events.insert(name, IndexedEvent { event, provenance });
        }
    }

    let mut entities = BTreeMap::new();
    for (type_id, items) in raw_entities {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupName {
                group_name: type_id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some((entity, provenance)) = items.into_iter().next() {
            _ = entities.insert(type_id, IndexedEntity { entity, provenance });
        }
    }

    let mut metric_refinements = BTreeMap::new();
    for (id, items) in raw_metric_refinements {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some(base) = metrics.get(&id) {
            let mut provenances = vec![base.provenance.clone()];
            provenances.extend(items.iter().map(|(_, p)| p.clone()));
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances,
            });
        }
        if let Some((refinement, provenance)) = items.into_iter().next() {
            _ = metric_refinements.insert(
                id,
                IndexedMetricRefinement {
                    refinement,
                    provenance,
                },
            );
        }
    }

    let mut span_refinements = BTreeMap::new();
    for (id, items) in raw_span_refinements {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some(base) = spans.get(&id) {
            let mut provenances = vec![base.provenance.clone()];
            provenances.extend(items.iter().map(|(_, p)| p.clone()));
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances,
            });
        }
        if let Some((refinement, provenance)) = items.into_iter().next() {
            _ = span_refinements.insert(
                id,
                IndexedSpanRefinement {
                    refinement,
                    provenance,
                },
            );
        }
    }

    let mut event_refinements = BTreeMap::new();
    for (id, items) in raw_event_refinements {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some(base) = events.get(&id) {
            let mut provenances = vec![base.provenance.clone()];
            provenances.extend(items.iter().map(|(_, p)| p.clone()));
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances,
            });
        }
        if let Some((refinement, provenance)) = items.into_iter().next() {
            _ = event_refinements.insert(
                id,
                IndexedEventRefinement {
                    refinement,
                    provenance,
                },
            );
        }
    }

    let mut entity_refinements = BTreeMap::new();
    for (id, items) in raw_entity_refinements {
        if items.len() > 1 {
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some(base) = entities.get(&id) {
            let mut provenances = vec![base.provenance.clone()];
            provenances.extend(items.iter().map(|(_, p)| p.clone()));
            warnings.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances,
            });
        }
        if let Some((refinement, provenance)) = items.into_iter().next() {
            _ = entity_refinements.insert(
                id,
                IndexedEntityRefinement {
                    refinement,
                    provenance,
                },
            );
        }
    }

    Ok((
        Index {
            attributes,
            attribute_groups,
            metrics,
            spans,
            events,
            entities,
            metric_refinements,
            span_refinements,
            event_refinements,
            entity_refinements,
            imports: indexed_imports,
        },
        warnings,
    ))
}
