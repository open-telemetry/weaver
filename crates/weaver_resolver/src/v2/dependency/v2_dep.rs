// SPDX-License-Identifier: Apache-2.0

//! Native adapter reading V2 schemas as dependencies into the V2 resolution pass.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use weaver_resolved_schema::v2::attribute::{Attribute, AttributeRef};
use weaver_resolved_schema::v2::catalog::AttributeCatalog;
use weaver_resolved_schema::v2::entity::EntityAssociation;
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::RequirementLevel;

use super::{
    DepAttribute, DepImportedAttributeGroup, DepImportedEntity, DepImportedEvent,
    DepImportedMetric, DepImportedSpan, DepSignal, DepSignalPayload,
};
use crate::error::Error;
use crate::graph::entity_lookup::{
    v2_association_origins, v2_source_url, EntityLocation, EntityLookup,
};
use crate::graph::exclusion::is_excluded;
use crate::v2::signals::{EntityAssociations, SignalKind};
use crate::WeaverResolvedSchema;

/// Location of a signal in a resolved V2 schema (refinement or base registry slice index).
#[derive(Debug, Clone, Copy)]
enum SignalSlot {
    Refinement(usize),
    Base(usize),
}

/// Builds an `O(1)` signal lookup map preferring `refinements` over `bases` on identical IDs.
fn index_signal_slots<'a, R, B>(
    refinements: &'a [R],
    ref_id: impl Fn(&'a R) -> &'a str,
    bases: &'a [B],
    base_id: impl Fn(&'a B) -> &'a str,
) -> HashMap<String, SignalSlot> {
    let mut map = HashMap::with_capacity(refinements.len() + bases.len());
    for (idx, r) in refinements.iter().enumerate() {
        let _ = map
            .entry(ref_id(r).to_owned())
            .or_insert(SignalSlot::Refinement(idx));
    }
    for (idx, b) in bases.iter().enumerate() {
        let _ = map
            .entry(base_id(b).to_owned())
            .or_insert(SignalSlot::Base(idx));
    }
    map
}

/// Adapter over a resolved V2 schema dependency.
#[derive(Debug, Clone)]
pub(crate) struct V2Dependency {
    bundle: Arc<WeaverResolvedSchema>,
    deps_vec: Vec<SchemaUrl>,
    exported_attrs: HashMap<String, u32>,
    attr_group_index: HashMap<String, usize>,
    span_index: HashMap<String, SignalSlot>,
    metric_index: HashMap<String, SignalSlot>,
    event_index: HashMap<String, SignalSlot>,
    entity_index: HashMap<String, SignalSlot>,
}

impl V2Dependency {
    /// Creates a new adapter over a resolved V2 schema.
    #[cfg(test)]
    #[must_use]
    pub fn new(schema: Arc<V2Schema>) -> Self {
        let owned = Arc::unwrap_or_clone(schema);
        Self::from_bundle(Arc::new(WeaverResolvedSchema::V2(owned)))
    }

    /// Creates a new adapter wrapping a shared `Arc<WeaverResolvedSchema>` without cloning.
    #[must_use]
    pub fn from_bundle(bundle: Arc<WeaverResolvedSchema>) -> Self {
        let schema = bundle
            .as_v2()
            .expect("V2Dependency::from_bundle requires a V2 schema");
        let deps_vec: Vec<SchemaUrl> = schema.dependencies.iter().cloned().collect();
        let mut exported_attrs = HashMap::with_capacity(schema.registry.attributes.len());
        for aref in &schema.registry.attributes {
            if let Some(attr) = schema.attribute_catalog.attribute(aref) {
                let _ = exported_attrs.entry(attr.key.clone()).or_insert(aref.0);
            }
        }
        for group in &schema.registry.attribute_groups {
            for attr_ref in &group.attributes {
                if let Some(attr) = schema.attribute_catalog.attribute(&attr_ref.base) {
                    let _ = exported_attrs
                        .entry(attr.key.clone())
                        .or_insert(attr_ref.base.0);
                }
            }
        }

        let span_index = index_signal_slots(
            &schema.refinements.spans,
            |r| &r.id,
            &schema.registry.spans,
            |s| &s.r#type,
        );
        let metric_index = index_signal_slots(
            &schema.refinements.metrics,
            |r| &r.id,
            &schema.registry.metrics,
            |m| &m.name,
        );
        let event_index = index_signal_slots(
            &schema.refinements.events,
            |r| &r.id,
            &schema.registry.events,
            |e| &e.name,
        );
        let entity_index = index_signal_slots(
            &schema.refinements.entities,
            |r| &r.id,
            &schema.registry.entities,
            |en| &en.r#type,
        );
        let attr_group_index = schema
            .registry
            .attribute_groups
            .iter()
            .enumerate()
            .map(|(idx, ag)| (ag.id.to_string(), idx))
            .collect();

        Self {
            bundle,
            deps_vec,
            exported_attrs,
            attr_group_index,
            span_index,
            metric_index,
            event_index,
            entity_index,
        }
    }

    /// Returns a reference to the underlying `V2Schema`.
    fn schema(&self) -> &V2Schema {
        self.bundle
            .as_v2()
            .expect("V2Dependency bundle must hold a V2 schema")
    }

    /// Looks up an attribute definition by key.
    pub fn lookup_attribute(&self, key: &str) -> Result<Option<DepAttribute>, Error> {
        let Some(&idx) = self.exported_attrs.get(key) else {
            return Ok(None);
        };
        let Some((attribute, origin)) = self.resolve_catalog_attr(&AttributeRef(idx)) else {
            return Ok(None);
        };
        let excluded = is_excluded(&attribute.common.annotations);

        Ok(Some(DepAttribute {
            attribute,
            origin,
            excluded,
        }))
    }

    /// Looks up a signal by kind and id in `O(1)` time.
    pub fn lookup_signal(&self, kind: SignalKind, id: &str) -> Option<DepSignal> {
        let schema = self.schema();
        match kind {
            SignalKind::Span => {
                let span = match *self.span_index.get(id)? {
                    SignalSlot::Refinement(i) => &schema.refinements.spans[i].span,
                    SignalSlot::Base(i) => &schema.registry.spans[i],
                };
                let attributes = span
                    .attributes
                    .iter()
                    .filter_map(|aref| {
                        let (attr, origin) = self.resolve_catalog_attr(&aref.base)?;
                        Some((
                            attr,
                            aref.requirement_level.clone(),
                            aref.sampling_relevant,
                            origin,
                        ))
                    })
                    .collect();

                Some(DepSignal {
                    payload: DepSignalPayload::Span {
                        r#type: span.r#type.clone(),
                        kind: span.kind,
                        name: span.name.clone(),
                        requirement_level: span.requirement_level.clone(),
                        attributes,
                    },
                    common: span.common.clone(),
                    origin: self.source_url(span.provenance.source),
                    excluded: is_excluded(&span.common.annotations),
                    entity_associations: EntityAssociations::Resolved(
                        span.entity_associations.clone(),
                    ),
                    association_origins: self.association_origins(&span.entity_associations),
                })
            }
            SignalKind::Metric => {
                let metric = match *self.metric_index.get(id)? {
                    SignalSlot::Refinement(i) => &schema.refinements.metrics[i].metric,
                    SignalSlot::Base(i) => &schema.registry.metrics[i],
                };
                let attributes = self.resolve_signal_attrs(
                    metric
                        .attributes
                        .iter()
                        .map(|a| (&a.base, &a.requirement_level)),
                );

                Some(DepSignal {
                    payload: DepSignalPayload::Metric {
                        name: metric.name.clone(),
                        instrument: metric.instrument,
                        unit: metric.unit.clone(),
                        requirement_level: metric.requirement_level.clone(),
                        attributes,
                    },
                    common: metric.common.clone(),
                    origin: self.source_url(metric.provenance.source),
                    excluded: is_excluded(&metric.common.annotations),
                    entity_associations: EntityAssociations::Resolved(
                        metric.entity_associations.clone(),
                    ),
                    association_origins: self.association_origins(&metric.entity_associations),
                })
            }
            SignalKind::Event => {
                let event = match *self.event_index.get(id)? {
                    SignalSlot::Refinement(i) => &schema.refinements.events[i].event,
                    SignalSlot::Base(i) => &schema.registry.events[i],
                };
                let attributes = self.resolve_signal_attrs(
                    event
                        .attributes
                        .iter()
                        .map(|a| (&a.base, &a.requirement_level)),
                );

                Some(DepSignal {
                    payload: DepSignalPayload::Event {
                        name: event.name.clone(),
                        requirement_level: event.requirement_level.clone(),
                        attributes,
                    },
                    common: event.common.clone(),
                    origin: self.source_url(event.provenance.source),
                    excluded: is_excluded(&event.common.annotations),
                    entity_associations: EntityAssociations::Resolved(
                        event.entity_associations.clone(),
                    ),
                    association_origins: self.association_origins(&event.entity_associations),
                })
            }
            SignalKind::Entity => {
                let entity = match *self.entity_index.get(id)? {
                    SignalSlot::Refinement(i) => &schema.refinements.entities[i].entity,
                    SignalSlot::Base(i) => &schema.registry.entities[i],
                };
                let identity = entity
                    .identity
                    .iter()
                    .filter_map(|aref| self.resolve_catalog_attr(aref))
                    .collect();
                let description = self.resolve_signal_attrs(
                    entity
                        .description
                        .iter()
                        .map(|a| (&a.base, &a.requirement_level)),
                );

                Some(DepSignal {
                    payload: DepSignalPayload::Entity {
                        r#type: entity.r#type.clone(),
                        requirement_level: entity.requirement_level.clone(),
                        identity,
                        description,
                    },
                    common: entity.common.clone(),
                    origin: self.source_url(entity.provenance.source),
                    excluded: is_excluded(&entity.common.annotations),
                    entity_associations: EntityAssociations::Resolved(vec![]),
                    association_origins: BTreeMap::new(),
                })
            }
        }
    }

    /// Returns all importable metrics from this dependency.
    pub fn importable_metrics(&self) -> Vec<DepImportedMetric> {
        self.schema()
            .registry
            .metrics
            .iter()
            .map(|m| DepImportedMetric {
                name: m.name.clone(),
                instrument: m.instrument,
                unit: m.unit.clone(),
                requirement_level: m.requirement_level.clone(),
                attributes: self.resolve_import_attrs(
                    m.attributes.iter().map(|a| (&a.base, &a.requirement_level)),
                ),
                entity_associations: EntityAssociations::Resolved(m.entity_associations.clone()),
                association_origins: self.association_origins(&m.entity_associations),
                common: m.common.clone(),
                provenance: m.provenance.clone(),
                origin: self.source_url(m.provenance.source),
                excluded: is_excluded(&m.common.annotations),
                match_keys: vec![m.name.to_string()],
            })
            .collect()
    }

    /// Returns all importable spans from this dependency.
    pub fn importable_spans(&self) -> Vec<DepImportedSpan> {
        self.schema()
            .registry
            .spans
            .iter()
            .map(|s| {
                let attributes = s
                    .attributes
                    .iter()
                    .filter_map(|aref| {
                        let (attr, origin) = self.resolve_catalog_attr(&aref.base)?;
                        Some((
                            attr,
                            aref.requirement_level.clone(),
                            aref.sampling_relevant,
                            origin,
                            false,
                        ))
                    })
                    .collect();
                DepImportedSpan {
                    r#type: s.r#type.clone(),
                    kind: s.kind,
                    name: s.name.clone(),
                    requirement_level: s.requirement_level.clone(),
                    attributes,
                    entity_associations: EntityAssociations::Resolved(
                        s.entity_associations.clone(),
                    ),
                    association_origins: self.association_origins(&s.entity_associations),
                    common: s.common.clone(),
                    provenance: s.provenance.clone(),
                    origin: self.source_url(s.provenance.source),
                    excluded: is_excluded(&s.common.annotations),
                    match_keys: vec![s.r#type.to_string()],
                }
            })
            .collect()
    }

    /// Returns all importable events from this dependency.
    pub fn importable_events(&self) -> Vec<DepImportedEvent> {
        self.schema()
            .registry
            .events
            .iter()
            .map(|e| DepImportedEvent {
                name: e.name.clone(),
                requirement_level: e.requirement_level.clone(),
                attributes: self.resolve_import_attrs(
                    e.attributes.iter().map(|a| (&a.base, &a.requirement_level)),
                ),
                entity_associations: EntityAssociations::Resolved(e.entity_associations.clone()),
                association_origins: self.association_origins(&e.entity_associations),
                common: e.common.clone(),
                provenance: e.provenance.clone(),
                origin: self.source_url(e.provenance.source),
                excluded: is_excluded(&e.common.annotations),
                match_keys: vec![e.name.to_string()],
            })
            .collect()
    }

    /// Returns all importable entities from this dependency.
    pub fn importable_entities(&self) -> Vec<DepImportedEntity> {
        self.schema()
            .registry
            .entities
            .iter()
            .map(|en| {
                let identity = en
                    .identity
                    .iter()
                    .filter_map(|aref| {
                        let (attr, origin) = self.resolve_catalog_attr(aref)?;
                        Some((attr, origin, false))
                    })
                    .collect();
                DepImportedEntity {
                    r#type: en.r#type.clone(),
                    requirement_level: en.requirement_level.clone(),
                    identity,
                    description: self.resolve_import_attrs(
                        en.description
                            .iter()
                            .map(|a| (&a.base, &a.requirement_level)),
                    ),
                    common: en.common.clone(),
                    provenance: en.provenance.clone(),
                    origin: self.source_url(en.provenance.source),
                    excluded: is_excluded(&en.common.annotations),
                    match_keys: vec![en.r#type.to_string()],
                }
            })
            .collect()
    }

    /// Looks up a public attribute group by ID.
    pub fn lookup_attribute_group(&self, id: &str) -> Option<DepImportedAttributeGroup> {
        let &idx = self.attr_group_index.get(id)?;
        Some(self.build_imported_attribute_group(&self.schema().registry.attribute_groups[idx]))
    }

    /// Returns all importable attribute groups from this dependency.
    pub fn importable_attribute_groups(&self) -> Vec<DepImportedAttributeGroup> {
        self.schema()
            .registry
            .attribute_groups
            .iter()
            .map(|ag| self.build_imported_attribute_group(ag))
            .collect()
    }

    /// Converts a resolved V2 public attribute group into a [`DepImportedAttributeGroup`].
    fn build_imported_attribute_group(
        &self,
        ag: &weaver_resolved_schema::v2::attribute_group::AttributeGroup,
    ) -> DepImportedAttributeGroup {
        DepImportedAttributeGroup {
            id: ag.id.clone(),
            attributes: self.resolve_import_attrs(
                ag.attributes
                    .iter()
                    .map(|a| (&a.base, &a.requirement_level)),
            ),
            common: ag.common.clone(),
            provenance: ag.provenance.clone(),
            origin: self.source_url(ag.provenance.source),
            excluded: is_excluded(&ag.common.annotations),
            is_public: true,
            match_keys: vec![ag.id.to_string()],
        }
    }

    /// Resolves a catalog `AttributeRef` into its cloned `Attribute` and defining `SchemaUrl`.
    fn resolve_catalog_attr(&self, aref: &AttributeRef) -> Option<(Attribute, SchemaUrl)> {
        let attr = self.schema().attribute_catalog.attribute(aref)?;
        let origin = self.source_url(attr.provenance.source);
        Some((attr.clone(), origin))
    }

    /// Resolves `(AttributeRef, RequirementLevel)` pairs for signal refinement lookup.
    fn resolve_signal_attrs<'a>(
        &self,
        items: impl Iterator<Item = (&'a AttributeRef, &'a RequirementLevel)>,
    ) -> Vec<(Attribute, RequirementLevel, SchemaUrl)> {
        items
            .filter_map(|(aref, req)| {
                let (attr, origin) = self.resolve_catalog_attr(aref)?;
                Some((attr, req.clone(), origin))
            })
            .collect()
    }

    /// Resolves `(AttributeRef, RequirementLevel)` pairs for importable items (`is_v1_def = false`).
    fn resolve_import_attrs<'a>(
        &self,
        items: impl Iterator<Item = (&'a AttributeRef, &'a RequirementLevel)>,
    ) -> Vec<(Attribute, RequirementLevel, SchemaUrl, bool)> {
        items
            .filter_map(|(aref, req)| {
                let (attr, origin) = self.resolve_catalog_attr(aref)?;
                Some((attr, req.clone(), origin, false))
            })
            .collect()
    }

    /// Maps each non-excluded entity referenced in `associations` to its defining `SchemaUrl`.
    fn association_origins(
        &self,
        associations: &[EntityAssociation],
    ) -> BTreeMap<String, SchemaUrl> {
        v2_association_origins(self.schema(), &self.deps_vec, associations)
    }

    /// Resolves an optional `DependencyRef` index to its `SchemaUrl`, defaulting to this schema's URL.
    fn source_url(&self, source: Option<DependencyRef>) -> SchemaUrl {
        v2_source_url(self.schema(), &self.deps_vec, source)
    }
}

impl EntityLookup for V2Dependency {
    /// Looks up an entity's origin and exclusion status in the underlying V2 schema.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        self.schema().lookup_entity(name)
    }
}
