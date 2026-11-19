// SPDX-License-Identifier: Apache-2.0

//! Native adapter reading V2 schemas as dependencies into the V2 resolution pass.

use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
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
use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::graph::exclusion::is_excluded;
use crate::v2::signals::EntityAssociations;
use crate::WeaverResolvedSchema;

/// Reference-counted handle to a resolved V2 schema.
#[derive(Debug, Clone)]
enum SharedV2Schema {
    Bundle(Arc<WeaverResolvedSchema>),
    #[cfg(test)]
    Direct(Arc<V2Schema>),
}

impl Deref for SharedV2Schema {
    type Target = V2Schema;

    /// Returns a reference to the underlying `V2Schema`.
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Bundle(bundle) => bundle.as_v2().expect("SharedV2Schema::Bundle must hold V2"),
            #[cfg(test)]
            Self::Direct(schema) => schema,
        }
    }
}

/// Adapter over a resolved V2 schema dependency.
#[derive(Debug, Clone)]
pub(crate) struct V2Dependency {
    schema: SharedV2Schema,
    deps_vec: Vec<SchemaUrl>,
    exported_attrs: HashMap<String, u32>,
}

impl V2Dependency {
    /// Creates a new adapter over a resolved V2 schema.
    #[cfg(test)]
    #[must_use]
    pub fn new(schema: Arc<V2Schema>) -> Self {
        Self::from_shared(SharedV2Schema::Direct(schema))
    }

    /// Creates a new adapter wrapping a shared `Arc<WeaverResolvedSchema>` without cloning.
    #[must_use]
    pub fn from_bundle(bundle: Arc<WeaverResolvedSchema>) -> Self {
        assert!(
            bundle.as_v2().is_some(),
            "V2Dependency::from_bundle requires a V2 schema"
        );
        Self::from_shared(SharedV2Schema::Bundle(bundle))
    }

    /// Indexes linearized dependency URLs and exported attribute catalog indices from `schema`.
    fn from_shared(schema: SharedV2Schema) -> Self {
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
        Self {
            schema,
            deps_vec,
            exported_attrs,
        }
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

    /// Looks up a signal by kind and id.
    pub fn lookup_signal(&self, kind: &str, id: &str) -> Option<DepSignal> {
        match kind {
            "span" => {
                let span = self
                    .schema
                    .refinements
                    .spans
                    .iter()
                    .find(|r| &*r.id == id)
                    .map(|r| &r.span)
                    .or_else(|| self.schema.registry.spans.iter().find(|s| &*s.r#type == id))?;

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
            "metric" => {
                let metric = self
                    .schema
                    .refinements
                    .metrics
                    .iter()
                    .find(|r| &*r.id == id)
                    .map(|r| &r.metric)
                    .or_else(|| self.schema.registry.metrics.iter().find(|m| &*m.name == id))?;

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
            "event" => {
                let event = self
                    .schema
                    .refinements
                    .events
                    .iter()
                    .find(|r| &*r.id == id)
                    .map(|r| &r.event)
                    .or_else(|| self.schema.registry.events.iter().find(|e| &*e.name == id))?;

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
            "entity" => {
                let entity = self
                    .schema
                    .refinements
                    .entities
                    .iter()
                    .find(|r| &*r.id == id)
                    .map(|r| &r.entity)
                    .or_else(|| {
                        self.schema
                            .registry
                            .entities
                            .iter()
                            .find(|en| &*en.r#type == id)
                    })?;

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
            _ => None,
        }
    }

    /// Looks up a signal by id across all kinds.
    pub fn lookup_signal_any_kind(&self, id: &str) -> Option<(&'static str, DepSignal)> {
        for kind in ["metric", "span", "event", "entity"] {
            if let Some(s) = self.lookup_signal(kind, id) {
                return Some((kind, s));
            }
        }
        None
    }

    /// Returns all importable metrics from this dependency.
    pub fn importable_metrics(&self) -> Vec<DepImportedMetric> {
        self.schema
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
        self.schema
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
        self.schema
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
        self.schema
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

    /// Returns all importable attribute groups from this dependency.
    pub fn importable_attribute_groups(&self) -> Vec<DepImportedAttributeGroup> {
        self.schema
            .registry
            .attribute_groups
            .iter()
            .map(|ag| DepImportedAttributeGroup {
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
            })
            .collect()
    }

    /// Resolves a catalog `AttributeRef` into its cloned `Attribute` and defining `SchemaUrl`.
    fn resolve_catalog_attr(&self, aref: &AttributeRef) -> Option<(Attribute, SchemaUrl)> {
        let attr = self.schema.attribute_catalog.attribute(aref)?;
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

    /// Maps each entity referenced in `associations` to its defining `SchemaUrl`.
    fn association_origins(
        &self,
        associations: &[EntityAssociation],
    ) -> BTreeMap<String, SchemaUrl> {
        associations
            .iter()
            .flat_map(|assoc| assoc.refs())
            .filter_map(|entity_ref| {
                let origin = match entity_ref.provenance.source {
                    Some(dep_ref) => self.deps_vec.get(dep_ref.0 as usize).cloned(),
                    None => self
                        .schema
                        .lookup_entity(&entity_ref.r#type)
                        .filter(|location| !location.excluded)
                        .map(|location| location.origin)
                        .or_else(|| Some(self.schema.schema_url.clone())),
                };
                origin.map(|origin| (entity_ref.r#type.to_string(), origin))
            })
            .collect()
    }

    /// Resolves an optional `DependencyRef` index to its `SchemaUrl`, defaulting to this schema's URL.
    fn source_url(&self, source: Option<DependencyRef>) -> SchemaUrl {
        source
            .and_then(|dep_ref| self.deps_vec.get(dep_ref.0 as usize).cloned())
            .unwrap_or_else(|| self.schema.schema_url.clone())
    }
}

impl EntityLookup for V2Dependency {
    /// Looks up an entity's origin and exclusion status in the underlying V2 schema.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        self.schema.lookup_entity(name)
    }
}
