// SPDX-License-Identifier: Apache-2.0

//! Adapter for reading resolved V1 dependency schemas in V2 resolution; the only module in `v2/` referencing V1 types.

#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Deref;
use std::sync::Arc;
use weaver_resolved_schema::convert::v2_namespace_id;
use weaver_resolved_schema::v1::attribute::Attribute as V1Attribute;
use weaver_resolved_schema::v1::registry::Group;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::attribute::Attribute as V2Attribute;
use weaver_resolved_schema::v2::entity::{
    EntityAssociation as V2EntityAssociation, EntityRef as V2EntityRef,
};
use weaver_resolved_schema::v2::provenance::Provenance;
use weaver_semconv::convert::{
    v1_attribute_type_to_v2, v1_examples_to_v2, v1_instrument_to_v2, v1_requirement_level_to_v2,
    v1_span_kind_to_v2, v1_span_name_to_v2, v1_stability_and_deprecated_to_v2,
};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::entity_association::EntityAssociation as V1EntityAssociation;
use weaver_semconv::v1::group::{AttributeGroupVisibilitySpec, GroupType};
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::{SpanKindSpec, SpanName};
use weaver_semconv::v2::CommonFields;
use weaver_semconv::AttributeRole;

use super::{
    DepAttribute, DepImportedAttributeGroup, DepImportedEntity, DepImportedEvent,
    DepImportedMetric, DepImportedSpan, DepSignal, DepSignalPayload,
};
use crate::error::Error;
use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::graph::exclusion::is_excluded;
use crate::v2::signals::EntityAssociations;
use crate::WeaverResolvedSchema;

/// Reference-counted handle to a resolved V1 schema.
#[derive(Debug, Clone)]
enum SharedV1Schema {
    Bundle(Arc<WeaverResolvedSchema>),
    #[cfg(test)]
    Direct(Arc<V1Schema>),
}

impl Deref for SharedV1Schema {
    type Target = V1Schema;

    /// Returns a reference to the underlying `V1Schema`.
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Bundle(bundle) => bundle.as_v1().expect("SharedV1Schema::Bundle must hold V1"),
            #[cfg(test)]
            Self::Direct(schema) => schema,
        }
    }
}

#[cfg(test)]
thread_local! {
    static V1_ADAPTER_DISALLOWED: Cell<bool> = const { Cell::new(false) };
}

/// A test-only RAII scope that asserts `V1Dependency` is never instantiated or called.
#[cfg(test)]
pub(crate) struct DisallowV1AdapterScope;

#[cfg(test)]
impl DisallowV1AdapterScope {
    /// Activates the check on the current test thread.
    #[must_use]
    pub(crate) fn new() -> Self {
        V1_ADAPTER_DISALLOWED.with(|c| c.set(true));
        Self
    }
}

#[cfg(test)]
impl Default for DisallowV1AdapterScope {
    /// Activates the V1 adapter guard on the current thread.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Drop for DisallowV1AdapterScope {
    /// Clears the V1 adapter guard on the current thread when the scope exits.
    fn drop(&mut self) {
        V1_ADAPTER_DISALLOWED.with(|c| c.set(false));
    }
}

/// Adapter over a resolved V1 schema dependency.
#[derive(Debug, Clone)]
pub(crate) struct V1Dependency {
    schema: SharedV1Schema,
    my_schema_url: Option<SchemaUrl>,
    group_origins: HashMap<String, SchemaUrl>,
    dep_name_urls: HashMap<String, SchemaUrl>,
}

impl V1Dependency {
    /// Creates a new adapter over a resolved V1 schema.
    #[cfg(test)]
    #[must_use]
    pub fn new(schema: V1Schema) -> Self {
        Self::from_shared(SharedV1Schema::Direct(Arc::new(schema)))
    }

    /// Creates a new adapter wrapping a shared `Arc<WeaverResolvedSchema>` without cloning.
    #[must_use]
    pub fn from_bundle(bundle: Arc<WeaverResolvedSchema>) -> Self {
        assert!(
            bundle.as_v1().is_some(),
            "V1Dependency::from_bundle requires a V1 schema"
        );
        Self::from_shared(SharedV1Schema::Bundle(bundle))
    }

