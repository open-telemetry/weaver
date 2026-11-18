// SPDX-License-Identifier: Apache-2.0

//! Resolution of base V2 signals: metrics, spans, events, and entities.

use std::collections::{BTreeMap, HashSet};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::entity::EntityAssociation as ResolvedEntityAssociation;
use weaver_resolved_schema::v2::provenance::Provenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::{AttributeRef, RequirementLevel};
use weaver_semconv::v2::entity_association::EntityAssociation as SemconvEntityAssociation;
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::span::{SpanKindSpec, SpanName};
use weaver_semconv::v2::CommonFields;

use super::attributes::Catalog;
use super::groups::{self, ExpandedGroups};
use super::index::Index;
use super::rules;
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::exclusion::is_excluded;

/// Unresolved or resolved entity associations on a signal.
#[derive(Debug, Clone)]
pub(crate) enum EntityAssociations {
    Raw(Vec<SemconvEntityAssociation>),
    Resolved(Vec<ResolvedEntityAssociation>),
}

impl Default for EntityAssociations {
    fn default() -> Self {
        EntityAssociations::Raw(Vec::new())
    }
}

impl EntityAssociations {
    /// Returns `true` if there are no entity associations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            EntityAssociations::Raw(v) => v.is_empty(),
            EntityAssociations::Resolved(v) => v.is_empty(),
        }
    }
}

