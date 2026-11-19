// SPDX-License-Identifier: Apache-2.0

//! Traits and adapters for reading dependencies in the V2 resolution pass.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::{DependencyRef, Provenance as SchemaProvenance};
use weaver_semconv::provenance::Provenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::RequirementLevel;

use super::signals::EntityAssociations;
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::span::{SpanKindSpec, SpanName};
use weaver_semconv::v2::CommonFields;

use crate::error::Error;
use crate::graph::chosen_versions::{
    chosen_version_if_superseded, lookup_chosen_schema, SchemaCacheLookup,
};
use crate::graph::conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion};
pub(crate) use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::WeaverResolvedSchema;

pub(crate) mod v1_dep;
pub(crate) mod v2_dep;

#[cfg(test)]
pub(crate) use v1_dep::DisallowV1AdapterScope;
pub(crate) use v1_dep::V1Dependency;
pub(crate) use v2_dep::V2Dependency;

/// Attribute definition resolved from a dependency schema with its origin and exclusion status.
#[derive(Debug, Clone)]
pub(crate) struct DepAttribute {
    pub attribute: Attribute,
    pub origin: SchemaUrl,
    pub excluded: bool,
}

/// Metric definition imported from a dependency schema.
#[derive(Debug, Clone)]
pub(crate) struct DepImportedMetric {
    pub name: SignalId,
    pub instrument: InstrumentSpec,
    pub unit: String,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub attributes: Vec<(Attribute, RequirementLevel, SchemaUrl, bool)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub match_keys: Vec<String>,
}

/// Span definition imported from a dependency schema.
#[derive(Debug, Clone)]
pub(crate) struct DepImportedSpan {
    pub r#type: SignalId,
    pub kind: SpanKindSpec,
    pub name: SpanName,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub attributes: Vec<(Attribute, RequirementLevel, Option<bool>, SchemaUrl, bool)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub match_keys: Vec<String>,
}

/// Event definition imported from a dependency schema.
#[derive(Debug, Clone)]
pub(crate) struct DepImportedEvent {
    pub name: SignalId,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub attributes: Vec<(Attribute, RequirementLevel, SchemaUrl, bool)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub match_keys: Vec<String>,
}

/// Entity definition imported from a dependency schema.
#[derive(Debug, Clone)]
pub(crate) struct DepImportedEntity {
    pub r#type: SignalId,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub identity: Vec<(Attribute, SchemaUrl, bool)>,
    pub description: Vec<(Attribute, RequirementLevel, SchemaUrl, bool)>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub match_keys: Vec<String>,
}

/// Attribute group imported from a dependency schema.
#[derive(Debug, Clone)]
pub(crate) struct DepImportedAttributeGroup {
    pub id: SignalId,
    pub attributes: Vec<(Attribute, RequirementLevel, SchemaUrl, bool)>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub is_public: bool,
    pub match_keys: Vec<String>,
}

/// Kind-specific fields of a signal looked up as a refinement parent in a dependency.
#[derive(Debug, Clone)]
pub(crate) enum DepSignalPayload {
    Metric {
        name: SignalId,
        instrument: InstrumentSpec,
        unit: String,
        requirement_level: Option<SignalRequirementLevel>,
        attributes: Vec<(Attribute, RequirementLevel, SchemaUrl)>,
    },
    Span {
        r#type: SignalId,
        kind: SpanKindSpec,
        name: SpanName,
        requirement_level: Option<SignalRequirementLevel>,
        attributes: Vec<(Attribute, RequirementLevel, Option<bool>, SchemaUrl)>,
    },
    Event {
        name: SignalId,
        requirement_level: Option<SignalRequirementLevel>,
        attributes: Vec<(Attribute, RequirementLevel, SchemaUrl)>,
    },
    Entity {
        r#type: SignalId,
        requirement_level: Option<SignalRequirementLevel>,
        identity: Vec<(Attribute, SchemaUrl)>,
        description: Vec<(Attribute, RequirementLevel, SchemaUrl)>,
    },
}