    /// Indexes dependency URLs and group provenance origins from `schema`.
    fn from_shared(schema: SharedV1Schema) -> Self {
        #[cfg(test)]
        V1_ADAPTER_DISALLOWED.with(|c| {
            if c.get() {
                panic!("V1Dependency was invoked while DisallowV1AdapterScope was active!");
            }
        });
        let my_schema_url = SchemaUrl::try_from(schema.schema_url.as_str()).ok();
        let mut group_origins = HashMap::with_capacity(schema.registry.groups.len());
        let mut dep_name_urls = HashMap::with_capacity(schema.dependencies.len());
        for url in &schema.dependencies {
            let _ = dep_name_urls
                .entry(url.name().to_owned())
                .or_insert_with(|| url.clone());
        }
        for group in &schema.registry.groups {
            if let Some(prov) = group.provenance() {
                let _ = group_origins
                    .entry(group.id.clone())
                    .or_insert_with(|| prov.schema_url.clone());
                let _ = dep_name_urls
                    .entry(prov.schema_url.name().to_owned())
                    .or_insert(prov.schema_url);
            }
        }
        Self {
            schema,
            my_schema_url,
            group_origins,
            dep_name_urls,
        }
    }

    /// The schema URL of this dependency.
    #[must_use]
    #[allow(dead_code)]
    pub fn schema_url(&self) -> SchemaUrl {
        self.my_schema_url
            .clone()
            .unwrap_or_else(SchemaUrl::new_unknown)
    }

    /// Collects direct and transitive dependencies declared by this schema.
    #[allow(dead_code)]
    pub fn collect_dependencies(&self, out: &mut BTreeSet<SchemaUrl>) {
        for dep in &self.schema.dependencies {
            _ = out.insert(dep.clone());
        }
    }