/// Resolved metric signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedMetric {
    pub name: SignalId,
    pub instrument: InstrumentSpec,
    pub unit: String,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved span signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedSpan {
    pub r#type: SignalId,
    pub kind: SpanKindSpec,
    pub name: SpanName,
    pub attributes: Vec<(Attribute, RequirementLevel, Option<bool>)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved event signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedEvent {
    pub name: SignalId,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved entity definition with identity and description attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedEntity {
    pub r#type: SignalId,
    pub identity: Vec<Attribute>,
    pub description: Vec<(Attribute, RequirementLevel)>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// All resolved local base signals in a registry.
#[derive(Debug, Default)]
pub(crate) struct ResolvedSignals {
    pub metrics: Vec<ResolvedMetric>,
    pub spans: Vec<ResolvedSpan>,
    pub events: Vec<ResolvedEvent>,
    pub entities: Vec<ResolvedEntity>,
}

/// Resolves all base signals in the index against expanded groups and catalog.
pub(crate) fn resolve_bases<'a, C: SchemaCacheLookup>(
    index: &Index,
    groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
) -> Result<ResolvedSignals, Error> {
    let mut validation_errors = Vec::new();

    let mut metrics = Vec::with_capacity(index.metrics().len());
    for (name, indexed_m) in index.metrics() {
        let m = &indexed_m.metric;
        let provenance = Provenance {
            source: None,
            path: indexed_m.provenance.path.clone(),
        };
        let m_excluded = is_excluded(&m.common.annotations);

        let used_in = format!("metric.{name}");
        let expanded_items = groups::expand_signal_attributes(
            &m.attributes,
            groups,
            name,
            "metric",
            m_excluded,
            &indexed_m.provenance,
        )?;

        let mut attributes = Vec::with_capacity(expanded_items.len());
        for item in expanded_items {
            let resolved_attr = catalog.resolve_attribute_ref(
                &item.aref,
                &used_in,
                false,
                m_excluded,
                Some(&indexed_m.provenance),
            )?;
            let req_level = rules::resolve_requirement_level(&item.aref);
            attributes.push((resolved_attr, req_level));
        }
        attributes.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

        metrics.push(ResolvedMetric {
            name: name.clone(),
            instrument: m.instrument,
            unit: m.unit.clone(),
            attributes,
            entity_associations: EntityAssociations::Raw(m.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: m.requirement_level.clone(),
            common: m.common.clone(),
            provenance,
        });
    }
    metrics.sort_by(|a, b| a.name.cmp(&b.name));

    let mut spans = Vec::with_capacity(index.spans().len());
    for (type_id, indexed_s) in index.spans() {
        let s = &indexed_s.span;
        let provenance = Provenance {
            source: None,
            path: indexed_s.provenance.path.clone(),
        };
        let s_excluded = is_excluded(&s.common.annotations);
        let used_in = format!("span.{type_id}");

        let expanded_items = groups::expand_span_attributes(
            &s.attributes,
            groups,
            type_id,
            s_excluded,
            &indexed_s.provenance,
        )?;

        let mut attributes = Vec::with_capacity(expanded_items.len());
        let mut span_attr_keys = HashSet::with_capacity(expanded_items.len());

        for item in expanded_items {
            let resolved_attr = catalog.resolve_attribute_ref(
                &item.aref,
                &used_in,
                false,
                s_excluded,
                Some(&indexed_s.provenance),
            )?;
            _ = span_attr_keys.insert(resolved_attr.key.clone());
            let req_level = rules::resolve_requirement_level(&item.aref);
            attributes.push((resolved_attr, req_level, item.sampling_relevant));
        }
        attributes.sort_by(|(a, _, _), (b, _, _)| a.key.cmp(&b.key));

        for template in &s.name.templates {
            for attr_key in &template.attributes {
                if !span_attr_keys.contains(attr_key) {
                    validation_errors.push(Error::SpanNameAttributeNotOnSpan {
                        span_id: type_id.to_string(),
                        attribute_key: attr_key.clone(),
                        provenance: Some(Box::new(indexed_s.provenance.clone())),
                    });
                }
            }
        }

        spans.push(ResolvedSpan {
            r#type: type_id.clone(),
            kind: s.kind,
            name: s.name.clone(),
            attributes,
            entity_associations: EntityAssociations::Raw(s.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: s.requirement_level.clone(),
            common: s.common.clone(),
            provenance,
        });
    }
    spans.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    let mut events = Vec::with_capacity(index.events().len());
    for (name, indexed_e) in index.events() {
        let e = &indexed_e.event;
        let provenance = Provenance {
            source: None,
            path: indexed_e.provenance.path.clone(),
        };
        let e_excluded = is_excluded(&e.common.annotations);
        let used_in = format!("event.{name}");

        let expanded_items = groups::expand_signal_attributes(
            &e.attributes,
            groups,
            name,
            "event",
            e_excluded,
            &indexed_e.provenance,
        )?;

        let mut attributes = Vec::with_capacity(expanded_items.len());
        for item in expanded_items {
            let resolved_attr = catalog.resolve_attribute_ref(
                &item.aref,
                &used_in,
                false,
                e_excluded,
                Some(&indexed_e.provenance),
            )?;
            let req_level = rules::resolve_requirement_level(&item.aref);
            attributes.push((resolved_attr, req_level));
        }
        attributes.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

        events.push(ResolvedEvent {
            name: name.clone(),
            attributes,
            entity_associations: EntityAssociations::Raw(e.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: e.requirement_level.clone(),
            common: e.common.clone(),
            provenance,
        });
    }
    events.sort_by(|a, b| a.name.cmp(&b.name));

    let mut entities = Vec::with_capacity(index.entities().len());
    for (type_id, indexed_en) in index.entities() {
        let en = &indexed_en.entity;
        let provenance = Provenance {
            source: None,
            path: indexed_en.provenance.path.clone(),
        };
        let en_excluded = is_excluded(&en.common.annotations);
        let used_in = format!("entity.{type_id}");

        if en.identity.is_empty() {
            validation_errors.push(Error::EntityMissingIdentity {
                entity_type: type_id.to_string(),
                provenance: Some(Box::new(indexed_en.provenance.clone())),
            });
        }

        let mut identity = Vec::with_capacity(en.identity.len());
        for id_ref in &en.identity {
            let fake_aref = AttributeRef {
                r#ref: id_ref.r#ref.clone(),
                brief: id_ref.brief.clone(),
                examples: id_ref.examples.clone(),
                requirement_level: None,
                note: id_ref.note.clone(),
                annotations: id_ref.annotations.clone(),
            };
            let resolved_attr = catalog.resolve_attribute_ref(
                &fake_aref,
                &used_in,
                false,
                en_excluded,
                Some(&indexed_en.provenance),
            )?;
            identity.push(resolved_attr);
        }
        identity.sort_by(|a, b| a.key.cmp(&b.key));

        let mut description = Vec::with_capacity(en.description.len());
        for desc_ref in &en.description {
            let resolved_attr = catalog.resolve_attribute_ref(
                desc_ref,
                &used_in,
                false,
                en_excluded,
                Some(&indexed_en.provenance),
            )?;
            let req_level = rules::resolve_requirement_level(desc_ref);
            description.push((resolved_attr, req_level));
        }
        description.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

        entities.push(ResolvedEntity {
            r#type: type_id.clone(),
            identity,
            description,
            requirement_level: en.requirement_level.clone(),
            common: en.common.clone(),
            provenance,
        });
    }
    entities.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    handle_errors(validation_errors)?;

    Ok(ResolvedSignals {
        metrics,
        spans,
        events,
        entities,
    })
}
