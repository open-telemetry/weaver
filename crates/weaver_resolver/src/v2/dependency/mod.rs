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

use super::signals::{EntityAssociations, SignalKind};
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::span::{SpanKindSpec, SpanName};
use weaver_semconv::v2::CommonFields;

use crate::error::Error;
use crate::graph::chosen_versions::{upgrade_if_superseded, SchemaCacheLookup};
use crate::graph::conflict_strategy::pick_winning_by_origin;
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
    /// Returns the signal kind for this payload.
    #[must_use]
    #[cfg(test)]
    pub fn kind(&self) -> SignalKind {
        match &self.payload {
            DepSignalPayload::Metric { .. } => SignalKind::Metric,
            DepSignalPayload::Span { .. } => SignalKind::Span,
            DepSignalPayload::Event { .. } => SignalKind::Event,
            DepSignalPayload::Entity { .. } => SignalKind::Entity,
        }
    }
}

/// Version-abstracted adapter over a resolved dependency schema.
#[derive(Debug, Clone)]
pub(crate) enum DependencyAdapter {
    V2(Box<V2Dependency>),
    V1(Box<V1Dependency>),
}

impl DependencyAdapter {
    /// Creates an adapter from an `Arc<WeaverResolvedSchema>`.
    #[must_use]
    pub fn from_resolved_schema(schema: &Arc<WeaverResolvedSchema>) -> Self {
        match schema.as_ref() {
            WeaverResolvedSchema::V2(_) => {
                DependencyAdapter::V2(Box::new(V2Dependency::from_bundle(Arc::clone(schema))))
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
    pub fn lookup_signal(&self, kind: SignalKind, id: &str) -> Option<DepSignal> {
        match self {
            DependencyAdapter::V2(d) => d.lookup_signal(kind, id),
            DependencyAdapter::V1(d) => d.lookup_signal(kind, id),
        }
    }

    /// Looks up a signal by id across all kinds, returning its kind if found.
    pub fn lookup_signal_any_kind(&self, id: &str) -> Option<(SignalKind, DepSignal)> {
        for kind in SignalKind::ALL {
            if let Some(sig) = self.lookup_signal(kind, id) {
                return Some((kind, sig));
            }
        }
        None
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

        let Some(mut winning) = pick_winning_by_origin(matches, key, |a| &a.origin)? else {
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

        if let Some((chosen_url, upgraded)) = upgrade_if_superseded(
            &winning.origin,
            cache_lookup,
            |chosen_url, chosen_schema| {
                Ok(self
                    .adapter_for(chosen_url, chosen_schema)
                    .lookup_attribute(key)?
                    .filter(|a| !a.excluded))
            },
            |orig, chosen| Error::AttributeNotFoundInUpgradedSchema {
                attribute_name: key.to_owned(),
                original_url: orig.to_string(),
                upgraded_url: chosen.to_string(),
            },
        )? {
            winning = upgraded;
            winning.origin = chosen_url;
        }

        Ok(Some(winning))
    }

    /// Looks up a non-excluded signal across dependencies, resolving conflicts and version upgrades.
    pub fn lookup_signal<C: SchemaCacheLookup>(
        &self,
        kind: SignalKind,
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
                            refinement_type: kind.title().to_owned(),
                            signal_type: other_kind.title().to_owned(),
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

        let mut winning = pick_winning_by_origin(all_matches, id, |s| &s.origin)?
            .expect("all_matches is not empty");

        if let Some((chosen_url, upgraded_signal)) = upgrade_if_superseded(
            &winning.origin,
            cache_lookup,
            |chosen_url, chosen_schema| {
                Ok(self
                    .adapter_for(chosen_url, chosen_schema)
                    .lookup_signal(kind, id)
                    .filter(|s| !s.excluded))
            },
            |orig, chosen| Error::GroupNotFoundInUpgradedSchema {
                group_id: id.to_owned(),
                original_url: orig.to_string(),
                upgraded_url: chosen.to_string(),
            },
        )? {
            winning = upgraded_signal;
            winning.origin = chosen_url;
        }

        Ok(winning)
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

        let dep = DependencyAdapter::V2(Box::new(V2Dependency::new(Arc::new(schema))));
        let sig = dep
            .lookup_signal(SignalKind::Metric, "test.metric")
            .expect("found");
        assert_eq!(sig.kind(), SignalKind::Metric);
        assert_eq!(sig.origin.as_str(), "https://example.com/v2/1.0.0");
        assert!(!sig.excluded);

        // Wrong kind lookup returns None for specific kind
        assert!(dep.lookup_signal(SignalKind::Span, "test.metric").is_none());

        // Any kind finds it
        let (found_kind, found_sig) = dep
            .lookup_signal_any_kind("test.metric")
            .expect("found any");
        assert_eq!(found_kind, SignalKind::Metric);
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
                DependencyAdapter::V2(Box::new(V2Dependency::new(Arc::new(s1)))),
                DependencyAdapter::V2(Box::new(V2Dependency::new(Arc::new(s2)))),
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
                DependencyAdapter::V2(Box::new(V2Dependency::new(Arc::new(s1)))),
                DependencyAdapter::V2(Box::new(V2Dependency::new(Arc::new(s2)))),
            ],
            urls,
        );

        let err = deps
            .lookup_signal(SignalKind::Metric, "shared.metric", "refiner", None, &())
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
            vec![DependencyAdapter::V2(Box::new(V2Dependency::new(
                Arc::new(s1),
            )))],
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
            vec![DependencyAdapter::V2(Box::new(V2Dependency::new(
                Arc::new(s1),
            )))],
            urls,
        );

        let ctx = TestLookupContext {
            chosen_version: chosen_url,
            schema: Arc::new(WeaverResolvedSchema::V2(chosen_schema)),
        };

        let err = deps
            .lookup_signal(SignalKind::Metric, "upgraded.metric", "refiner", None, &ctx)
            .unwrap_err();
        assert!(
            matches!(err, Error::GroupNotFoundInUpgradedSchema { ref group_id, .. } if group_id == "upgraded.metric"),
            "Expected GroupNotFoundInUpgradedSchema, got: {err:?}"
        );
    }
}