    /// Looks up an attribute definition by key.
    pub fn lookup_attribute(&self, key: &str) -> Result<Option<DepAttribute>, Error> {
        let Some(my_schema_url) = self.my_schema_url.as_ref() else {
            return Err(Error::InvalidUrl {
                url: self.schema.schema_url.clone(),
                error: SchemaUrl::try_from(self.schema.schema_url.as_str())
                    .expect_err("my_schema_url is None only when schema_url is invalid"),
            });
        };

        if let Some((attr, source_group)) = self.schema.catalog.root_attribute_definition(key) {
            let origin = self.find_attribute_origin(source_group, my_schema_url);
            let (v2_attr, excluded) = Self::convert_v1_attribute(attr);
            return Ok(Some(DepAttribute {
                attribute: v2_attr,
                origin,
                excluded,
            }));
        }

        // Fallback for deserialized schemas without root attributes:
        for group in &self.schema.registry.groups {
            for attr_ref in &group.attributes {
                if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                    if a.name == key {
                        let origin = group
                            .provenance()
                            .map(|prov| prov.schema_url)
                            .unwrap_or_else(|| my_schema_url.clone());
                        let (v2_attr, excluded) = Self::convert_v1_attribute(a);
                        return Ok(Some(DepAttribute {
                            attribute: v2_attr,
                            origin,
                            excluded,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Looks up a signal by kind and id.
    pub fn lookup_signal(&self, kind: &str, id: &str) -> Option<DepSignal> {
        let my_schema_url = self.my_schema_url.as_ref()?;
        let expected_group_type = match kind {
            "span" => GroupType::Span,
            "metric" => GroupType::Metric,
            "event" => GroupType::Event,
            "entity" => GroupType::Entity,
            _ => return None,
        };

        let group = self.schema.registry.groups.iter().find(|g| {
            if g.r#type != expected_group_type {
                return false;
            }
            v2_namespace_id(g)
                .map(|sig_id| &*sig_id == id)
                .unwrap_or(false)
        })?;

        let (stability, deprecated) =
            v1_stability_and_deprecated_to_v2(group.stability.clone(), group.deprecated.clone());

        let common = CommonFields {
            brief: group.brief.clone(),
            note: group.note.clone(),
            stability: stability.unwrap_or_default(),
            deprecated,
            annotations: group.annotations.clone().unwrap_or_default(),
        };

        let origin = group
            .provenance()
            .map(|p| p.schema_url)
            .unwrap_or(my_schema_url.clone());

        let excluded = group.annotations.as_ref().is_some_and(is_excluded);

        let payload = match expected_group_type {
            GroupType::Metric => {
                let instrument = group
                    .instrument
                    .clone()
                    .map(v1_instrument_to_v2)
                    .unwrap_or(InstrumentSpec::UpDownCounter);
                let unit = group.unit.clone().unwrap_or_default();
                let metric_name = group
                    .metric_name
                    .clone()
                    .map(SignalId::from)
                    .unwrap_or_else(|| SignalId::from(id));
                let requirement_level = group.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::with_capacity(group.attributes.len());
                for attr_ref in &group.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, my_schema_url)
                        } else {
                            self.find_attribute_origin(&group.id, my_schema_url)
                        };
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, attr_origin));
                    }
                }

                DepSignalPayload::Metric {
                    name: metric_name,
                    instrument,
                    unit,
                    requirement_level,
                    attributes,
                }
            }
            GroupType::Span => {
                let kind = group
                    .span_kind
                    .clone()
                    .map(v1_span_kind_to_v2)
                    .unwrap_or(SpanKindSpec::Internal);
                let span_name = group
                    .span_name
                    .clone()
                    .map(v1_span_name_to_v2)
                    .unwrap_or_else(|| SpanName {
                        templates: vec![],
                        note: group.name.clone(),
                    });
                let requirement_level = group.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::with_capacity(group.attributes.len());
                for attr_ref in &group.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, my_schema_url)
                        } else {
                            self.find_attribute_origin(&group.id, my_schema_url)
                        };
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, a.sampling_relevant, attr_origin));
                    }
                }

                DepSignalPayload::Span {
                    r#type: SignalId::from(id),
                    kind,
                    name: span_name,
                    requirement_level,
                    attributes,
                }
            }
            GroupType::Event => {
                let name = group
                    .name
                    .clone()
                    .map(SignalId::from)
                    .unwrap_or_else(|| SignalId::from(id));
                let requirement_level = group.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::with_capacity(group.attributes.len());
                for attr_ref in &group.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, my_schema_url)
                        } else {
                            self.find_attribute_origin(&group.id, my_schema_url)
                        };
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, attr_origin));
                    }
                }

                DepSignalPayload::Event {
                    name,
                    requirement_level,
                    attributes,
                }
            }
            GroupType::Entity => {
                let r#type = group
                    .name
                    .clone()
                    .map(SignalId::from)
                    .unwrap_or_else(|| SignalId::from(id));
                let requirement_level = group.requirement_level.clone().map(Into::into);

                let mut identity = Vec::new();
                let mut description = Vec::new();

                for attr_ref in &group.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, my_schema_url)
                        } else {
                            self.find_attribute_origin(&group.id, my_schema_url)
                        };
                        if a.role == Some(AttributeRole::Identifying) {
                            identity.push((v2_attr, attr_origin));
                        } else {
                            let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                            description.push((v2_attr, req, attr_origin));
                        }
                    }
                }

                DepSignalPayload::Entity {
                    r#type,
                    requirement_level,
                    identity,
                    description,
                }
            }
            _ => return None,
        };

        let (entity_associations, association_origins) = match group.r#type {
            GroupType::Metric | GroupType::Span | GroupType::Event => (
                EntityAssociations::Resolved(
                    group
                        .entity_associations
                        .iter()
                        .map(Self::convert_entity_association)
                        .collect(),
                ),
                self.association_origins(group),
            ),
            _ => (EntityAssociations::Raw(vec![]), BTreeMap::new()),
        };

        Some(DepSignal {
            payload,
            common,
            origin,
            excluded,
            entity_associations,
            association_origins,
        })
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

    /// Resolves the defining `SchemaUrl` for an attribute's `source_group`, falling back to `my_schema_url`.
    fn find_attribute_origin(&self, source_group: &str, my_schema_url: &SchemaUrl) -> SchemaUrl {
        if let Some(schema_name) = source_group.strip_prefix("v2_dependency.") {
            self.dep_name_urls
                .get(schema_name)
                .cloned()
                .unwrap_or_else(|| my_schema_url.clone())
        } else {
            self.group_origins
                .get(source_group)
                .cloned()
                .unwrap_or_else(|| my_schema_url.clone())
        }
    }

    /// Converts a resolved V1 attribute into a V2 `Attribute` and its exclusion flag.
    fn convert_v1_attribute(a: &V1Attribute) -> (V2Attribute, bool) {
        let r#type = v1_attribute_type_to_v2(a.r#type.clone());
        let examples = a.examples.clone().map(v1_examples_to_v2);
        let (stability, deprecated) =
            v1_stability_and_deprecated_to_v2(a.stability.clone(), a.deprecated.clone());
        let excluded = a.annotations.as_ref().is_some_and(is_excluded);
        let attr = V2Attribute {
            key: a.name.clone(),
            r#type,
            examples,
            common: CommonFields {
                brief: a.brief.clone(),
                note: (!a.note.is_empty()).then_some(a.note.clone()),
                stability: stability.unwrap_or_default(),
                deprecated,
                annotations: a.annotations.clone().unwrap_or_default(),
            },
            provenance: Provenance::default(),
        };
        (attr, excluded)
    }

    /// Returns all importable metrics from this dependency.
    pub fn importable_metrics(&self) -> Vec<DepImportedMetric> {
        let my_schema_url = self.schema_url();
        self.schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Metric || g.r#type == GroupType::MetricGroup)
            .map(|g| {
                let (stability, deprecated) =
                    v1_stability_and_deprecated_to_v2(g.stability.clone(), g.deprecated.clone());
                let common = CommonFields {
                    brief: g.brief.clone(),
                    note: g.note.clone(),
                    stability: stability.unwrap_or_default(),
                    deprecated,
                    annotations: g.annotations.clone().unwrap_or_default(),
                };
                let origin = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                let excluded = g.annotations.as_ref().is_some_and(is_excluded);
                let instrument = g
                    .instrument
                    .clone()
                    .map(v1_instrument_to_v2)
                    .unwrap_or(InstrumentSpec::UpDownCounter);
                let unit = g.unit.clone().unwrap_or_default();
                let metric_name = g
                    .metric_name
                    .clone()
                    .map(SignalId::from)
                    .or_else(|| v2_namespace_id(g))
                    .unwrap_or_else(|| SignalId::from(g.id.as_str()));
                let requirement_level = g.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::new();
                for attr_ref in &g.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, &my_schema_url)
                        } else {
                            self.find_attribute_origin(&g.id, &my_schema_url)
                        };
                        let is_v1_def = g
                            .lineage
                            .as_ref()
                            .and_then(|l| l.attribute(&a.name))
                            .is_none();
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, attr_origin, is_v1_def));
                    }
                }

                let entity_associations = EntityAssociations::Resolved(
                    g.entity_associations
                        .iter()
                        .map(Self::convert_entity_association)
                        .collect(),
                );
                let association_origins = self.association_origins(g);
                let match_keys = Self::v1_match_keys(g);
                let provenance = Provenance {
                    source: None,
                    path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
                };

                DepImportedMetric {
                    name: metric_name,
                    instrument,
                    unit,
                    requirement_level,
                    attributes,
                    entity_associations,
                    association_origins,
                    common,
                    provenance,
                    origin,
                    excluded,
                    match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable spans from this dependency.
    pub fn importable_spans(&self) -> Vec<DepImportedSpan> {
        let my_schema_url = self.schema_url();
        self.schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Span)
            .map(|g| {
                let (stability, deprecated) =
                    v1_stability_and_deprecated_to_v2(g.stability.clone(), g.deprecated.clone());
                let common = CommonFields {
                    brief: g.brief.clone(),
                    note: g.note.clone(),
                    stability: stability.unwrap_or_default(),
                    deprecated,
                    annotations: g.annotations.clone().unwrap_or_default(),
                };
                let origin = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                let excluded = g.annotations.as_ref().is_some_and(is_excluded);
                let type_id = v2_namespace_id(g).unwrap_or_else(|| SignalId::from(g.id.as_str()));
                let kind = g
                    .span_kind
                    .clone()
                    .map(v1_span_kind_to_v2)
                    .unwrap_or(SpanKindSpec::Internal);
                let span_name = g
                    .span_name
                    .clone()
                    .map(v1_span_name_to_v2)
                    .unwrap_or_else(|| SpanName {
                        templates: vec![],
                        note: g.name.clone(),
                    });
                let requirement_level = g.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::new();
                for attr_ref in &g.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, &my_schema_url)
                        } else {
                            self.find_attribute_origin(&g.id, &my_schema_url)
                        };
                        let is_v1_def = g
                            .lineage
                            .as_ref()
                            .and_then(|l| l.attribute(&a.name))
                            .is_none();
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((
                            v2_attr,
                            req,
                            a.sampling_relevant,
                            attr_origin,
                            is_v1_def,
                        ));
                    }
                }

                let entity_associations = EntityAssociations::Resolved(
                    g.entity_associations
                        .iter()
                        .map(Self::convert_entity_association)
                        .collect(),
                );
                let association_origins = self.association_origins(g);
                let match_keys = Self::v1_match_keys(g);
                let provenance = Provenance {
                    source: None,
                    path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
                };

                DepImportedSpan {
                    r#type: type_id,
                    kind,
                    name: span_name,
                    requirement_level,
                    attributes,
                    entity_associations,
                    association_origins,
                    common,
                    provenance,
                    origin,
                    excluded,
                    match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable events from this dependency.
    pub fn importable_events(&self) -> Vec<DepImportedEvent> {
        let my_schema_url = self.schema_url();
        self.schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Event)
            .map(|g| {
                let (stability, deprecated) =
                    v1_stability_and_deprecated_to_v2(g.stability.clone(), g.deprecated.clone());
                let common = CommonFields {
                    brief: g.brief.clone(),
                    note: g.note.clone(),
                    stability: stability.unwrap_or_default(),
                    deprecated,
                    annotations: g.annotations.clone().unwrap_or_default(),
                };
                let origin = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                let excluded = g.annotations.as_ref().is_some_and(is_excluded);
                let name = g
                    .name
                    .clone()
                    .map(SignalId::from)
                    .or_else(|| v2_namespace_id(g))
                    .unwrap_or_else(|| SignalId::from(g.id.as_str()));
                let requirement_level = g.requirement_level.clone().map(Into::into);

                let mut attributes = Vec::new();
                for attr_ref in &g.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, &my_schema_url)
                        } else {
                            self.find_attribute_origin(&g.id, &my_schema_url)
                        };
                        let is_v1_def = g
                            .lineage
                            .as_ref()
                            .and_then(|l| l.attribute(&a.name))
                            .is_none();
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, attr_origin, is_v1_def));
                    }
                }

                let entity_associations = EntityAssociations::Resolved(
                    g.entity_associations
                        .iter()
                        .map(Self::convert_entity_association)
                        .collect(),
                );
                let association_origins = self.association_origins(g);
                let match_keys = Self::v1_match_keys(g);
                let provenance = Provenance {
                    source: None,
                    path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
                };

                DepImportedEvent {
                    name,
                    requirement_level,
                    attributes,
                    entity_associations,
                    association_origins,
                    common,
                    provenance,
                    origin,
                    excluded,
                    match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable entities from this dependency.
    pub fn importable_entities(&self) -> Vec<DepImportedEntity> {
        let my_schema_url = self.schema_url();
        self.schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Entity)
            .map(|g| {
                let (stability, deprecated) =
                    v1_stability_and_deprecated_to_v2(g.stability.clone(), g.deprecated.clone());
                let common = CommonFields {
                    brief: g.brief.clone(),
                    note: g.note.clone(),
                    stability: stability.unwrap_or_default(),
                    deprecated,
                    annotations: g.annotations.clone().unwrap_or_default(),
                };
                let origin = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                let excluded = g.annotations.as_ref().is_some_and(is_excluded);
                let type_id = v2_namespace_id(g)
                    .or_else(|| g.name.clone().map(SignalId::from))
                    .unwrap_or_else(|| SignalId::from(g.id.as_str()));
                let requirement_level = g.requirement_level.clone().map(Into::into);

                let mut identity = Vec::new();
                let mut description = Vec::new();

                for attr_ref in &g.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, &my_schema_url)
                        } else {
                            self.find_attribute_origin(&g.id, &my_schema_url)
                        };
                        let is_v1_def = g
                            .lineage
                            .as_ref()
                            .and_then(|l| l.attribute(&a.name))
                            .is_none();
                        if a.role == Some(AttributeRole::Identifying) {
                            identity.push((v2_attr, attr_origin, is_v1_def));
                        } else {
                            let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                            description.push((v2_attr, req, attr_origin, is_v1_def));
                        }
                    }
                }

                let match_keys = Self::v1_match_keys(g);
                let provenance = Provenance {
                    source: None,
                    path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
                };

                DepImportedEntity {
                    r#type: type_id,
                    requirement_level,
                    identity,
                    description,
                    common,
                    provenance,
                    origin,
                    excluded,
                    match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable attribute groups from this dependency.
    pub fn importable_attribute_groups(&self) -> Vec<DepImportedAttributeGroup> {
        let my_schema_url = self.schema_url();
        self.schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::AttributeGroup)
            .map(|g| {
                let (stability, deprecated) =
                    v1_stability_and_deprecated_to_v2(g.stability.clone(), g.deprecated.clone());
                let common = CommonFields {
                    brief: g.brief.clone(),
                    note: g.note.clone(),
                    stability: stability.unwrap_or_default(),
                    deprecated,
                    annotations: g.annotations.clone().unwrap_or_default(),
                };
                let origin = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                let excluded = g.annotations.as_ref().is_some_and(is_excluded);
                let id = SignalId::from(Self::fix_group_id("attribute_group.", &g.id));
                let is_public = g.visibility == Some(AttributeGroupVisibilitySpec::Public);

                let mut attributes = Vec::new();

                for attr_ref in &g.attributes {
                    if let Some(a) = self.schema.catalog.attribute(attr_ref) {
                        let (v2_attr, _) = Self::convert_v1_attribute(a);
                        let attr_origin = if let Some((_, src)) =
                            self.schema.catalog.root_attribute_definition(&a.name)
                        {
                            self.find_attribute_origin(src, &my_schema_url)
                        } else {
                            self.find_attribute_origin(&g.id, &my_schema_url)
                        };
                        let is_v1_def = g
                            .lineage
                            .as_ref()
                            .and_then(|l| l.attribute(&a.name))
                            .is_none();
                        let req = v1_requirement_level_to_v2(a.requirement_level.clone());
                        attributes.push((v2_attr, req, attr_origin, is_v1_def));
                    }
                }

                let match_keys = Self::v1_match_keys(g);
                let provenance = Provenance {
                    source: None,
                    path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
                };

                DepImportedAttributeGroup {
                    id,
                    attributes,
                    common,
                    provenance,
                    origin,
                    excluded,
                    is_public,
                    match_keys,
                }
            })
            .collect()
    }

    /// Strips `prefix` from `id` when present.
    fn fix_group_id<'a>(prefix: &str, id: &'a str) -> &'a str {
        id.strip_prefix(prefix).unwrap_or(id)
    }

    /// Computes candidate wildcard-matching keys for importing a V1 `Group` in V2.
    fn v1_match_keys(g: &Group) -> Vec<String> {
        let name = g.name.as_deref();
        let metric_name = g.metric_name.as_deref();
        if g.is_v2 {
            let (signal_name, prefix) = match g.r#type {
                GroupType::AttributeGroup => (None, "attribute_group."),
                GroupType::Span => (name, "span."),
                GroupType::Event => (name, "event."),
                GroupType::Metric | GroupType::MetricGroup => (metric_name, "metric."),
                GroupType::Entity => (name, "entity."),
                GroupType::Scope | GroupType::Undefined => return vec![],
            };
            let mut keys = vec![g.id.clone()];
            if let Some(n) = signal_name {
                keys.push(n.to_owned());
            }
            if g.r#type == GroupType::AttributeGroup {
                if let Some(stripped) = g.id.strip_prefix("registry.") {
                    keys.push(stripped.to_owned());
                }
            }
            if let Some(stripped) = g.id.strip_prefix(prefix) {
                keys.push(stripped.to_owned());
            }
            keys
        } else {
            match g.r#type {
                GroupType::AttributeGroup | GroupType::Span => {
                    let mut keys = vec![g.id.clone()];
                    if let Some(stripped) = g.id.strip_prefix("span.") {
                        keys.push(stripped.to_owned());
                    }
                    if let Some(stripped) = g.id.strip_prefix("attribute_group.") {
                        keys.push(stripped.to_owned());
                    }
                    keys
                }
                GroupType::Event => name.map(|n| vec![n.to_owned()]).unwrap_or_default(),
                GroupType::Entity => {
                    let mut keys = vec![g.id.clone()];
                    if let Some(stripped) = g.id.strip_prefix("entity.") {
                        keys.push(stripped.to_owned());
                    }
                    if let Some(stripped) = g.id.strip_prefix("resource.") {
                        keys.push(stripped.to_owned());
                    }
                    if let Some(n) = name {
                        if !keys.iter().any(|k| k == n) {
                            keys.push(n.to_owned());
                        }
                    }
                    keys
                }
                GroupType::Metric => metric_name.map(|n| vec![n.to_owned()]).unwrap_or_default(),
                GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => vec![],
            }
        }
    }

    /// Recursively converts a V1 `EntityAssociation` tree into a V2 `EntityAssociation` tree.
    fn convert_entity_association(assoc: &V1EntityAssociation) -> V2EntityAssociation {
        match assoc {
            V1EntityAssociation::Ref(name) => {
                V2EntityAssociation::Ref(V2EntityRef::local(SignalId::from(name.as_str())))
            }
            V1EntityAssociation::OneOf { one_of } => V2EntityAssociation::OneOf {
                one_of: one_of
                    .iter()
                    .map(Self::convert_entity_association)
                    .collect(),
            },
            V1EntityAssociation::AllOf { all_of } => V2EntityAssociation::AllOf {
                all_of: all_of
                    .iter()
                    .map(Self::convert_entity_association)
                    .collect(),
            },
        }
    }

    /// Collects the defining `SchemaUrl` for each entity referenced in `group.entity_associations`.
    fn association_origins(&self, group: &Group) -> BTreeMap<String, SchemaUrl> {
        let recorded = self
            .schema
            .registry
            .entity_association_origins
            .get(&group.id);
        group
            .entity_associations
            .iter()
            .flat_map(|assoc| assoc.referenced_entities())
            .filter_map(|name| {
                let origin = match recorded.and_then(|origins| origins.get(name)) {
                    Some(origin) => Some(origin.clone()),
                    None => self
                        .schema
                        .lookup_entity(name)
                        .filter(|location| !location.excluded)
                        .map(|location| location.origin),
                };
                origin.map(|origin| (name.to_owned(), origin))
            })
            .collect()
    }
}

