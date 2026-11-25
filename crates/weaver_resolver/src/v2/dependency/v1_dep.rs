// SPDX-License-Identifier: Apache-2.0

//! Adapter for reading resolved V1 dependency schemas in V2 resolution; the only module in `v2/` referencing V1 types.

#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
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
use weaver_semconv::v2::attribute::RequirementLevel;
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
use crate::v2::signals::{EntityAssociations, SignalKind};
use crate::WeaverResolvedSchema;

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

/// Converted V1 group metadata shared across `lookup_signal` and `importable_*` methods.
struct GroupMeta {
    common: CommonFields,
    origin: SchemaUrl,
    excluded: bool,
    provenance: Provenance,
    match_keys: Vec<String>,
}

/// Converted attribute entry extracted from a V1 group's catalog references.
struct ConvertedGroupAttr {
    attr: V2Attribute,
    req: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
    origin: SchemaUrl,
    is_v1_def: bool,
}

/// Adapter over a resolved V1 schema dependency.
#[derive(Debug, Clone)]
pub(crate) struct V1Dependency {
    bundle: Arc<WeaverResolvedSchema>,
    my_schema_url: Option<SchemaUrl>,
    group_origins: HashMap<String, SchemaUrl>,
    dep_name_urls: HashMap<String, SchemaUrl>,
}

impl V1Dependency {
    /// Creates a new adapter over a resolved V1 schema.
    #[cfg(test)]
    #[must_use]
    pub fn new(schema: V1Schema) -> Self {
        Self::from_bundle(Arc::new(WeaverResolvedSchema::V1(schema)))
    }