/// Signal definition looked up from a dependency schema for refinement inheritance.
#[derive(Debug, Clone)]
pub(crate) struct DepSignal {
    pub payload: DepSignalPayload,
    pub common: CommonFields,
    pub origin: SchemaUrl,
    pub excluded: bool,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
}

impl DepSignal {
    /// Returns the lowercase signal kind identifier for this payload.
    #[must_use]
    #[cfg(test)]
    pub fn kind(&self) -> &'static str {
        match &self.payload {
            DepSignalPayload::Metric { .. } => "metric",
            DepSignalPayload::Span { .. } => "span",
            DepSignalPayload::Event { .. } => "event",
            DepSignalPayload::Entity { .. } => "entity",
        }
    }
}

/// Version-abstracted adapter over a resolved dependency schema.
#[derive(Debug, Clone)]
pub(crate) enum DependencyAdapter {
    V2(V2Dependency),
    V1(Box<V1Dependency>),
}

impl DependencyAdapter {
    /// Creates an adapter from an `Arc<WeaverResolvedSchema>`.
    #[must_use]
    pub fn from_resolved_schema(schema: &Arc<WeaverResolvedSchema>) -> Self {
        match schema.as_ref() {
            WeaverResolvedSchema::V2(_) => {
                DependencyAdapter::V2(V2Dependency::from_bundle(Arc::clone(schema)))
            }
            WeaverResolvedSchema::V1(_) => {
                DependencyAdapter::V1(Box::new(V1Dependency::from_bundle(Arc::clone(schema))))
            }
        }
    }

    /// Looks up an attribute definition by key.
    pub fn lookup_attribute(&self, key: &str) -> Result<Option<DepAttribute>, Error> {
        match self {
            DependencyAdapter::V2(d) => d.lookup_attribute(key),
            DependencyAdapter::V1(d) => d.lookup_attribute(key),
        }
    }

    /// Looks up a signal by kind and id.
    pub fn lookup_signal(&self, kind: &str, id: &str) -> Option<DepSignal> {
        match self {
            DependencyAdapter::V2(d) => d.lookup_signal(kind, id),
            DependencyAdapter::V1(d) => d.lookup_signal(kind, id),
        }
    }

    /// Looks up a signal by id across all kinds, returning its kind if found.
    pub fn lookup_signal_any_kind(&self, id: &str) -> Option<(&'static str, DepSignal)> {
        match self {
            DependencyAdapter::V2(d) => d.lookup_signal_any_kind(id),
            DependencyAdapter::V1(d) => d.lookup_signal_any_kind(id),
        }
    }

    /// Returns all importable metrics from this dependency.
    pub fn importable_metrics(&self) -> Vec<DepImportedMetric> {
        match self {
            DependencyAdapter::V2(d) => d.importable_metrics(),
            DependencyAdapter::V1(d) => d.importable_metrics(),
        }
    }

    /// Returns all importable spans from this dependency.
    pub fn importable_spans(&self) -> Vec<DepImportedSpan> {
        match self {
            DependencyAdapter::V2(d) => d.importable_spans(),
            DependencyAdapter::V1(d) => d.importable_spans(),
        }
    }

    /// Returns all importable events from this dependency.
    pub fn importable_events(&self) -> Vec<DepImportedEvent> {
        match self {
            DependencyAdapter::V2(d) => d.importable_events(),
            DependencyAdapter::V1(d) => d.importable_events(),
        }
    }

    /// Returns all importable entities from this dependency.
    pub fn importable_entities(&self) -> Vec<DepImportedEntity> {
        match self {
            DependencyAdapter::V2(d) => d.importable_entities(),
            DependencyAdapter::V1(d) => d.importable_entities(),
        }
    }

    /// Returns all importable attribute groups from this dependency.
    pub fn importable_attribute_groups(&self) -> Vec<DepImportedAttributeGroup> {
        match self {
            DependencyAdapter::V2(d) => d.importable_attribute_groups(),
            DependencyAdapter::V1(d) => d.importable_attribute_groups(),
        }
    }
}

impl EntityLookup for DependencyAdapter {
    /// Delegates entity lookup to the underlying V1 or V2 dependency adapter.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        match self {
            DependencyAdapter::V2(d) => d.lookup_entity(name),
            DependencyAdapter::V1(d) => d.lookup_entity(name),
        }
    }
}