impl EntityLookup for V1Dependency {
    /// Looks up an entity's origin and exclusion status in the underlying V1 schema.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        self.schema.lookup_entity(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_v1_schema() -> V1Schema {
        let attr = V1Attribute {
            name: "a.test".to_owned(),
            r#type: weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "test attr".to_owned(),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: String::new(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: None,
        };
        let group = Group {
            id: "a".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: String::new(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![weaver_resolved_schema::v1::attribute::AttributeRef(0)],
            span_kind: None,
            span_name: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            requirement_level: None,
            name: None,
            lineage: None,
            display_name: None,
            body: None,
            annotations: None,
            entity_associations: vec![],
            visibility: None,
            is_v2: false,
        };
        V1Schema {
            file_format: "resolved/1.0.0".to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "test".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "v1-test".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![group],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::new(vec![attr], HashMap::new()),
            resource: None,
            instrumentation_library: None,
            dependencies: BTreeSet::new(),
            versions: None,
            registry_manifest: None,
        }
    }

    #[test]
    fn test_v1_dependency_lookup_attribute() {
        let v1_schema = sample_v1_schema();
        let dep = V1Dependency::new(v1_schema);
        let looked_up = dep.lookup_attribute("a.test").unwrap().expect("found");
        assert_eq!(looked_up.attribute.key, "a.test");
        assert!(!looked_up.excluded);
        assert!(dep.lookup_attribute("nonexistent").unwrap().is_none());
    }
}
