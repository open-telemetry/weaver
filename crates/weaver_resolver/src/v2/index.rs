// SPDX-License-Identifier: Apache-2.0

//! Local symbol tables and duplicate checks across V2 specification files.

use std::collections::BTreeMap;
use weaver_common::error::handle_errors;
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
use super::signals::SignalKind;
use crate::error::Error;

/// Local specification item paired with its source provenance.
#[derive(Debug, Clone)]
pub(crate) struct Indexed<T> {
    pub item: T,
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
    attributes: BTreeMap<String, Indexed<AttributeDef>>,
    attribute_groups: BTreeMap<SignalId, Indexed<AttributeGroup>>,
    metrics: BTreeMap<SignalId, Indexed<Metric>>,
    spans: BTreeMap<SignalId, Indexed<Span>>,
    events: BTreeMap<SignalId, Indexed<Event>>,
    entities: BTreeMap<SignalId, Indexed<Entity>>,
    metric_refinements: BTreeMap<SignalId, Indexed<MetricRefinement>>,
    span_refinements: BTreeMap<SignalId, Indexed<SpanRefinement>>,
    event_refinements: BTreeMap<SignalId, Indexed<EventRefinement>>,
    entity_refinements: BTreeMap<SignalId, Indexed<EntityRefinement>>,
    imports: Vec<IndexedImports>,
}

impl Index {
    /// Returns the indexed local attribute definitions keyed by attribute name.
    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<String, Indexed<AttributeDef>> {
        &self.attributes
    }

    /// Returns the indexed local attribute groups keyed by group ID.
    #[must_use]
    pub fn attribute_groups(&self) -> &BTreeMap<SignalId, Indexed<AttributeGroup>> {
        &self.attribute_groups
    }

    /// Returns the indexed local base metrics keyed by metric name.
    #[must_use]
    pub fn metrics(&self) -> &BTreeMap<SignalId, Indexed<Metric>> {
        &self.metrics
    }

    /// Returns the indexed local base spans keyed by span type.
    #[must_use]
    pub fn spans(&self) -> &BTreeMap<SignalId, Indexed<Span>> {
        &self.spans
    }

    /// Returns the indexed local base events keyed by event name.
    #[must_use]
    pub fn events(&self) -> &BTreeMap<SignalId, Indexed<Event>> {
        &self.events
    }

    /// Returns the indexed local base entities keyed by entity type.
    #[must_use]
    pub fn entities(&self) -> &BTreeMap<SignalId, Indexed<Entity>> {
        &self.entities
    }

    /// Returns the indexed metric refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn metric_refinements(&self) -> &BTreeMap<SignalId, Indexed<MetricRefinement>> {
        &self.metric_refinements
    }

    /// Returns the indexed span refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn span_refinements(&self) -> &BTreeMap<SignalId, Indexed<SpanRefinement>> {
        &self.span_refinements
    }

    /// Returns the indexed event refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn event_refinements(&self) -> &BTreeMap<SignalId, Indexed<EventRefinement>> {
        &self.event_refinements
    }

    /// Returns the indexed entity refinements keyed by normalized refinement ID.
    #[must_use]
    pub fn entity_refinements(&self) -> &BTreeMap<SignalId, Indexed<EntityRefinement>> {
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

/// Appends `(item, provenance)` to the candidate list for `key`.
fn push_raw<K: Ord, T>(
    map: &mut BTreeMap<K, Vec<(T, Provenance)>>,
    key: K,
    item: T,
    provenance: &Provenance,
) {
    map.entry(key).or_default().push((item, provenance.clone()));
}

/// Keeps the first definition per key and records a duplicate error when multiple exist.
fn collect_unique<K: Ord, T>(
    raw: BTreeMap<K, Vec<(T, Provenance)>>,
    errors: &mut Vec<Error>,
    on_duplicate: impl Fn(&K, Vec<Provenance>) -> Error,
) -> BTreeMap<K, Indexed<T>> {
    let mut out = BTreeMap::new();
    for (key, items) in raw {
        if items.len() > 1 {
            let provenances = items.iter().map(|(_, p)| p.clone()).collect();
            errors.push(on_duplicate(&key, provenances));
        }
        if let Some((item, provenance)) = items.into_iter().next() {
            _ = out.insert(key, Indexed { item, provenance });
        }
    }
    out
}

/// Keeps the first refinement per normalized ID and records duplicate errors on collisions with siblings or `bases`.
fn collect_unique_refinements<T, B>(
    raw: BTreeMap<SignalId, Vec<(T, Provenance)>>,
    bases: &BTreeMap<SignalId, Indexed<B>>,
    errors: &mut Vec<Error>,
) -> BTreeMap<SignalId, Indexed<T>> {
    let mut out = BTreeMap::new();
    for (id, items) in raw {
        if items.len() > 1 {
            errors.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances: items.iter().map(|(_, p)| p.clone()).collect(),
            });
        }
        if let Some(base) = bases.get(&id) {
            let mut provenances = vec![base.provenance.clone()];
            provenances.extend(items.iter().map(|(_, p)| p.clone()));
            errors.push(Error::DuplicateGroupId {
                group_id: id.to_string(),
                provenances,
            });
        }
        if let Some((item, provenance)) = items.into_iter().next() {
            _ = out.insert(id, Indexed { item, provenance });
        }
    }
    out
}