/// Collection of dependencies available during V2 schema resolution.
#[derive(Debug, Clone)]
pub(crate) struct V2Dependencies {
    deps: Vec<DependencyAdapter>,
    urls: BTreeSet<SchemaUrl>,
    url_indices: HashMap<SchemaUrl, DependencyRef>,
    adapter_cache: RefCell<HashMap<SchemaUrl, Arc<DependencyAdapter>>>,
}

impl V2Dependencies {
    /// Creates a new dependency collection.
    #[must_use]
    pub fn new(deps: Vec<DependencyAdapter>, urls: BTreeSet<SchemaUrl>) -> Self {
        let url_indices = urls
            .iter()
            .enumerate()
            .map(|(idx, url)| (url.clone(), DependencyRef(idx as u32)))
            .collect();
        Self {
            deps,
            urls,
            url_indices,
            adapter_cache: RefCell::new(HashMap::new()),
        }
    }

    /// The linearized set of chosen dependency URLs.
    #[must_use]
    pub fn urls(&self) -> &BTreeSet<SchemaUrl> {
        &self.urls
    }

    /// Returns the `DependencyRef` index for a linearized dependency URL if present.
    #[must_use]
    pub fn dependency_ref(&self, url: &SchemaUrl) -> Option<DependencyRef> {
        self.url_indices.get(url).copied()
    }

    /// The list of dependency adapters.
    #[must_use]
    pub fn adapters(&self) -> &[DependencyAdapter] {
        &self.deps
    }

    /// Returns a cached adapter for `url`, constructing and indexing it at most once.
    pub fn adapter_for(
        &self,
        url: &SchemaUrl,
        schema: &Arc<WeaverResolvedSchema>,
    ) -> Arc<DependencyAdapter> {
        if let Some(cached) = self.adapter_cache.borrow().get(url) {
            return Arc::clone(cached);
        }
        let adapter = Arc::new(DependencyAdapter::from_resolved_schema(schema));
        let _ = self
            .adapter_cache
            .borrow_mut()
            .insert(url.clone(), Arc::clone(&adapter));
        adapter
    }