    /// Creates a new adapter wrapping a shared `Arc<WeaverResolvedSchema>` without cloning.
    #[must_use]
    pub fn from_bundle(bundle: Arc<WeaverResolvedSchema>) -> Self {
        #[cfg(test)]
        V1_ADAPTER_DISALLOWED.with(|c| {
            if c.get() {
                panic!("V1Dependency was invoked while DisallowV1AdapterScope was active!");
            }
        });
        let schema = bundle
            .as_v1()
            .expect("V1Dependency::from_bundle requires a V1 schema");
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
            bundle,
            my_schema_url,
            group_origins,
            dep_name_urls,
        }
    }

    /// Returns a reference to the underlying `V1Schema`.
    fn schema(&self) -> &V1Schema {
        self.bundle
            .as_v1()
            .expect("V1Dependency bundle must hold a V1 schema")
    }

    /// Returns the parsed schema URL of this dependency, falling back to `SchemaUrl::new_unknown`.
    #[must_use]
    fn schema_url(&self) -> SchemaUrl {
        self.my_schema_url
            .clone()
            .unwrap_or_else(SchemaUrl::new_unknown)
    }

    /// Extracts common V2 fields, origin, exclusion flag, provenance, and wildcard keys from `g`.
    fn group_meta(&self, g: &Group, default_url: &SchemaUrl) -> GroupMeta {
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
            .unwrap_or_else(|| default_url.clone());
        let excluded = g.annotations.as_ref().is_some_and(is_excluded);
        let provenance = Provenance {
            source: None,
            path: g.provenance().map(|p| p.path.clone()).unwrap_or_default(),
        };
        GroupMeta {
            common,
            origin,
            excluded,
            provenance,
            match_keys: Self::v1_match_keys(g),
        }
    }

    /// Converts a V1 group's `entity_associations` and collects their resolved origin URLs.
    fn group_associations(&self, g: &Group) -> (EntityAssociations, BTreeMap<String, SchemaUrl>) {
        let assocs = EntityAssociations::Resolved(
            g.entity_associations
                .iter()
                .map(Self::convert_entity_association)
                .collect(),
        );
        (assocs, self.association_origins(g))
    }

    /// Extracts converted metric header fields `(name, instrument, unit)` from a V1 `Group`.
    fn metric_header(g: &Group, fallback_id: Option<&str>) -> (SignalId, InstrumentSpec, String) {
        let name = g
            .metric_name
            .clone()
            .map(SignalId::from)
            .or_else(|| fallback_id.map(SignalId::from))
            .or_else(|| v2_namespace_id(g))
            .unwrap_or_else(|| SignalId::from(g.id.as_str()));
        let instrument = g
            .instrument
            .clone()
            .map(v1_instrument_to_v2)
            .unwrap_or(InstrumentSpec::UpDownCounter);
        let unit = g.unit.clone().unwrap_or_default();
        (name, instrument, unit)
    }

    /// Extracts converted span header fields `(type_id, kind, name)` from a V1 `Group`.
    fn span_header(g: &Group, fallback_id: Option<&str>) -> (SignalId, SpanKindSpec, SpanName) {
        let type_id = fallback_id
            .map(SignalId::from)
            .or_else(|| v2_namespace_id(g))
            .unwrap_or_else(|| SignalId::from(g.id.as_str()));
        let kind = g
            .span_kind
            .clone()
            .map(v1_span_kind_to_v2)
            .unwrap_or(SpanKindSpec::Internal);
        let name = g
            .span_name
            .clone()
            .map(v1_span_name_to_v2)
            .unwrap_or_else(|| SpanName {
                templates: vec![],
                note: g.name.clone().or_else(|| Some(type_id.to_string())),
            });
        (type_id, kind, name)
    }

    /// Extracts the V2 event name from a V1 `Group`.
    fn event_name(g: &Group, fallback_id: Option<&str>) -> SignalId {
        g.name
            .clone()
            .map(SignalId::from)
            .or_else(|| fallback_id.map(SignalId::from))
            .or_else(|| v2_namespace_id(g))
            .unwrap_or_else(|| SignalId::from(g.id.as_str()))
    }

    /// Extracts the V2 entity type ID from a V1 `Group`.
    fn entity_type(g: &Group, fallback_id: Option<&str>) -> SignalId {
        if let Some(id) = fallback_id {
            g.name
                .clone()
                .map(SignalId::from)
                .unwrap_or_else(|| SignalId::from(id))
        } else {
            v2_namespace_id(g)
                .or_else(|| g.name.clone().map(SignalId::from))
                .unwrap_or_else(|| SignalId::from(g.id.as_str()))
        }
    }

    /// Converts all catalog attributes referenced by `g` with their origin URLs and V1 definition flags.
    fn convert_group_attrs(&self, g: &Group, default_url: &SchemaUrl) -> Vec<ConvertedGroupAttr> {
        let schema = self.schema();
        g.attributes
            .iter()
            .filter_map(|attr_ref| {
                let a = schema.catalog.attribute(attr_ref)?;
                let (attr, _) = Self::convert_v1_attribute(a);
                let origin =
                    if let Some((_, src)) = schema.catalog.root_attribute_definition(&a.name) {
                        self.find_attribute_origin(src, default_url)
                    } else {
                        self.find_attribute_origin(&g.id, default_url)
                    };
                let is_v1_def = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.attribute(&a.name))
                    .is_none();
                Some(ConvertedGroupAttr {
                    attr,
                    req: v1_requirement_level_to_v2(a.requirement_level.clone()),
                    sampling_relevant: a.sampling_relevant,
                    role: a.role.clone(),
                    origin,
                    is_v1_def,
                })
            })
            .collect()
    }

    /// Looks up an attribute definition by key.
    pub fn lookup_attribute(&self, key: &str) -> Result<Option<DepAttribute>, Error> {
        let schema = self.schema();
        let Some(my_schema_url) = self.my_schema_url.as_ref() else {
            return Err(Error::InvalidUrl {
                url: schema.schema_url.clone(),
                error: SchemaUrl::try_from(schema.schema_url.as_str())
                    .expect_err("my_schema_url is None only when schema_url is invalid"),
            });
        };

        if let Some((attr, source_group)) = schema.catalog.root_attribute_definition(key) {
            let origin = self.find_attribute_origin(source_group, my_schema_url);
            let (v2_attr, excluded) = Self::convert_v1_attribute(attr);
            return Ok(Some(DepAttribute {
                attribute: v2_attr,
                origin,
                excluded,
            }));
        }

        // Fallback for deserialized schemas without root attributes.
        for group in &schema.registry.groups {
            for attr_ref in &group.attributes {
                if let Some(a) = schema.catalog.attribute(attr_ref) {
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
    pub fn lookup_signal(&self, kind: SignalKind, id: &str) -> Option<DepSignal> {
        let my_schema_url = self.my_schema_url.as_ref()?;
        let expected_group_type = match kind {
            SignalKind::Span => GroupType::Span,
            SignalKind::Metric => GroupType::Metric,
            SignalKind::Event => GroupType::Event,
            SignalKind::Entity => GroupType::Entity,
        };

        let group = self.schema().registry.groups.iter().find(|g| {
            g.r#type == expected_group_type
                && v2_namespace_id(g).is_some_and(|sig_id| &*sig_id == id)
        })?;

        let meta = self.group_meta(group, my_schema_url);
        let attrs = self.convert_group_attrs(group, my_schema_url);
        let requirement_level = group.requirement_level.clone().map(Into::into);

        let payload = match kind {
            SignalKind::Metric => {
                let (name, instrument, unit) = Self::metric_header(group, Some(id));
                DepSignalPayload::Metric {
                    name,
                    instrument,
                    unit,
                    requirement_level,
                    attributes: attrs
                        .into_iter()
                        .map(|a| (a.attr, a.req, a.origin))
                        .collect(),
                }
            }
            SignalKind::Span => {
                let (r#type, span_kind, name) = Self::span_header(group, Some(id));
                DepSignalPayload::Span {
                    r#type,
                    kind: span_kind,
                    name,
                    requirement_level,
                    attributes: attrs
                        .into_iter()
                        .map(|a| (a.attr, a.req, a.sampling_relevant, a.origin))
                        .collect(),
                    // V1 resolved schemas do not carry span links.
                    links: Vec::new(),
                }
            }
            SignalKind::Event => DepSignalPayload::Event {
                name: Self::event_name(group, Some(id)),
                requirement_level,
                attributes: attrs
                    .into_iter()
                    .map(|a| (a.attr, a.req, a.origin))
                    .collect(),
            },
            SignalKind::Entity => {
                let mut identity = Vec::new();
                let mut description = Vec::new();
                for a in attrs {
                    if a.role == Some(AttributeRole::Identifying) {
                        identity.push((a.attr, a.origin));
                    } else {
                        description.push((a.attr, a.req, a.origin));
                    }
                }
                DepSignalPayload::Entity {
                    r#type: Self::entity_type(group, Some(id)),
                    requirement_level,
                    identity,
                    description,
                }
            }
        };

        let (entity_associations, association_origins) = match kind {
            SignalKind::Metric | SignalKind::Span | SignalKind::Event => {
                self.group_associations(group)
            }
            SignalKind::Entity => (EntityAssociations::Raw(vec![]), BTreeMap::new()),
        };

        Some(DepSignal {
            payload,
            common: meta.common,
            origin: meta.origin,
            excluded: meta.excluded,
            entity_associations,
            association_origins,
        })
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

    /// Returns `true` if `g` is a V1 refinement of another group of the same `GroupType`.
    fn is_refinement(g: &Group) -> bool {
        g.lineage
            .as_ref()
            .is_some_and(|l| l.extends_group_type.as_ref() == Some(&g.r#type))
    }

    /// Returns all importable metrics from this dependency.
    pub fn importable_metrics(&self) -> Vec<DepImportedMetric> {
        let my_schema_url = self.schema_url();
        self.schema()
            .registry
            .groups
            .iter()
            .filter(|g| {
                (g.r#type == GroupType::Metric || g.r#type == GroupType::MetricGroup)
                    && !Self::is_refinement(g)
            })
            .map(|g| {
                let meta = self.group_meta(g, &my_schema_url);
                let (entity_associations, association_origins) = self.group_associations(g);
                let attributes = self
                    .convert_group_attrs(g, &my_schema_url)
                    .into_iter()
                    .map(|a| (a.attr, a.req, a.origin, a.is_v1_def))
                    .collect();
                let (name, instrument, unit) = Self::metric_header(g, None);
                DepImportedMetric {
                    name,
                    instrument,
                    unit,
                    requirement_level: g.requirement_level.clone().map(Into::into),
                    attributes,
                    entity_associations,
                    association_origins,
                    common: meta.common,
                    provenance: meta.provenance,
                    origin: meta.origin,
                    excluded: meta.excluded,
                    match_keys: meta.match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable spans from this dependency.
    pub fn importable_spans(&self) -> Vec<DepImportedSpan> {
        let my_schema_url = self.schema_url();
        self.schema()
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Span && !Self::is_refinement(g))
            .map(|g| {
                let meta = self.group_meta(g, &my_schema_url);
                let (entity_associations, association_origins) = self.group_associations(g);
                let attributes = self
                    .convert_group_attrs(g, &my_schema_url)
                    .into_iter()
                    .map(|a| (a.attr, a.req, a.sampling_relevant, a.origin, a.is_v1_def))
                    .collect();
                let (r#type, kind, name) = Self::span_header(g, None);
                DepImportedSpan {
                    r#type,
                    kind,
                    name,
                    requirement_level: g.requirement_level.clone().map(Into::into),
                    attributes,
                    links: Vec::new(),
                    entity_associations,
                    association_origins,
                    common: meta.common,
                    provenance: meta.provenance,
                    origin: meta.origin,
                    excluded: meta.excluded,
                    match_keys: meta.match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable events from this dependency.
    pub fn importable_events(&self) -> Vec<DepImportedEvent> {
        let my_schema_url = self.schema_url();
        self.schema()
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Event && !Self::is_refinement(g))
            .map(|g| {
                let meta = self.group_meta(g, &my_schema_url);
                let (entity_associations, association_origins) = self.group_associations(g);
                let attributes = self
                    .convert_group_attrs(g, &my_schema_url)
                    .into_iter()
                    .map(|a| (a.attr, a.req, a.origin, a.is_v1_def))
                    .collect();
                DepImportedEvent {
                    name: Self::event_name(g, None),
                    requirement_level: g.requirement_level.clone().map(Into::into),
                    attributes,
                    entity_associations,
                    association_origins,
                    common: meta.common,
                    provenance: meta.provenance,
                    origin: meta.origin,
                    excluded: meta.excluded,
                    match_keys: meta.match_keys,
                }
            })
            .collect()
    }

    /// Returns all importable entities from this dependency.
    pub fn importable_entities(&self) -> Vec<DepImportedEntity> {
        let my_schema_url = self.schema_url();
        self.schema()
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Entity && !Self::is_refinement(g))
            .map(|g| {
                let meta = self.group_meta(g, &my_schema_url);
                let mut identity = Vec::new();
                let mut description = Vec::new();
                for a in self.convert_group_attrs(g, &my_schema_url) {
                    if a.role == Some(AttributeRole::Identifying) {
                        identity.push((a.attr, a.origin, a.is_v1_def));
                    } else {
                        description.push((a.attr, a.req, a.origin, a.is_v1_def));
                    }
                }
                DepImportedEntity {
                    r#type: Self::entity_type(g, None),
                    requirement_level: g.requirement_level.clone().map(Into::into),
                    identity,
                    description,
                    common: meta.common,
                    provenance: meta.provenance,
                    origin: meta.origin,
                    excluded: meta.excluded,
                    match_keys: meta.match_keys,
                }
            })
            .collect()
    }

    /// Looks up a public attribute group by ID (or V1 match key).
    pub fn lookup_attribute_group(&self, id: &str) -> Option<DepImportedAttributeGroup> {
        let stripped = Self::fix_group_id("attribute_group.", id);
        self.importable_attribute_groups().into_iter().find(|ag| {
            &*ag.id == id || &*ag.id == stripped || ag.match_keys.iter().any(|k| k == id)
        })
    }

    /// Returns all importable attribute groups from this dependency.
    pub fn importable_attribute_groups(&self) -> Vec<DepImportedAttributeGroup> {
        let my_schema_url = self.schema_url();
        self.schema()
            .registry
            .groups
            .iter()
            .filter(|g| {
                g.r#type == GroupType::AttributeGroup
                    && g.visibility == Some(AttributeGroupVisibilitySpec::Public)
            })
            .map(|g| {
                let meta = self.group_meta(g, &my_schema_url);
                let attributes = self
                    .convert_group_attrs(g, &my_schema_url)
                    .into_iter()
                    .map(|a| (a.attr, a.req, a.origin, a.is_v1_def))
                    .collect();
                DepImportedAttributeGroup {
                    id: SignalId::from(Self::fix_group_id("attribute_group.", &g.id)),
                    attributes,
                    common: meta.common,
                    provenance: meta.provenance,
                    origin: meta.origin,
                    excluded: meta.excluded,
                    is_public: g.visibility == Some(AttributeGroupVisibilitySpec::Public),
                    match_keys: meta.match_keys,
                }
            })
            .collect()
    }

    /// Strips `prefix` from `id` when present.
    fn fix_group_id<'a>(prefix: &str, id: &'a str) -> &'a str {
        id.strip_prefix(prefix).unwrap_or(id)
    }

    /// Computes candidate wildcard-matching keys for importing a V1 `Group` in V2.
    ///
    /// Intentionally strips legacy `span.`, `attribute_group.`, and `resource.` prefixes on
    /// V1 groups so V2 consumers can write unprefixed `imports:` patterns against V1 dependencies.
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
        let schema = self.schema();
        let recorded = schema.registry.entity_association_origins.get(&group.id);
        group
            .entity_associations
            .iter()
            .flat_map(|assoc| assoc.referenced_entities())
            .filter_map(|name| {
                let origin = match recorded.and_then(|origins| origins.get(name)) {
                    Some(origin) => Some(origin.clone()),
                    None => schema
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
        self.schema().lookup_entity(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};

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
            span_links: Vec::new(),
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

    #[test]
    fn test_v1_dependency_excludes_refinements_from_importable_metrics() {
        let mut v1_schema = sample_v1_schema();
        let base_metric = Group {
            id: "metric.hw.errors".to_owned(),
            r#type: GroupType::Metric,
            brief: "base metric".to_owned(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            span_name: None,
            span_links: Vec::new(),
            events: vec![],
            metric_name: Some("hw.errors".to_owned()),
            instrument: Some(weaver_semconv::v1::group::InstrumentSpec::Counter),
            unit: Some("{error}".to_owned()),
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
        let mut refinement_metric = base_metric.clone();
        refinement_metric.id = "metric.hw.cpu.errors".to_owned();
        refinement_metric.brief = "cpu refinement".to_owned();
        let mut lineage = weaver_resolved_schema::v1::lineage::GroupLineage::new(
            weaver_semconv::provenance::Provenance::new(
                SchemaUrl::try_from("http://test/schemas/1.0.0").unwrap(),
                "test.yaml",
            ),
        );
        lineage.extends_group_type = Some(GroupType::Metric);
        refinement_metric.lineage = Some(lineage);

        v1_schema.registry.groups.push(refinement_metric);
        v1_schema.registry.groups.push(base_metric);

        let dep = V1Dependency::new(v1_schema);
        let imported = dep.importable_metrics();
        assert_eq!(imported.len(), 1);
        assert_eq!(&*imported[0].name, "hw.errors");
        assert_eq!(imported[0].common.brief, "base metric");
    }

    #[test]
    fn test_v1_dependency_excludes_non_public_attribute_groups() {
        let mut v1_schema = sample_v1_schema();
        let mut public_group = v1_schema.registry.groups[0].clone();
        public_group.id = "attribute_group.public.attrs".to_owned();
        public_group.visibility = Some(AttributeGroupVisibilitySpec::Public);
        v1_schema.registry.groups.push(public_group);

        let dep = V1Dependency::new(v1_schema);
        let imported = dep.importable_attribute_groups();
        assert_eq!(imported.len(), 1);
        assert_eq!(&*imported[0].id, "public.attrs");
    }

    #[test]
    fn test_v1_dependency_span_without_span_name_uses_fallback_note() {
        let mut v1_schema = sample_v1_schema();
        let span_group = Group {
            id: "span.http.client".to_owned(),
            r#type: GroupType::Span,
            brief: "http client span".to_owned(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![],
            span_kind: Some(weaver_semconv::v1::group::SpanKindSpec::Client),
            span_name: None,
            span_links: Vec::new(),
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
        v1_schema.registry.groups.push(span_group);

        let dep = V1Dependency::new(v1_schema);
        let imported = dep.importable_spans();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name.note.as_deref(), Some("http.client"));

        let looked_up = dep
            .lookup_signal(SignalKind::Span, "http.client")
            .expect("found span");
        match looked_up.payload {
            DepSignalPayload::Span { name, .. } => {
                assert_eq!(name.note.as_deref(), Some("http.client"));
            }
            other => panic!("Expected Span payload, got: {other:?}"),
        }
    }
}