/// Builds the local symbol tables from specs, returning a fatal error if any duplicate definitions exist.
pub(crate) fn build(specs: &[SemConvSpecWithProvenance]) -> Result<Index, Error> {
    let mut raw_attributes = BTreeMap::new();
    let mut raw_groups = BTreeMap::new();
    let mut raw_metrics = BTreeMap::new();
    let mut raw_spans = BTreeMap::new();
    let mut raw_events = BTreeMap::new();
    let mut raw_entities = BTreeMap::new();
    let mut raw_metric_refinements = BTreeMap::new();
    let mut raw_span_refinements = BTreeMap::new();
    let mut raw_event_refinements = BTreeMap::new();
    let mut raw_entity_refinements = BTreeMap::new();
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
        let prov = &spec_with_prov.provenance;

        if let Some(imports) = v2_spec.imports() {
            indexed_imports.push(IndexedImports {
                imports: imports.clone(),
                origin: ImportOrigin::Explicit(prov.clone()),
            });
        }

        for attr in v2_spec.attributes() {
            push_raw(&mut raw_attributes, attr.key.clone(), attr.clone(), prov);
        }
        for group in v2_spec.attribute_groups() {
            let id = match group {
                AttributeGroup::Internal(i) => i.id.clone(),
                AttributeGroup::Public(p) => p.id.clone(),
            };
            push_raw(&mut raw_groups, id, group.clone(), prov);
        }
        for metric in v2_spec.metrics() {
            push_raw(&mut raw_metrics, metric.name.clone(), metric.clone(), prov);
        }
        for span in v2_spec.spans() {
            push_raw(&mut raw_spans, span.r#type.clone(), span.clone(), prov);
        }
        for event in v2_spec.events() {
            push_raw(&mut raw_events, event.name.clone(), event.clone(), prov);
        }
        for entity in v2_spec.entities() {
            push_raw(
                &mut raw_entities,
                entity.r#type.clone(),
                entity.clone(),
                prov,
            );
        }
        for metric_ref in v2_spec.metric_refinements() {
            let norm_id = normalize_refinement_id(SignalKind::Metric, &metric_ref.id);
            push_raw(
                &mut raw_metric_refinements,
                norm_id,
                metric_ref.clone(),
                prov,
            );
        }
        for span_ref in v2_spec.span_refinements() {
            let norm_id = normalize_refinement_id(SignalKind::Span, &span_ref.id);
            push_raw(&mut raw_span_refinements, norm_id, span_ref.clone(), prov);
        }
        for event_ref in v2_spec.event_refinements() {
            let norm_id = normalize_refinement_id(SignalKind::Event, &event_ref.id);
            push_raw(&mut raw_event_refinements, norm_id, event_ref.clone(), prov);
        }
        for entity_ref in v2_spec.entity_refinements() {
            let norm_id = normalize_refinement_id(SignalKind::Entity, &entity_ref.id);
            push_raw(
                &mut raw_entity_refinements,
                norm_id,
                entity_ref.clone(),
                prov,
            );
        }
    }

    let mut errors = Vec::new();

    let attributes = collect_unique(raw_attributes, &mut errors, |key, provenances| {
        Error::DuplicateAttributeId {
            attribute_id: key.clone(),
            group_ids: provenances.into_iter().map(|p| p.path).collect(),
        }
    });
    let attribute_groups = collect_unique(raw_groups, &mut errors, |id, provenances| {
        Error::DuplicateGroupId {
            group_id: id.to_string(),
            provenances,
        }
    });
    let metrics = collect_unique(raw_metrics, &mut errors, |name, provenances| {
        Error::DuplicateMetricName {
            metric_name: name.to_string(),
            provenances,
        }
    });
    let duplicate_group_name = |name: &SignalId, provenances| Error::DuplicateGroupName {
        group_name: name.to_string(),
        provenances,
    };
    let spans = collect_unique(raw_spans, &mut errors, duplicate_group_name);
    let events = collect_unique(raw_events, &mut errors, duplicate_group_name);
    let entities = collect_unique(raw_entities, &mut errors, duplicate_group_name);

    let metric_refinements =
        collect_unique_refinements(raw_metric_refinements, &metrics, &mut errors);
    let span_refinements = collect_unique_refinements(raw_span_refinements, &spans, &mut errors);
    let event_refinements = collect_unique_refinements(raw_event_refinements, &events, &mut errors);
    let entity_refinements =
        collect_unique_refinements(raw_entity_refinements, &entities, &mut errors);

    handle_errors(errors)?;

    Ok(Index {
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
    })
}