    /// Looks up a non-excluded attribute definition across dependencies, resolving conflicts and version upgrades.
    pub fn lookup_attribute<C: SchemaCacheLookup>(
        &self,
        key: &str,
        cache_lookup: &C,
    ) -> Result<Option<DepAttribute>, Error> {
        let mut matches = Vec::new();
        for d in &self.deps {
            if let Some(attr) = d.lookup_attribute(key)? {
                if !attr.excluded {
                    matches.push(attr);
                }
            }
        }

        let winning =
            matches
                .into_iter()
                .try_fold(None, |acc: Option<DepAttribute>, m| match acc {
                    None => Ok::<Option<DepAttribute>, Error>(Some(m)),
                    Some(existing) => {
                        let winning_url = UseLatestMajorVersion
                            .resolve_conflict(&m.origin, &existing.origin)
                            .map_err(|e| match e {
                                Error::IncompatibleDependencyVersions { .. } => {
                                    Error::AmbiguousReference {
                                        r#ref: key.to_owned(),
                                        schema_url1: m.origin.to_string(),
                                        schema_url2: existing.origin.to_string(),
                                    }
                                }
                                other => other,
                            })?;
                        if winning_url == existing.origin {
                            Ok(Some(existing))
                        } else {
                            Ok(Some(m))
                        }
                    }
                })?;

        let Some(mut winning) = winning else {
            return Ok(None);
        };

        // Fetch the base definition from its origin when re-exported transitively.
        if winning.attribute.provenance.source.is_some() {
            if let Some(origin_schema) = cache_lookup.lookup_schema(&winning.origin) {
                let origin_adapter = self.adapter_for(&winning.origin, &origin_schema);
                if let Some(base_attr) = origin_adapter.lookup_attribute(key)? {
                    winning = base_attr;
                }
            }
        }

        if let Some(chosen_url) = chosen_version_if_superseded(&winning.origin, cache_lookup)? {
            let chosen_schema = lookup_chosen_schema(&chosen_url, cache_lookup)?;
            let chosen_adapter = self.adapter_for(&chosen_url, &chosen_schema);
            if let Some(upgraded) = chosen_adapter.lookup_attribute(key)? {
                if upgraded.excluded {
                    return Err(Error::AttributeNotFoundInUpgradedSchema {
                        attribute_name: key.to_owned(),
                        original_url: winning.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                winning = upgraded;
                winning.origin = chosen_url;
            } else {
                return Err(Error::AttributeNotFoundInUpgradedSchema {
                    attribute_name: key.to_owned(),
                    original_url: winning.origin.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            }
        }

        Ok(Some(winning))
    }

    /// Looks up a non-excluded signal across dependencies, resolving conflicts and version upgrades.
    pub fn lookup_signal<C: SchemaCacheLookup>(
        &self,
        kind: &str,
        id: &str,
        refinement_id: &str,
        provenance: Option<&Provenance>,
        cache_lookup: &C,
    ) -> Result<DepSignal, Error> {
        let mut all_matches = Vec::new();
        for d in &self.deps {
            if let Some(signal) = d.lookup_signal(kind, id) {
                if !signal.excluded {
                    all_matches.push(signal);
                }
            }
        }

        if all_matches.is_empty() {
            for d in &self.deps {
                if let Some((other_kind, sig)) = d.lookup_signal_any_kind(id) {
                    if !sig.excluded {
                        return Err(Error::InvalidRefinement {
                            refinement_id: refinement_id.to_owned(),
                            r#ref: id.to_owned(),
                            refinement_type: capitalize_kind(kind),
                            signal_type: capitalize_kind(other_kind),
                        });
                    }
                }
            }
            return Err(Error::UnresolvedRefinementParent {
                refinement_id: refinement_id.to_owned(),
                r#ref: id.to_owned(),
                provenance: provenance.cloned().map(Box::new),
            });
        }

        let winning =
            all_matches
                .into_iter()
                .try_fold(None, |acc: Option<DepSignal>, m| match acc {
                    None => Ok::<Option<DepSignal>, Error>(Some(m)),
                    Some(existing) => {
                        let winning_url = UseLatestMajorVersion
                            .resolve_conflict(&m.origin, &existing.origin)
                            .map_err(|e| match e {
                                Error::IncompatibleDependencyVersions { .. } => {
                                    Error::AmbiguousReference {
                                        r#ref: id.to_owned(),
                                        schema_url1: m.origin.to_string(),
                                        schema_url2: existing.origin.to_string(),
                                    }
                                }
                                other => other,
                            })?;
                        if winning_url == existing.origin {
                            Ok(Some(existing))
                        } else {
                            Ok(Some(m))
                        }
                    }
                })?;

        let mut winning = winning.expect("all_matches is not empty");

        if let Some(chosen_url) = chosen_version_if_superseded(&winning.origin, cache_lookup)? {
            let chosen_schema = lookup_chosen_schema(&chosen_url, cache_lookup)?;
            let chosen_adapter = self.adapter_for(&chosen_url, &chosen_schema);
            match chosen_adapter.lookup_signal(kind, id) {
                Some(upgraded_signal) => {
                    if upgraded_signal.excluded {
                        return Err(Error::GroupNotFoundInUpgradedSchema {
                            group_id: id.to_owned(),
                            original_url: winning.origin.to_string(),
                            upgraded_url: chosen_url.to_string(),
                        });
                    }
                    winning = upgraded_signal;
                    winning.origin = chosen_url;
                }
                None => {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: id.to_owned(),
                        original_url: winning.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
            }
        }

        Ok(winning)
    }

    /// Looks up a signal by id across all kinds in any dependency.
    #[allow(dead_code)]
    pub fn lookup_signal_any_kind(&self, id: &str) -> Option<(&'static str, DepSignal)> {
        for d in &self.deps {
            if let Some((other_kind, sig)) = d.lookup_signal_any_kind(id) {
                if !sig.excluded {
                    return Some((other_kind, sig));
                }
            }
        }
        None
    }
}

/// Formats a lowercase signal kind identifier in title case for diagnostic messages.
fn capitalize_kind(kind: &str) -> String {
    match kind {
        "metric" => "Metric".to_owned(),
        "span" => "Span".to_owned(),
        "event" => "Event".to_owned(),
        "entity" => "Entity".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_resolved_schema::v2::attribute::Attribute;
    use weaver_resolved_schema::v2::provenance::Provenance;
    use weaver_resolved_schema::v2::refinements::Refinements;
    use weaver_resolved_schema::v2::registry::Registry;
    use weaver_semconv::v2::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};

    #[test]
    fn test_v2_dependency_lookup_attribute() {
        let attr = Attribute {
            key: "test.key".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };
        let schema = weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: SchemaUrl::try_from("https://example.com/v2/1.0.0").unwrap(),
            attribute_catalog: vec![attr],
            registry: Registry {
                attributes: vec![weaver_resolved_schema::v2::attribute::AttributeRef(0)],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };
        let dep = V2Dependency::new(Arc::new(schema));
        let looked_up = dep.lookup_attribute("test.key").unwrap().expect("found");
        assert_eq!(looked_up.attribute.key, "test.key");
        assert_eq!(looked_up.origin.as_str(), "https://example.com/v2/1.0.0");
        assert!(!looked_up.excluded);

        assert!(dep.lookup_attribute("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_v2_dependency_lookup_signal() {
        use weaver_resolved_schema::v2::metric::Metric;
        use weaver_semconv::v2::metric::InstrumentSpec;

        let metric = Metric {
            name: SignalId::from("test.metric"),
            instrument: InstrumentSpec::Counter,
            unit: "1".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };

        let schema = weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: SchemaUrl::try_from("https://example.com/v2/1.0.0").unwrap(),
            attribute_catalog: vec![],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![metric],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        };

        let dep = V2Dependency::new(Arc::new(schema));
        let sig = dep.lookup_signal("metric", "test.metric").expect("found");
        assert_eq!(sig.kind(), "metric");
        assert_eq!(sig.origin.as_str(), "https://example.com/v2/1.0.0");
        assert!(!sig.excluded);

        // Wrong kind lookup returns None for specific kind
        assert!(dep.lookup_signal("span", "test.metric").is_none());

        // Any kind finds it
        let (found_kind, found_sig) = dep
            .lookup_signal_any_kind("test.metric")
            .expect("found any");
        assert_eq!(found_kind, "metric");
        assert_eq!(found_sig.origin.as_str(), "https://example.com/v2/1.0.0");
    }

    fn make_test_v2_schema(
        url_str: &str,
        attrs: Vec<Attribute>,
        metrics: Vec<weaver_resolved_schema::v2::metric::Metric>,
    ) -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        let attr_refs = (0..attrs.len())
            .map(|i| weaver_resolved_schema::v2::attribute::AttributeRef(i as u32))
            .collect();
        weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
            schema_url: SchemaUrl::try_from(url_str).unwrap(),
            attribute_catalog: attrs,
            registry: Registry {
                attributes: attr_refs,
                attribute_groups: vec![],
                spans: vec![],
                metrics,
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
        }
    }

    struct TestLookupContext {
        chosen_version: SchemaUrl,
        schema: Arc<WeaverResolvedSchema>,
    }

    impl SchemaCacheLookup for TestLookupContext {
        fn chosen_version(&self, _registry_name: &str) -> Option<&SchemaUrl> {
            Some(&self.chosen_version)
        }

        fn lookup_schema(&self, url: &SchemaUrl) -> Option<Arc<WeaverResolvedSchema>> {
            if url == &self.chosen_version {
                Some(self.schema.clone())
            } else {
                None
            }
        }
    }

    #[test]
    fn test_dependencies_ambiguous_attribute_across_major_versions() {
        let attr = Attribute {
            key: "shared.key".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };
        let s1 = make_test_v2_schema(
            "https://example.com/schemas/1.0.0",
            vec![attr.clone()],
            vec![],
        );
        let s2 = make_test_v2_schema("https://example.com/schemas/2.0.0", vec![attr], vec![]);
        let urls = [s1.schema_url.clone(), s2.schema_url.clone()]
            .into_iter()
            .collect();
        let deps = V2Dependencies::new(
            vec![
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s1))),
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s2))),
            ],
            urls,
        );

        let err = deps.lookup_attribute("shared.key", &()).unwrap_err();
        assert!(
            matches!(err, Error::AmbiguousReference { ref r#ref, .. } if r#ref == "shared.key"),
            "Expected AmbiguousReference, got: {err:?}"
        );
    }

    #[test]
    fn test_dependencies_ambiguous_signal_across_major_versions() {
        use weaver_resolved_schema::v2::metric::Metric;
        use weaver_semconv::v2::metric::InstrumentSpec;

        let metric = Metric {
            name: SignalId::from("shared.metric"),
            instrument: InstrumentSpec::Counter,
            unit: "1".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };
        let s1 = make_test_v2_schema(
            "https://example.com/schemas/1.0.0",
            vec![],
            vec![metric.clone()],
        );
        let s2 = make_test_v2_schema("https://example.com/schemas/2.0.0", vec![], vec![metric]);
        let urls = [s1.schema_url.clone(), s2.schema_url.clone()]
            .into_iter()
            .collect();
        let deps = V2Dependencies::new(
            vec![
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s1))),
                DependencyAdapter::V2(V2Dependency::new(Arc::new(s2))),
            ],
            urls,
        );

        let err = deps
            .lookup_signal("metric", "shared.metric", "refiner", None, &())
            .unwrap_err();
        assert!(
            matches!(err, Error::AmbiguousReference { ref r#ref, .. } if r#ref == "shared.metric"),
            "Expected AmbiguousReference, got: {err:?}"
        );
    }

    #[test]
    fn test_dependencies_upgraded_schema_missing_attribute() {
        let attr = Attribute {
            key: "upgraded.key".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };
        let s1 = make_test_v2_schema("https://example.com/schemas/1.0.0", vec![attr], vec![]);
        let chosen_schema =
            make_test_v2_schema("https://example.com/schemas/1.1.0", vec![], vec![]);
        let chosen_url = chosen_schema.schema_url.clone();
        let urls = [s1.schema_url.clone()].into_iter().collect();
        let deps = V2Dependencies::new(
            vec![DependencyAdapter::V2(V2Dependency::new(Arc::new(s1)))],
            urls,
        );

        let ctx = TestLookupContext {
            chosen_version: chosen_url,
            schema: Arc::new(WeaverResolvedSchema::V2(chosen_schema)),
        };

        let err = deps.lookup_attribute("upgraded.key", &ctx).unwrap_err();
        assert!(
            matches!(err, Error::AttributeNotFoundInUpgradedSchema { ref attribute_name, .. } if attribute_name == "upgraded.key"),
            "Expected AttributeNotFoundInUpgradedSchema, got: {err:?}"
        );
    }

    #[test]
    fn test_dependencies_upgraded_schema_missing_group() {
        use weaver_resolved_schema::v2::metric::Metric;
        use weaver_semconv::v2::metric::InstrumentSpec;

        let metric = Metric {
            name: SignalId::from("upgraded.metric"),
            instrument: InstrumentSpec::Counter,
            unit: "1".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: Default::default(),
            provenance: Provenance {
                source: None,
                path: "test.yaml".to_owned(),
            },
        };
        let s1 = make_test_v2_schema("https://example.com/schemas/1.0.0", vec![], vec![metric]);
        let chosen_schema =
            make_test_v2_schema("https://example.com/schemas/1.1.0", vec![], vec![]);
        let chosen_url = chosen_schema.schema_url.clone();
        let urls = [s1.schema_url.clone()].into_iter().collect();
        let deps = V2Dependencies::new(
            vec![DependencyAdapter::V2(V2Dependency::new(Arc::new(s1)))],
            urls,
        );

        let ctx = TestLookupContext {
            chosen_version: chosen_url,
            schema: Arc::new(WeaverResolvedSchema::V2(chosen_schema)),
        };

        let err = deps
            .lookup_signal("metric", "upgraded.metric", "refiner", None, &ctx)
            .unwrap_err();
        assert!(
            matches!(err, Error::GroupNotFoundInUpgradedSchema { ref group_id, .. } if group_id == "upgraded.metric"),
            "Expected GroupNotFoundInUpgradedSchema, got: {err:?}"
        );
    }
}
