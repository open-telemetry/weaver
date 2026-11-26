//! Version two of registry specification.

use crate::{
    error::Error,
    v2::{
        attribute::Attribute,
        attribute_group::{AttributeGroup, AttributeGroupAttribute},
        entity::{
            from_resolved_associations, Entity, EntityAttribute, EntityRef, EntityRefinement,
        },
        event::{Event, EventAttribute, EventRefinement},
        metric::{Metric, MetricAttribute, MetricRefinement},
        provenance::Provenance,
        span::{Span, SpanAttribute, SpanRefinement},
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::iter;
use weaver_common::result::WResult;
use weaver_resolved_schema::v2::{
    attribute::AttributeRef,
    catalog::AttributeCatalog,
    entity::{EntityAssociation as ResolvedEntityAssociation, EntityAttributeRef},
    provenance::Provenance as ResolvedProvenance,
    ResolvedTelemetrySchema,
};
use weaver_resolver::{Error as ResolverError, SchemaResolver, WeaverResolvedSchema};
use weaver_semconv::schema_url::SchemaUrl;

/// A resolved semantic convention registry used in the context of the template and policy
/// engines.
///
/// This includes all registries fully fleshed out and ready for codegen.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForgeResolvedRegistry {
    /// The semantic convention registry url.
    pub schema_url: SchemaUrl,
    // TODO - Attribute Groups
    /// The signals defined in this registry.
    pub registry: Registry,
    /// The set of refinments defined in this registry.
    pub refinements: Refinements,
    /// Every registry this one depends on, directly or indirectly, keyed by
    /// schema url.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<SchemaUrl, ForgeDependency>,
    /// The direct dependencies of each registry, this one included. Each edge
    /// records the resolved (chosen) `schema_url` and, when dependency version
    /// conflict resolution upgraded the declared version, `upgraded_from`. A registry
    /// with no entry has unknown dependencies, not none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependency_graph: BTreeMap<SchemaUrl, Vec<DependencyEdge>>,
}

/// A directed edge in [`ForgeResolvedRegistry::dependency_graph`].
///
/// `schema_url` is always the resolved (chosen) dependency URL after dependency
/// version conflict resolution. When dependency version conflict resolution
/// upgraded the originally declared version to a higher compatible version,
/// `upgraded_from` records the declared URL before the upgrade.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyEdge {
    /// The resolved (chosen) schema URL of the direct dependency.
    pub schema_url: SchemaUrl,
    /// The originally declared schema URL when dependency resolution upgraded
    /// it to `schema_url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgraded_from: Option<SchemaUrl>,
}

impl DependencyEdge {
    /// Creates an edge whose resolved schema URL matches the declared URL.
    #[must_use]
    pub fn new(schema_url: SchemaUrl) -> Self {
        Self {
            schema_url,
            upgraded_from: None,
        }
    }

    /// Creates an edge recording that `upgraded_from` was upgraded to `schema_url`.
    #[must_use]
    pub fn upgraded(schema_url: SchemaUrl, upgraded_from: SchemaUrl) -> Self {
        Self {
            schema_url,
            upgraded_from: Some(upgraded_from),
        }
    }

    /// Returns the originally declared schema URL for this dependency edge.
    #[must_use]
    pub fn declared_schema_url(&self) -> &SchemaUrl {
        self.upgraded_from.as_ref().unwrap_or(&self.schema_url)
    }

    /// Returns `true` when dependency resolution upgraded this edge from its declared version.
    #[must_use]
    pub fn is_upgraded(&self) -> bool {
        self.upgraded_from.is_some()
    }
}

impl From<SchemaUrl> for DependencyEdge {
    fn from(schema_url: SchemaUrl) -> Self {
        Self::new(schema_url)
    }
}

/// A dependency of a [`ForgeResolvedRegistry`], stored under its schema url.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForgeDependency {
    /// The signals defined in this registry.
    pub registry: Registry,
    /// The set of refinements defined in this registry.
    pub refinements: Refinements,
}

/// Finds the entity matching `name` across `registry.entities` and `refinements.entities`,
/// since entity types and refinement IDs share a namespace.
fn defined_entity<'a>(
    registry: &'a Registry,
    refinements: &'a Refinements,
    name: &str,
) -> Option<&'a Entity> {
    registry
        .entities
        .iter()
        .find(|entity| &*entity.r#type == name)
        .or_else(|| {
            refinements
                .entities
                .iter()
                .find(|refinement| &*refinement.id == name)
                .map(|refinement| &refinement.entity)
        })
}

/// The set of all defined signals for a given semantic convention registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The raw attributes in this registry.
    pub attributes: Vec<Attribute>,
    /// The public attribute groups in this registry.
    pub attribute_groups: Vec<AttributeGroup>,
    /// The metric signals defined.
    pub metrics: Vec<Metric>,
    /// The span signals defined.
    pub spans: Vec<Span>,
    /// The event signals defined.
    pub events: Vec<Event>,
    /// The entity signals defined.
    pub entities: Vec<Entity>,
}

/// The set of all refinements for a semantic convention registry.
///
/// A refinement is a specialization of a signal for a particular purpose,
/// e.g. creating a MySQL specific instance of a database span for the purpose
/// of codegeneration for MySQL.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Refinements {
    /// The metric refinements defined.
    pub metrics: Vec<MetricRefinement>,
    /// The span refinements defined.
    pub spans: Vec<SpanRefinement>,
    /// The event refinements defined.
    pub events: Vec<EventRefinement>,
    /// The entity refinements defined.
    pub entities: Vec<EntityRefinement>,
}

/// Returns the chosen URL from `chosen_by_major` when `declared` has a valid
/// SemVer version in the same registry and major series that was superseded by
/// a different chosen URL.
fn find_upgraded_url<'a>(
    declared: &SchemaUrl,
    chosen_by_major: &'a HashMap<(String, u64), SchemaUrl>,
) -> Option<&'a SchemaUrl> {
    let declared_ver = declared.semver().ok()?;
    let chosen = chosen_by_major.get(&(declared.name().to_owned(), declared_ver.major))?;
    (chosen != declared).then_some(chosen)
}

/// Computes the shortest directed path length from `start` to `target` in `graph`.
fn distance_in_dependency_graph(
    graph: &BTreeMap<SchemaUrl, Vec<DependencyEdge>>,
    start: &SchemaUrl,
    target: &SchemaUrl,
) -> Option<usize> {
    if start == target {
        return Some(0);
    }
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back((start, 0));
    let _ = visited.insert(start);

    while let Some((curr, dist)) = queue.pop_front() {
        if let Some(directs) = graph.get(curr) {
            for edge in directs {
                if &edge.schema_url == target || edge.upgraded_from.as_ref() == Some(target) {
                    return Some(dist + 1);
                }
                if visited.insert(&edge.schema_url) {
                    queue.push_back((&edge.schema_url, dist + 1));
                }
            }
        }
    }
    None
}

impl ForgeResolvedRegistry {
    /// Resolves `url` to the chosen dependency URL in this registry.
    ///
    /// If `url` is an originally declared version that was upgraded during
    /// dependency resolution, returns the chosen `schema_url` from
    /// [`Self::dependency_graph`]; otherwise returns `url` unchanged.
    #[must_use]
    pub fn resolve_dependency_url<'a>(&'a self, url: &'a SchemaUrl) -> &'a SchemaUrl {
        if self.dependencies.contains_key(url) {
            return url;
        }
        self.dependency_graph
            .values()
            .flatten()
            .find(|edge| edge.upgraded_from.as_ref() == Some(url))
            .map_or(url, |edge| &edge.schema_url)
    }

    /// Looks up a dependency by either its chosen [`SchemaUrl`] or an
    /// originally declared URL that was upgraded during resolution.
    #[must_use]
    pub fn lookup_dependency(&self, url: &SchemaUrl) -> Option<&ForgeDependency> {
        self.dependencies.get(self.resolve_dependency_url(url))
    }

    /// Returns dependencies that transitively depend on `target_url`, ordered nearest-first.
    fn candidate_dependencies_for_target(
        &self,
        target_url: &SchemaUrl,
    ) -> Vec<(&SchemaUrl, &ForgeDependency)> {
        let mut candidates: Vec<(usize, usize, &SchemaUrl, &ForgeDependency)> = Vec::new();
        let nearest = self.dependencies_nearest_first();

        for (root_idx, (dep_url, dep)) in nearest.into_iter().enumerate() {
            if let Some(dist) =
                distance_in_dependency_graph(&self.dependency_graph, dep_url, target_url)
            {
                candidates.push((dist, root_idx, dep_url, dep));
            }
        }

        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        candidates
            .into_iter()
            .map(|(_, _, url, dep)| (url, dep))
            .collect()
    }

    /// Resolves `entity_ref` to its [`Entity`] definition, following upgraded dependency URLs
    /// and falling back to loaded dependencies in `dependency_graph` ordered nearest-first when
    /// the source registry is not directly loaded.
    pub fn lookup_entity(&self, entity_ref: &EntityRef) -> Result<&Entity, Error> {
        let target_url = match &entity_ref.provenance.source {
            None => {
                return defined_entity(&self.registry, &self.refinements, &entity_ref.r#type)
                    .ok_or_else(|| Error::EntityNotFound {
                        entity_type: entity_ref.r#type.to_string(),
                        registry: None,
                    });
            }
            Some(url) => url,
        };

        let effective_url = self.resolve_dependency_url(target_url);

        if let Some(dep) = self.dependencies.get(effective_url) {
            if let Some(entity) =
                defined_entity(&dep.registry, &dep.refinements, &entity_ref.r#type)
            {
                return Ok(entity);
            }
        }

        let candidate_deps = self.candidate_dependencies_for_target(effective_url);

        for (_dep_url, dep) in &candidate_deps {
            if let Some(entity) =
                defined_entity(&dep.registry, &dep.refinements, &entity_ref.r#type)
            {
                if entity
                    .provenance
                    .source
                    .as_ref()
                    .is_some_and(|s| s == effective_url || s == target_url)
                {
                    return Ok(entity);
                }
            }
        }

        Err(Error::EntityNotFound {
            entity_type: entity_ref.r#type.to_string(),
            registry: Some(target_url.to_string()),
        })
    }

    /// Returns dependencies in breadth-first order of `dependency_graph`, followed by any
    /// unreachable entries in [`SchemaUrl`] order.
    #[must_use]
    pub fn dependencies_nearest_first(&self) -> Vec<(&SchemaUrl, &ForgeDependency)> {
        let mut ordered = Vec::with_capacity(self.dependencies.len());
        let mut seen = HashSet::with_capacity(self.dependencies.len() + 1);
        let _ = seen.insert(&self.schema_url);
        let mut queue: VecDeque<&SchemaUrl> = self
            .dependency_graph
            .get(&self.schema_url)
            .map(|direct| direct.iter().map(|edge| &edge.schema_url).collect())
            .unwrap_or_default();

        while let Some(url) = queue.pop_front() {
            if !seen.insert(url) {
                continue;
            }
            if let Some(entry) = self.dependencies.get_key_value(url) {
                ordered.push(entry);
            }
            if let Some(direct) = self.dependency_graph.get(url) {
                queue.extend(direct.iter().map(|edge| &edge.schema_url));
            }
        }

        ordered.extend(
            self.dependencies
                .iter()
                .filter(|(url, _)| !seen.contains(url)),
        );
        ordered
    }

    /// Builds a [`ForgeResolvedRegistry`] from a [`ResolvedTelemetrySchema`], resolving
    /// dependencies via `resolver`.
    ///
    /// Note: Dependencies SHOULD be in cache, following normal resolution.
    pub fn try_from_resolved_schema<R: SchemaResolver>(
        schema: ResolvedTelemetrySchema,
        resolver: &mut R,
    ) -> WResult<Self, Error> {
        let mut non_fatal_errors = Vec::new();
        let schema_url = schema.schema_url.clone();
        // The root schema's `dependencies` set holds the chosen version selected by
        // dependency version conflict resolution for each `(registry_name, major_version)`.
        // Seed `chosen_by_major` from it so superseded versions declared by intermediate
        // dependencies are not materialized alongside the chosen one and are recorded as
        // `upgraded_from` edges in `dependency_graph`.
        let mut chosen_by_major: HashMap<(String, u64), SchemaUrl> = schema
            .dependencies
            .iter()
            .filter_map(|url| {
                let ver = url.semver().ok()?;
                Some(((url.name().to_owned(), ver.major), url.clone()))
            })
            .collect();
        let mut pending: Vec<SchemaUrl> = schema.dependencies.iter().cloned().collect();

        let (registry, refinements) = match Self::materialize(schema) {
            Ok(parts) => parts,
            Err(e) => return WResult::FatalErr(e),
        };

        let mut dependencies: BTreeMap<SchemaUrl, ForgeDependency> = BTreeMap::new();
        while let Some(dep_url) = pending.pop() {
            if dependencies.contains_key(&dep_url) {
                continue;
            }
            let (resolved_bundle, dep_nfes) = match resolver.resolve_schema(&dep_url) {
                WResult::Ok(bundle) => (bundle, vec![]),
                WResult::OkWithNFEs(bundle, nfes) => (bundle, nfes),
                WResult::FatalErr(ResolverError::SchemaNotInCache { schema_url }) => {
                    // An offline cache may omit transitive dependencies of published packages;
                    // warn instead of silently dropping them from materialized dependencies.
                    non_fatal_errors.push(Error::MissingDependencyWarning {
                        schema_url: schema_url.to_string(),
                    });
                    continue;
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e.into()),
            };
            non_fatal_errors.extend(dep_nfes.into_iter().map(Error::from));

            let dep_schema = match &*resolved_bundle {
                WeaverResolvedSchema::V2(v2) => v2.clone(),
                WeaverResolvedSchema::V1(v1) => {
                    match ResolvedTelemetrySchema::try_from(v1.clone()) {
                        Ok(v2) => v2,
                        Err(e) => return WResult::FatalErr(Error::from(e)),
                    }
                }
            };
            for sub_url in &dep_schema.dependencies {
                let effective_url =
                    if let Some(chosen) = find_upgraded_url(sub_url, &chosen_by_major) {
                        chosen.clone()
                    } else {
                        if let Ok(ver) = sub_url.semver() {
                            let _ = chosen_by_major
                                .entry((sub_url.name().to_owned(), ver.major))
                                .or_insert_with(|| sub_url.clone());
                        }
                        sub_url.clone()
                    };
                if !dependencies.contains_key(&effective_url) {
                    pending.push(effective_url);
                }
            }

            let (dep_registry, dep_refinements) = match Self::materialize(dep_schema) {
                Ok(parts) => parts,
                Err(e) => return WResult::FatalErr(e),
            };
            let _ = dependencies.insert(
                dep_url,
                ForgeDependency {
                    registry: dep_registry,
                    refinements: dep_refinements,
                },
            );
        }

        let resolve_edge = |declared: &SchemaUrl| -> DependencyEdge {
            if dependencies.contains_key(declared) {
                return DependencyEdge::new(declared.clone());
            }
            match find_upgraded_url(declared, &chosen_by_major) {
                Some(chosen) => DependencyEdge::upgraded(chosen.clone(), declared.clone()),
                None => DependencyEdge::new(declared.clone()),
            }
        };

        let dependency_graph = iter::once(&schema_url)
            .chain(dependencies.keys())
            .filter_map(|url| {
                resolver.direct_dependencies(url).map(|direct| {
                    let edges = direct.iter().map(resolve_edge).collect();
                    (url.clone(), edges)
                })
            })
            .collect();

        let forge_registry = Self {
            schema_url,
            registry,
            refinements,
            dependencies,
            dependency_graph,
        };

        WResult::with_non_fatal_errors(forge_registry, non_fatal_errors)
    }

    /// Converts a [`ResolvedTelemetrySchema`] into materialized [`Registry`] and [`Refinements`] models.
    fn materialize(schema: ResolvedTelemetrySchema) -> Result<(Registry, Refinements), Error> {
        let mut errors = Vec::new();

        let deps_list: Vec<_> = schema.dependencies.iter().cloned().collect();
        let resolve_provenance =
            |prov: &ResolvedProvenance| Provenance::from_resolved(prov, &deps_list);
        let resolve_associations =
            |assocs: &[ResolvedEntityAssociation]| from_resolved_associations(assocs, &deps_list);

        let attribute_lookup = |r: &AttributeRef| schema.attribute_catalog.attribute(r);
        let mut attributes: Vec<Attribute> = schema
            .registry
            .attributes
            .iter()
            .filter_map(&attribute_lookup)
            .map(|a| Attribute {
                key: a.key.clone(),
                r#type: a.r#type.clone(),
                examples: a.examples.clone(),
                common: a.common.clone(),
                provenance: resolve_provenance(&a.provenance),
            })
            .collect();

        let mut metrics = Vec::new();
        for metric in schema.registry.metrics {
            let attributes = metric
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| MetricAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("metric.{}", metric.name),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            metrics.push(Metric {
                name: metric.name,
                instrument: metric.instrument,
                unit: metric.unit,
                attributes,
                entity_associations: resolve_associations(&metric.entity_associations),
                requirement_level: metric.requirement_level,
                common: metric.common,
                provenance: resolve_provenance(&metric.provenance),
            });
        }
        metrics.sort_by(|l, r| l.name.cmp(&r.name));

        let mut metric_refinements: Vec<MetricRefinement> = Vec::new();
        for metric in schema.refinements.metrics {
            let attributes = metric
                .metric
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| MetricAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("metric.{}", metric.metric.name),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            metric_refinements.push(MetricRefinement {
                id: metric.id.clone(),
                metric: Metric {
                    name: metric.metric.name,
                    instrument: metric.metric.instrument,
                    unit: metric.metric.unit,
                    attributes,
                    entity_associations: resolve_associations(&metric.metric.entity_associations),
                    requirement_level: metric.metric.requirement_level,
                    common: metric.metric.common,
                    provenance: resolve_provenance(&metric.metric.provenance),
                },
            });
        }
        metric_refinements.sort_by(|l, r| l.id.cmp(&r.id));

        let mut spans = Vec::new();
        for span in schema.registry.spans {
            let attributes = span
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| SpanAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                        sampling_relevant: ar.sampling_relevant,
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("span.{}", span.r#type),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            spans.push(Span {
                r#type: span.r#type,
                kind: span.kind,
                name: span.name,
                attributes,
                entity_associations: resolve_associations(&span.entity_associations),
                requirement_level: span.requirement_level,
                common: span.common,
                provenance: resolve_provenance(&span.provenance),
            });
        }
        spans.sort_by(|l, r| l.r#type.cmp(&r.r#type));
        let mut span_refinements = Vec::new();
        for span in schema.refinements.spans {
            let attributes = span
                .span
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| SpanAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                        sampling_relevant: ar.sampling_relevant,
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("span.{}", span.id),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            span_refinements.push(SpanRefinement {
                id: span.id,
                span: Span {
                    r#type: span.span.r#type,
                    kind: span.span.kind,
                    name: span.span.name,
                    attributes,
                    entity_associations: resolve_associations(&span.span.entity_associations),
                    requirement_level: span.span.requirement_level,
                    common: span.span.common,
                    provenance: resolve_provenance(&span.span.provenance),
                },
            });
        }
        span_refinements.sort_by(|l, r| l.id.cmp(&r.id));

        let mut events = Vec::new();
        for event in schema.registry.events {
            let attributes = event
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| EventAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("event.{}", event.name),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            events.push(Event {
                name: event.name,
                attributes,
                entity_associations: resolve_associations(&event.entity_associations),
                requirement_level: event.requirement_level,
                common: event.common,
                provenance: resolve_provenance(&event.provenance),
            });
        }
        events.sort_by(|l, r| l.name.cmp(&r.name));

        let mut event_refinements = Vec::new();
        for event in schema.refinements.events {
            let attributes = event
                .event
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| EventAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("event.{}", event.id),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            event_refinements.push(EventRefinement {
                id: event.id,
                event: Event {
                    name: event.event.name,
                    attributes,
                    entity_associations: resolve_associations(&event.event.entity_associations),
                    requirement_level: event.event.requirement_level,
                    common: event.event.common,
                    provenance: resolve_provenance(&event.event.provenance),
                },
            });
        }
        event_refinements.sort_by(|l, r| l.id.cmp(&r.id));

        let convert_identity_attrs =
            |attrs: &[AttributeRef], group_id: &str, errors: &mut Vec<Error>| -> Vec<Attribute> {
                attrs
                    .iter()
                    .filter_map(|ar| {
                        let attr = attribute_lookup(ar).map(|a| Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        });
                        if attr.is_none() {
                            errors.push(Error::AttributeNotFound {
                                group_id: group_id.to_owned(),
                                attr_ref: ar.0,
                            });
                        }
                        attr
                    })
                    .collect()
            };

        let convert_entity_attrs = |attrs: &[EntityAttributeRef],
                                    group_id: &str,
                                    errors: &mut Vec<Error>|
         -> Vec<EntityAttribute> {
            attrs
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| EntityAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: group_id.to_owned(),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect()
        };

        let mut entities = Vec::new();
        for e in schema.registry.entities {
            let group_id = format!("entity.{}", e.r#type);
            let identity = convert_identity_attrs(&e.identity, &group_id, &mut errors);
            let description = convert_entity_attrs(&e.description, &group_id, &mut errors);
            entities.push(Entity {
                r#type: e.r#type,
                identity,
                description,
                requirement_level: e.requirement_level,
                common: e.common,
                provenance: resolve_provenance(&e.provenance),
            });
        }
        entities.sort_by(|l, r| l.r#type.cmp(&r.r#type));

        let mut entity_refinements = Vec::new();
        for e in schema.refinements.entities {
            let group_id = format!("entity.{}", e.id);
            let identity = convert_identity_attrs(&e.entity.identity, &group_id, &mut errors);
            let description = convert_entity_attrs(&e.entity.description, &group_id, &mut errors);
            entity_refinements.push(EntityRefinement {
                id: e.id,
                entity: Entity {
                    r#type: e.entity.r#type,
                    identity,
                    description,
                    requirement_level: e.entity.requirement_level,
                    common: e.entity.common,
                    provenance: resolve_provenance(&e.entity.provenance),
                },
            });
        }
        entity_refinements.sort_by(|l, r| l.id.cmp(&r.id));

        let mut attribute_groups = Vec::new();
        for ag in schema.registry.attribute_groups {
            let attributes = ag
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| AttributeGroupAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                            provenance: resolve_provenance(&a.provenance),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("attribute_group.{}", ag.id),
                            attr_ref: ar.base.0,
                        });
                    }
                    attr
                })
                .collect();
            attribute_groups.push(AttributeGroup {
                id: ag.id,
                attributes,
                common: ag.common.clone(),
                provenance: resolve_provenance(&ag.provenance),
            });
        }
        attribute_groups.sort_by(|l, r| l.id.cmp(&r.id));
        attributes.sort_by(|l, r| l.key.cmp(&r.key));

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok((
            Registry {
                attributes,
                attribute_groups,
                metrics,
                spans,
                events,
                entities,
            },
            Refinements {
                metrics: metric_refinements,
                spans: span_refinements,
                events: event_refinements,
                entities: entity_refinements,
            },
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::sync::Arc;

    use crate::v2::entity::EntityAssociation;
    use schemars::schema_for;
    use serde_json::to_string_pretty;
    use weaver_resolved_schema::v2::{
        self,
        attribute::{self, AttributeRef},
        attribute_group, entity, event, metric, provenance, refinements, span,
        ResolvedTelemetrySchema,
    };
    use weaver_resolver::NullSchemaResolver;
    use weaver_semconv::{
        schema_url::SchemaUrl,
        v2::{
            attribute::{
                AttributeType, BasicRequirementLevelSpec, Examples, PrimitiveOrArrayTypeSpec,
                RequirementLevel,
            },
            metric::InstrumentSpec,
            signal_id::SignalId,
            signal_requirement_level::SignalRequirementLevel,
            span::{SpanKindSpec, SpanName},
            stability::Stability,
            CommonFields,
        },
    };

    use super::*;

    enum MockResolution {
        Ok(Arc<WeaverResolvedSchema>),
        OkWithNFEs(Arc<WeaverResolvedSchema>, Vec<weaver_resolver::Error>),
        Fatal(weaver_resolver::Error),
    }

    pub(crate) struct MockSchemaResolver {
        schemas: HashMap<SchemaUrl, MockResolution>,
        direct_dependencies: HashMap<SchemaUrl, Vec<SchemaUrl>>,
    }

    impl MockSchemaResolver {
        pub(crate) fn new() -> Self {
            Self {
                schemas: HashMap::new(),
                direct_dependencies: HashMap::new(),
            }
        }

        fn add_v2_schema(&mut self, schema: ResolvedTelemetrySchema) {
            let _ = self.schemas.insert(
                schema.schema_url.clone(),
                MockResolution::Ok(Arc::new(WeaverResolvedSchema::V2(schema))),
            );
        }

        pub(crate) fn add_resolved_schema(&mut self, url: SchemaUrl, schema: WeaverResolvedSchema) {
            let _ = self
                .schemas
                .insert(url, MockResolution::Ok(Arc::new(schema)));
        }

        fn add_with_nfes(
            &mut self,
            url: SchemaUrl,
            schema: WeaverResolvedSchema,
            nfes: Vec<weaver_resolver::Error>,
        ) {
            let _ = self
                .schemas
                .insert(url, MockResolution::OkWithNFEs(Arc::new(schema), nfes));
        }

        fn add_fatal(&mut self, url: SchemaUrl, error: weaver_resolver::Error) {
            let _ = self.schemas.insert(url, MockResolution::Fatal(error));
        }

        fn add_direct_dependencies(&mut self, url: SchemaUrl, direct: Vec<SchemaUrl>) {
            let _ = self.direct_dependencies.insert(url, direct);
        }
    }

    impl SchemaResolver for MockSchemaResolver {
        fn resolve_schema(
            &mut self,
            schema_url: &SchemaUrl,
        ) -> WResult<Arc<WeaverResolvedSchema>, weaver_resolver::Error> {
            if let Some(res) = self.schemas.get(schema_url) {
                match res {
                    MockResolution::Ok(s) => WResult::Ok(s.clone()),
                    MockResolution::OkWithNFEs(s, nfes) => {
                        WResult::OkWithNFEs(s.clone(), nfes.clone())
                    }
                    MockResolution::Fatal(e) => WResult::FatalErr(e.clone()),
                }
            } else {
                WResult::FatalErr(weaver_resolver::Error::FailToResolveSchemaUrl {})
            }
        }

        fn direct_dependencies(&self, schema_url: &SchemaUrl) -> Option<&[SchemaUrl]> {
            self.direct_dependencies
                .get(schema_url)
                .map(|urls| urls.as_slice())
        }
    }

    #[test]
    fn test_try_from_resolved_schema_all_signals_and_refinements() {
        let dep_url: SchemaUrl = "https://example.com/dependency".try_into().unwrap();
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/schema".try_into().unwrap(),
            attribute_catalog: vec![
                attribute::Attribute {
                    key: "test.attr".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: Some(Examples::String("example_value".to_owned())),
                    common: CommonFields {
                        brief: "Brief description".to_owned(),
                        note: Some("Note description".to_owned()),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                    provenance: provenance::Provenance {
                        source: Some(provenance::DependencyRef(0)),
                        path: "some/path.yaml".to_owned(),
                    },
                },
                attribute::Attribute {
                    key: "test.desc.attr".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: provenance::Provenance {
                        source: None,
                        path: "".to_owned(),
                    },
                },
            ],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![AttributeRef(0), AttributeRef(1)],
                spans: vec![span::Span {
                    links: vec![],
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: Some("My Span".to_owned()),
                        ..Default::default()
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: Some(true),
                    }],
                    entity_associations: vec![entity::EntityAssociation::Ref(
                        entity::EntityRef::local("my-entity".to_owned().into()),
                    )],
                    requirement_level: Some(SignalRequirementLevel::OptIn),
                    common: CommonFields::default(),
                    provenance: provenance::Provenance {
                        source: Some(provenance::DependencyRef(0)),
                        path: "span.yaml".to_owned(),
                    },
                }],
                metrics: vec![metric::Metric {
                    name: SignalId::from("my-metric".to_owned()),
                    instrument: InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![metric::MetricAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![entity::EntityAssociation::Ref(
                        entity::EntityRef::local("my-entity".to_owned().into()),
                    )],
                    requirement_level: Some(SignalRequirementLevel::OptIn),
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                events: vec![event::Event {
                    name: SignalId::from("my-event".to_owned()),
                    attributes: vec![event::EventAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![entity::EntityAssociation::Ref(
                        entity::EntityRef::local("my-entity".to_owned().into()),
                    )],
                    requirement_level: Some(SignalRequirementLevel::Recommended),
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                entities: vec![entity::Entity {
                    r#type: SignalId::from("my-entity".to_owned()),
                    identity: vec![AttributeRef(0)],
                    description: vec![EntityAttributeRef {
                        base: AttributeRef(1),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Recommended,
                        ),
                    }],
                    requirement_level: Some(SignalRequirementLevel::Recommended),
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                attribute_groups: vec![attribute_group::AttributeGroup {
                    id: SignalId::from("my-group".to_owned()),
                    attributes: vec![attribute_group::AttributeGroupAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
            },
            refinements: refinements::Refinements {
                spans: vec![span::SpanRefinement {
                    id: SignalId::from("my-refined-span".to_owned()),
                    span: span::Span {
                        r#type: SignalId::from("my-span".to_owned()),
                        kind: SpanKindSpec::Client,
                        name: SpanName {
                            note: Some("My Refined Span".to_owned()),
                            ..Default::default()
                        },
                        attributes: vec![span::SpanAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: Some(false),
                        }],
                        entity_associations: vec![],
                        links: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                metrics: vec![metric::MetricRefinement {
                    id: SignalId::from("my-refined-metric".to_owned()),
                    metric: metric::Metric {
                        name: SignalId::from("my-metric".to_owned()),
                        instrument: InstrumentSpec::Histogram,
                        unit: "ms".to_owned(),
                        attributes: vec![metric::MetricAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Recommended,
                            ),
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                events: vec![event::EventRefinement {
                    id: SignalId::from("my-refined-event".to_owned()),
                    event: event::Event {
                        name: SignalId::from("my-event".to_owned()),
                        attributes: vec![event::EventAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::OptIn,
                            ),
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                entities: vec![entity::EntityRefinement {
                    id: SignalId::from("my-refined-entity".to_owned()),
                    entity: entity::Entity {
                        r#type: SignalId::from("my-entity".to_owned()),
                        identity: vec![AttributeRef(0)],
                        description: vec![EntityAttributeRef {
                            base: AttributeRef(1),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Recommended,
                            ),
                        }],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
            },
        };

        let dep_resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_url.clone(),
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(dep_resolved_schema);

        let forge_registry = match ForgeResolvedRegistry::try_from_resolved_schema(
            resolved_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) => r,
            WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert_eq!(
            forge_registry.dependencies.keys().collect::<Vec<_>>(),
            vec![&dep_url]
        );

        assert_eq!(forge_registry.registry.attributes.len(), 2);
        assert_eq!(forge_registry.registry.spans.len(), 1);
        assert_eq!(forge_registry.registry.metrics.len(), 1);
        assert_eq!(forge_registry.registry.events.len(), 1);
        assert_eq!(forge_registry.registry.entities.len(), 1);
        assert_eq!(forge_registry.registry.attribute_groups.len(), 1);

        let attr0 = &forge_registry.registry.attributes[0];
        assert_eq!(attr0.key, "test.attr");
        assert_eq!(attr0.provenance.source, Some(dep_url.clone()));
        assert_eq!(attr0.provenance.path, Some("some/path.yaml".to_owned()));
        assert_eq!(
            attr0.examples,
            Some(Examples::String("example_value".to_owned()))
        );

        let attr1 = &forge_registry.registry.attributes[1];
        assert_eq!(attr1.key, "test.desc.attr");
        assert_eq!(attr1.provenance.source, None);
        assert_eq!(attr1.provenance.path, None);

        let group = &forge_registry.registry.attribute_groups[0];
        assert_eq!(group.id, "my-group".to_owned().into());
        assert_eq!(group.attributes.len(), 1);
        assert_eq!(group.attributes[0].base.key, "test.attr");
        assert_eq!(
            group.attributes[0].requirement_level,
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
        );

        let span = &forge_registry.registry.spans[0];
        assert_eq!(span.r#type, "my-span".to_owned().into());
        assert_eq!(span.attributes.len(), 1);
        assert_eq!(span.attributes[0].base.key, "test.attr");
        assert_eq!(span.attributes[0].sampling_relevant, Some(true));
        assert_eq!(
            span.entity_associations,
            vec![EntityAssociation::Ref(EntityRef::local(
                "my-entity".to_owned().into()
            ))]
        );
        assert_eq!(span.provenance.source, Some(dep_url.clone()));
        assert_eq!(span.provenance.path, Some("span.yaml".to_owned()));

        let metric = &forge_registry.registry.metrics[0];
        assert_eq!(metric.name, "my-metric".to_owned().into());
        assert_eq!(metric.instrument, InstrumentSpec::Counter);
        assert_eq!(metric.unit, "1");
        assert_eq!(metric.attributes.len(), 1);
        assert_eq!(metric.attributes[0].base.key, "test.attr");
        assert_eq!(
            metric.entity_associations,
            vec![EntityAssociation::Ref(EntityRef::local(
                "my-entity".to_owned().into()
            ))]
        );

        let event = &forge_registry.registry.events[0];
        assert_eq!(event.name, "my-event".to_owned().into());
        assert_eq!(event.attributes.len(), 1);
        assert_eq!(event.attributes[0].base.key, "test.attr");
        assert_eq!(
            event.entity_associations,
            vec![EntityAssociation::Ref(EntityRef::local(
                "my-entity".to_owned().into()
            ))]
        );

        let entity = &forge_registry.registry.entities[0];
        assert_eq!(entity.r#type, "my-entity".to_owned().into());
        assert_eq!(entity.identity.len(), 1);
        assert_eq!(entity.identity[0].key, "test.attr");
        assert_eq!(entity.description.len(), 1);
        assert_eq!(entity.description[0].base.key, "test.desc.attr");
        assert_eq!(
            entity.requirement_level,
            Some(SignalRequirementLevel::Recommended)
        );

        assert_eq!(forge_refinements_len(&forge_registry), (1, 1, 1, 1));

        let refined_span = &forge_registry.refinements.spans[0];
        assert_eq!(refined_span.id, "my-refined-span".to_owned().into());
        assert_eq!(refined_span.span.r#type, "my-span".to_owned().into());
        assert_eq!(refined_span.span.attributes.len(), 1);
        assert_eq!(refined_span.span.attributes[0].base.key, "test.attr");
        assert_eq!(
            refined_span.span.attributes[0].sampling_relevant,
            Some(false)
        );

        let refined_metric = &forge_registry.refinements.metrics[0];
        assert_eq!(refined_metric.id, "my-refined-metric".to_owned().into());
        assert_eq!(refined_metric.metric.name, "my-metric".to_owned().into());
        assert_eq!(refined_metric.metric.instrument, InstrumentSpec::Histogram);
        assert_eq!(refined_metric.metric.unit, "ms");
        assert_eq!(refined_metric.metric.attributes.len(), 1);
        assert_eq!(refined_metric.metric.attributes[0].base.key, "test.attr");

        let refined_event = &forge_registry.refinements.events[0];
        assert_eq!(refined_event.id, "my-refined-event".to_owned().into());
        assert_eq!(refined_event.event.name, "my-event".to_owned().into());
        assert_eq!(refined_event.event.attributes.len(), 1);
        assert_eq!(refined_event.event.attributes[0].base.key, "test.attr");

        let refined_entity = &forge_registry.refinements.entities[0];
        assert_eq!(refined_entity.id, "my-refined-entity".to_owned().into());
        assert_eq!(refined_entity.entity.r#type, "my-entity".to_owned().into());
        assert_eq!(refined_entity.entity.identity.len(), 1);
        assert_eq!(refined_entity.entity.identity[0].key, "test.attr");
        assert_eq!(refined_entity.entity.description.len(), 1);
        assert_eq!(
            refined_entity.entity.description[0].base.key,
            "test.desc.attr"
        );

        assert_conforms_to_json_schema(&forge_registry);
    }

    fn forge_refinements_len(forge: &ForgeResolvedRegistry) -> (usize, usize, usize, usize) {
        (
            forge.refinements.spans.len(),
            forge.refinements.metrics.len(),
            forge.refinements.events.len(),
            forge.refinements.entities.len(),
        )
    }

    /// Asserts that the serialized registry conforms to the published JSON Schema of
    /// `ForgeResolvedRegistry` (`weaver registry json-schema -j materialized-registry-v2`).
    fn assert_conforms_to_json_schema(forge: &ForgeResolvedRegistry) {
        let schema = serde_json::to_value(schema_for!(ForgeResolvedRegistry)).unwrap();
        let validator = jsonschema::validator_for(&schema).expect("JSON Schema should compile");
        let value = serde_json::to_value(forge).expect("serialization should succeed");
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("{}: {e}", e.instance_path()))
            .collect();
        assert!(
            errors.is_empty(),
            "serialized registry does not match its JSON Schema:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn test_try_from_resolved_schema_deterministic_sorting() {
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/schema".try_into().unwrap(),
            attribute_catalog: vec![
                attribute::Attribute {
                    key: "z.attr".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
                attribute::Attribute {
                    key: "a.attr".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
                attribute::Attribute {
                    key: "m.attr".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
            ],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                // Intentionally out of alphabetical order
                attributes: vec![AttributeRef(0), AttributeRef(1), AttributeRef(2)],
                spans: vec![
                    span::Span {
                        links: vec![],
                        r#type: SignalId::from("z-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: Some("".to_owned()),
                            ..Default::default()
                        },
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    span::Span {
                        links: vec![],
                        r#type: SignalId::from("a-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: Some("".to_owned()),
                            ..Default::default()
                        },
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
                metrics: vec![
                    metric::Metric {
                        name: SignalId::from("z-metric".to_owned()),
                        instrument: InstrumentSpec::Counter,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    metric::Metric {
                        name: SignalId::from("a-metric".to_owned()),
                        instrument: InstrumentSpec::Counter,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
                events: vec![
                    event::Event {
                        name: SignalId::from("z-event".to_owned()),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    event::Event {
                        name: SignalId::from("a-event".to_owned()),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
                entities: vec![
                    entity::Entity {
                        r#type: SignalId::from("z-entity".to_owned()),
                        identity: vec![],
                        description: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    entity::Entity {
                        r#type: SignalId::from("a-entity".to_owned()),
                        identity: vec![],
                        description: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![
                    span::SpanRefinement {
                        id: SignalId::from("z-span-ref".to_owned()),
                        span: span::Span {
                            links: vec![],
                            r#type: SignalId::from("z-span".to_owned()),
                            kind: SpanKindSpec::Internal,
                            name: SpanName {
                                note: Some("".to_owned()),
                                ..Default::default()
                            },
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                    span::SpanRefinement {
                        id: SignalId::from("a-span-ref".to_owned()),
                        span: span::Span {
                            links: vec![],
                            r#type: SignalId::from("a-span".to_owned()),
                            kind: SpanKindSpec::Internal,
                            name: SpanName {
                                note: Some("".to_owned()),
                                ..Default::default()
                            },
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                ],
                metrics: vec![
                    metric::MetricRefinement {
                        id: SignalId::from("z-metric-ref".to_owned()),
                        metric: metric::Metric {
                            name: SignalId::from("z-metric".to_owned()),
                            instrument: InstrumentSpec::Counter,
                            unit: "1".to_owned(),
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                    metric::MetricRefinement {
                        id: SignalId::from("a-metric-ref".to_owned()),
                        metric: metric::Metric {
                            name: SignalId::from("a-metric".to_owned()),
                            instrument: InstrumentSpec::Counter,
                            unit: "1".to_owned(),
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                ],
                events: vec![
                    event::EventRefinement {
                        id: SignalId::from("z-event-ref".to_owned()),
                        event: event::Event {
                            name: SignalId::from("z-event".to_owned()),
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                    event::EventRefinement {
                        id: SignalId::from("a-event-ref".to_owned()),
                        event: event::Event {
                            name: SignalId::from("a-event".to_owned()),
                            attributes: vec![],
                            entity_associations: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                ],
                entities: vec![
                    entity::EntityRefinement {
                        id: SignalId::from("z-entity-ref".to_owned()),
                        entity: entity::Entity {
                            r#type: SignalId::from("z-entity".to_owned()),
                            identity: vec![],
                            description: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                    entity::EntityRefinement {
                        id: SignalId::from("a-entity-ref".to_owned()),
                        entity: entity::Entity {
                            r#type: SignalId::from("a-entity".to_owned()),
                            identity: vec![],
                            description: vec![],
                            requirement_level: None,
                            common: CommonFields::default(),
                            provenance: Default::default(),
                        },
                    },
                ],
            },
        };

        let mut resolver = NullSchemaResolver;
        let forge =
            match ForgeResolvedRegistry::try_from_resolved_schema(resolved_schema, &mut resolver) {
                WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
                WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
            };

        assert_eq!(
            forge
                .registry
                .attributes
                .iter()
                .map(|a| a.key.as_str())
                .collect::<Vec<_>>(),
            vec!["a.attr", "m.attr", "z.attr"]
        );
        assert_eq!(
            forge
                .registry
                .spans
                .iter()
                .map(|s| &s.r#type[..])
                .collect::<Vec<_>>(),
            vec!["a-span", "z-span"]
        );
        assert_eq!(
            forge
                .registry
                .metrics
                .iter()
                .map(|m| &m.name[..])
                .collect::<Vec<_>>(),
            vec!["a-metric", "z-metric"]
        );
        assert_eq!(
            forge
                .registry
                .events
                .iter()
                .map(|e| &e.name[..])
                .collect::<Vec<_>>(),
            vec!["a-event", "z-event"]
        );
        assert_eq!(
            forge
                .registry
                .entities
                .iter()
                .map(|e| &e.r#type[..])
                .collect::<Vec<_>>(),
            vec!["a-entity", "z-entity"]
        );
        assert_eq!(
            forge
                .refinements
                .spans
                .iter()
                .map(|s| &s.id[..])
                .collect::<Vec<_>>(),
            vec!["a-span-ref", "z-span-ref"]
        );
        assert_eq!(
            forge
                .refinements
                .metrics
                .iter()
                .map(|m| &m.id[..])
                .collect::<Vec<_>>(),
            vec!["a-metric-ref", "z-metric-ref"]
        );
        assert_eq!(
            forge
                .refinements
                .events
                .iter()
                .map(|e| &e.id[..])
                .collect::<Vec<_>>(),
            vec!["a-event-ref", "z-event-ref"]
        );
        assert_eq!(
            forge
                .refinements
                .entities
                .iter()
                .map(|e| &e.id[..])
                .collect::<Vec<_>>(),
            vec!["a-entity-ref", "z-entity-ref"]
        );
    }

    #[test]
    fn test_provenance_resolution_edge_cases() {
        let dep_url: SchemaUrl = "https://example.com/dep".try_into().unwrap();
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/schema".try_into().unwrap(),
            attribute_catalog: vec![
                // Out-of-bounds dependency ref -> source should be None
                attribute::Attribute {
                    key: "attr.oob".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: provenance::Provenance {
                        source: Some(provenance::DependencyRef(999)),
                        path: "path.yaml".to_owned(),
                    },
                },
                // Empty path -> path should be None
                attribute::Attribute {
                    key: "attr.empty_path".to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: CommonFields::default(),
                    provenance: provenance::Provenance {
                        source: None,
                        path: "".to_owned(),
                    },
                },
            ],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![AttributeRef(0), AttributeRef(1)],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_url,
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        });

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            resolved_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        let attr_empty_path = forge
            .registry
            .attributes
            .iter()
            .find(|a| a.key == "attr.empty_path")
            .unwrap();
        assert_eq!(attr_empty_path.provenance.source, None);
        assert_eq!(attr_empty_path.provenance.path, None);

        let attr_oob = forge
            .registry
            .attributes
            .iter()
            .find(|a| a.key == "attr.oob")
            .unwrap();
        assert_eq!(attr_oob.provenance.source, None);
        assert_eq!(attr_oob.provenance.path, Some("path.yaml".to_owned()));
    }

    #[test]
    fn test_missing_attribute_errors_in_all_signals_and_refinements() {
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/schema".try_into().unwrap(),
            attribute_catalog: vec![], // Empty catalog, all refs are invalid
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![span::Span {
                    links: vec![],
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: Some("".to_owned()),
                        ..Default::default()
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: AttributeRef(10),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: None,
                    }],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                metrics: vec![metric::Metric {
                    name: SignalId::from("my-metric".to_owned()),
                    instrument: InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![metric::MetricAttributeRef {
                        base: AttributeRef(11),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                events: vec![event::Event {
                    name: SignalId::from("my-event".to_owned()),
                    attributes: vec![event::EventAttributeRef {
                        base: AttributeRef(12),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                entities: vec![entity::Entity {
                    r#type: SignalId::from("my-entity".to_owned()),
                    identity: vec![AttributeRef(13)],
                    description: vec![EntityAttributeRef {
                        base: AttributeRef(14),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Recommended,
                        ),
                    }],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                attribute_groups: vec![attribute_group::AttributeGroup {
                    id: SignalId::from("my-group".to_owned()),
                    attributes: vec![attribute_group::AttributeGroupAttributeRef {
                        base: AttributeRef(15),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
            },
            refinements: refinements::Refinements {
                spans: vec![span::SpanRefinement {
                    id: SignalId::from("refined-span".to_owned()),
                    span: span::Span {
                        links: vec![],
                        r#type: SignalId::from("my-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: Some("".to_owned()),
                            ..Default::default()
                        },
                        attributes: vec![span::SpanAttributeRef {
                            base: AttributeRef(16),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: None,
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                metrics: vec![metric::MetricRefinement {
                    id: SignalId::from("refined-metric".to_owned()),
                    metric: metric::Metric {
                        name: SignalId::from("my-metric".to_owned()),
                        instrument: InstrumentSpec::Counter,
                        unit: "1".to_owned(),
                        attributes: vec![metric::MetricAttributeRef {
                            base: AttributeRef(17),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                events: vec![event::EventRefinement {
                    id: SignalId::from("refined-event".to_owned()),
                    event: event::Event {
                        name: SignalId::from("my-event".to_owned()),
                        attributes: vec![event::EventAttributeRef {
                            base: AttributeRef(18),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                entities: vec![entity::EntityRefinement {
                    id: SignalId::from("refined-entity".to_owned()),
                    entity: entity::Entity {
                        r#type: SignalId::from("my-entity".to_owned()),
                        identity: vec![AttributeRef(19)],
                        description: vec![EntityAttributeRef {
                            base: AttributeRef(20),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Recommended,
                            ),
                        }],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
            },
        };

        let mut resolver = NullSchemaResolver;
        let result =
            ForgeResolvedRegistry::try_from_resolved_schema(resolved_schema, &mut resolver);
        assert!(result.is_fatal());

        if let WResult::FatalErr(Error::CompoundError(errors)) = result {
            assert_eq!(errors.len(), 11);

            let mut expected_errors = vec![
                ("span.my-span", 10),
                ("metric.my-metric", 11),
                ("event.my-event", 12),
                ("entity.my-entity", 13),
                ("entity.my-entity", 14),
                ("attribute_group.my-group", 15),
                ("span.refined-span", 16),
                ("metric.my-metric", 17),
                ("event.refined-event", 18),
                ("entity.refined-entity", 19),
                ("entity.refined-entity", 20),
            ];

            for err in &errors {
                if let Error::AttributeNotFound { group_id, attr_ref } = err {
                    if let Some(pos) = expected_errors
                        .iter()
                        .position(|(gid, r)| gid == group_id && r == attr_ref)
                    {
                        let _ = expected_errors.remove(pos);
                    } else {
                        panic!("Unexpected error: {err:?}");
                    }
                } else {
                    panic!("Expected AttributeNotFound, got {err:?}");
                }
            }

            assert!(
                expected_errors.is_empty(),
                "Missing expected errors: {expected_errors:?}"
            );
        } else {
            panic!("Expected FatalErr(CompoundError)");
        }
    }

    #[test]
    fn test_dependency_resolution_resolver_fatal_error() {
        let dep_url: SchemaUrl = "https://example.com/dep".try_into().unwrap();
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/root".try_into().unwrap(),
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_fatal(dep_url, weaver_resolver::Error::FailToResolveSchemaUrl {});

        let result =
            ForgeResolvedRegistry::try_from_resolved_schema(resolved_schema, &mut mock_resolver);
        assert!(result.is_fatal());
        if let WResult::FatalErr(Error::ResolverError(
            weaver_resolver::Error::FailToResolveSchemaUrl {},
        )) = result
        {
            // Expected
        } else {
            panic!("Expected ResolverError(FailToResolveSchemaUrl)");
        }
    }

    const ROOT: &str = "https://example.com/dependency-tree-root/1.0.0";
    const MIDDLE: &str = "https://example.com/dependency-tree-middle/1.0.0";
    const SUB: &str = "https://example.com/dependency-tree-sub/1.0.0";
    const LEAF: &str = "https://example.com/dependency-tree-leaf/1.0.0";
    const FORK: &str = "https://example.com/dependency-tree-fork/1.0.0";
    const BRANCH: &str = "https://example.com/dependency-tree-branch/1.0.0";
    const PUBLISHED_ROOT: &str = "https://example.com/dependency-tree-published-root/1.0.0";

    fn url(s: &str) -> SchemaUrl {
        s.try_into().expect("a valid schema url")
    }

    /// The urls of every registry depended on, sorted.
    fn dependency_urls(forge: &ForgeResolvedRegistry) -> Vec<String> {
        let mut urls: Vec<String> = forge
            .dependencies
            .keys()
            .map(SchemaUrl::to_string)
            .collect();
        urls.sort();
        urls
    }

    /// The direct dependencies recorded for `of`, or `None` when there is no
    /// entry.
    fn edges(forge: &ForgeResolvedRegistry, of: &str) -> Option<Vec<String>> {
        forge
            .dependency_graph
            .get(&url(of))
            .map(|deps| deps.iter().map(|e| e.schema_url.to_string()).collect())
    }

    /// Loads a `data/dependency_tree` registry and builds its forge registry.
    fn dependency_tree_forge(
        resolver: &mut weaver_resolver::WeaverResolver,
        path: &str,
    ) -> ForgeResolvedRegistry {
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_resolver::DefaultSchemaVisitor;
        use weaver_semconv::registry_repo::RegistryRepo;

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: path.to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create the registry repo");
        let v1 = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => {
                r.into_v1().expect("expected a v1 schema")
            }
            WResult::FatalErr(e) => panic!("failed to resolve `{path}`: {e}"),
        };
        let v2 = ResolvedTelemetrySchema::try_from(v1).expect("failed to convert to v2");
        match ForgeResolvedRegistry::try_from_resolved_schema(v2, resolver) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry for `{path}`: {e}"),
        }
    }

    /// root -> middle -> sub -> leaf.
    #[test]
    fn dependencies_and_graph_follow_the_manifests() {
        let mut resolver =
            weaver_resolver::WeaverResolver::new(weaver_resolver::WeaverResolverConfig::default());
        let forge = dependency_tree_forge(&mut resolver, "data/dependency_tree/root");

        assert_eq!(dependency_urls(&forge), vec![LEAF, MIDDLE, SUB]);
        assert_eq!(edges(&forge, ROOT), Some(vec![MIDDLE.to_owned()]));
        assert_eq!(edges(&forge, MIDDLE), Some(vec![SUB.to_owned()]));
        assert_eq!(edges(&forge, SUB), Some(vec![LEAF.to_owned()]));
        assert_eq!(edges(&forge, LEAF), Some(vec![]));
    }

    /// `leaf` sits at the bottom of a diamond, under both `sub` and `branch`.
    #[test]
    fn a_registry_reached_by_two_paths_is_materialized_once() {
        let mut resolver =
            weaver_resolver::WeaverResolver::new(weaver_resolver::WeaverResolverConfig::default());
        let forge = dependency_tree_forge(&mut resolver, "data/dependency_tree/fork");

        assert_eq!(dependency_urls(&forge), vec![BRANCH, LEAF, MIDDLE, SUB]);
        // The manifest lists middle before branch, which is not alphabetical.
        assert_eq!(
            edges(&forge, FORK),
            Some(vec![MIDDLE.to_owned(), BRANCH.to_owned()])
        );
        assert_eq!(edges(&forge, SUB), Some(vec![LEAF.to_owned()]));
        assert_eq!(edges(&forge, BRANCH), Some(vec![LEAF.to_owned()]));
    }

    /// The urls of `dependencies_nearest_first`.
    fn nearest_first(forge: &ForgeResolvedRegistry) -> Vec<String> {
        forge
            .dependencies_nearest_first()
            .into_iter()
            .map(|(url, _)| url.to_string())
            .collect()
    }

    /// The `fork` fixture declares middle before branch, and both depend on leaf.
    #[test]
    fn dependencies_nearest_first_walks_the_graph_by_distance() {
        let mut resolver =
            weaver_resolver::WeaverResolver::new(weaver_resolver::WeaverResolverConfig::default());
        let forge = dependency_tree_forge(&mut resolver, "data/dependency_tree/fork");

        assert_eq!(nearest_first(&forge), vec![MIDDLE, BRANCH, SUB, LEAF]);
    }

    #[test]
    fn dependencies_nearest_first_without_a_graph_keeps_every_dependency() {
        let (root_url, middle_url, leaf_url, mut mock_resolver) = mock_dependencies();
        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            mock_root_schema(&root_url, &middle_url, &leaf_url),
            &mut mock_resolver,
        ) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert!(forge.dependency_graph.is_empty());
        assert_eq!(
            nearest_first(&forge),
            vec![leaf_url.to_string(), middle_url.to_string()]
        );
    }

    /// The same graph, with `middle` consumed as an already-resolved artifact
    /// whose publication manifest is the only record of its dependencies.
    #[test]
    fn dependency_graph_follows_a_published_manifest() {
        use weaver_common::vdir::VirtualDirectoryPath;

        let mut resolver =
            weaver_resolver::WeaverResolver::new(weaver_resolver::WeaverResolverConfig::default());
        // The published manifest names sub by schema URL alone, so map that URL to
        // the definition files rather than fetching it.
        resolver.add_schema_url_override(
            url(SUB),
            VirtualDirectoryPath::LocalFolder {
                path: "data/dependency_tree/sub".to_owned(),
            },
        );
        let forge = dependency_tree_forge(&mut resolver, "data/dependency_tree/published/root");

        assert_eq!(dependency_urls(&forge), vec![LEAF, MIDDLE, SUB]);
        assert_eq!(edges(&forge, PUBLISHED_ROOT), Some(vec![MIDDLE.to_owned()]));
        assert_eq!(edges(&forge, MIDDLE), Some(vec![SUB.to_owned()]));
        assert_eq!(edges(&forge, SUB), Some(vec![LEAF.to_owned()]));
    }

    /// A v2 schema with no signals, for the given url and dependencies.
    fn empty_schema(url: &SchemaUrl, deps: BTreeSet<SchemaUrl>) -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: url.clone(),
            attribute_catalog: vec![],
            dependencies: deps,
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        }
    }

    /// The urls of the mock registries, and a resolver holding the two
    /// dependencies.
    fn mock_dependencies() -> (SchemaUrl, SchemaUrl, SchemaUrl, MockSchemaResolver) {
        let leaf_url = url("https://example.com/leaf/1.0.0");
        let middle_url = url("https://example.com/middle/1.0.0");
        let root_url = url("https://example.com/root/1.0.0");

        let mut middle_deps = BTreeSet::new();
        let _ = middle_deps.insert(leaf_url.clone());

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(empty_schema(&leaf_url, BTreeSet::new()));
        mock_resolver.add_v2_schema(empty_schema(&middle_url, middle_deps));
        (root_url, middle_url, leaf_url, mock_resolver)
    }

    /// The mock root schema, listing both dependencies as resolution would.
    fn mock_root_schema(
        root_url: &SchemaUrl,
        middle_url: &SchemaUrl,
        leaf_url: &SchemaUrl,
    ) -> ResolvedTelemetrySchema {
        let mut root_deps = BTreeSet::new();
        let _ = root_deps.insert(leaf_url.clone());
        let _ = root_deps.insert(middle_url.clone());
        empty_schema(root_url, root_deps)
    }

    #[test]
    fn an_unknown_dependency_graph_still_lists_every_dependency() {
        let (root_url, middle_url, leaf_url, mut mock_resolver) = mock_dependencies();
        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            mock_root_schema(&root_url, &middle_url, &leaf_url),
            &mut mock_resolver,
        ) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(forge.dependencies.len(), 2, "every dependency is listed");
        assert!(
            forge.dependency_graph.is_empty(),
            "an absent registry means unknown edges, not none"
        );
    }

    #[test]
    fn a_known_dependency_graph_is_recorded() {
        let (root_url, middle_url, leaf_url, mut mock_resolver) = mock_dependencies();
        mock_resolver.add_direct_dependencies(root_url.clone(), vec![middle_url.clone()]);
        mock_resolver.add_direct_dependencies(middle_url.clone(), vec![leaf_url.clone()]);
        mock_resolver.add_direct_dependencies(leaf_url.clone(), vec![]);

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            mock_root_schema(&root_url, &middle_url, &leaf_url),
            &mut mock_resolver,
        ) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(forge.dependencies.len(), 2);
        assert_eq!(
            forge.dependency_graph,
            BTreeMap::from([
                (root_url, vec![DependencyEdge::new(middle_url.clone())]),
                (middle_url, vec![DependencyEdge::new(leaf_url.clone())]),
                (leaf_url, vec![]),
            ])
        );
    }

    #[test]
    fn test_dependency_resolution_resolver_nfes() {
        let dep_url: SchemaUrl = "https://example.com/dep".try_into().unwrap();
        let dep_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_url.clone(),
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let root_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/root".try_into().unwrap(),
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_with_nfes(
            dep_url,
            WeaverResolvedSchema::V2(dep_schema),
            vec![weaver_resolver::Error::DeprecatedIncludeUnreferencedWarning {}],
        );

        let result =
            ForgeResolvedRegistry::try_from_resolved_schema(root_schema, &mut mock_resolver);
        if let WResult::OkWithNFEs(forge, nfes) = result {
            assert_eq!(forge.dependencies.len(), 1);
            assert_eq!(nfes.len(), 1);
        } else {
            panic!("Expected OkWithNFEs");
        }
    }

    #[test]
    fn test_dependency_recursive_fatal_error() {
        let dep_b_url: SchemaUrl = "https://example.com/dep-b".try_into().unwrap();
        // Dep B has a missing attribute error
        let dep_b_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_b_url.clone(),
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![span::Span {
                    r#type: SignalId::from("dep-b-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: Some("".to_owned()),
                        ..Default::default()
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: None,
                    }],
                    entity_associations: vec![],
                    links: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let dep_a_url: SchemaUrl = "https://example.com/dep-a".try_into().unwrap();
        let dep_a_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_a_url.clone(),
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_b_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let root_url: SchemaUrl = "https://example.com/root".try_into().unwrap();
        let root_url_for_graph = root_url.clone();
        let root_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: root_url,
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_a_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(dep_a_schema);
        mock_resolver.add_v2_schema(dep_b_schema);
        // The error is two levels down, so the graph must be known to reach it.
        mock_resolver.add_direct_dependencies(root_url_for_graph, vec![dep_a_url.clone()]);
        mock_resolver.add_direct_dependencies(dep_a_url.clone(), vec![dep_b_url.clone()]);

        let result =
            ForgeResolvedRegistry::try_from_resolved_schema(root_schema, &mut mock_resolver);
        assert!(result.is_fatal());
    }

    #[test]
    fn test_json_schema_and_serde() {
        // Test JSON schema generation
        let schema = schema_for!(ForgeResolvedRegistry);
        assert!(to_string_pretty(&schema).is_ok());

        let registry_schema = schema_for!(Registry);
        assert!(to_string_pretty(&registry_schema).is_ok());

        let refinements_schema = schema_for!(Refinements);
        assert!(to_string_pretty(&refinements_schema).is_ok());

        // Test serde roundtrip with empty registry
        let empty_forge = ForgeResolvedRegistry {
            schema_url: "https://example.com/schema".try_into().unwrap(),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: BTreeMap::new(),
            dependency_graph: BTreeMap::new(),
        };

        let json_val = serde_json::to_value(&empty_forge).expect("serialization should succeed");
        assert!(json_val.get("dependencies").is_none());
        assert!(json_val.get("dependency_graph").is_none());
        assert_conforms_to_json_schema(&empty_forge);

        let round_trip: ForgeResolvedRegistry =
            serde_json::from_value(json_val).expect("deserialization should succeed");
        assert_eq!(round_trip, empty_forge);

        // Test deny_unknown_fields on ForgeResolvedRegistry
        let invalid_json = serde_json::json!({
            "schema_url": "https://example.com/schema",
            "registry": {
                "attributes": [],
                "attribute_groups": [],
                "metrics": [],
                "spans": [],
                "events": [],
                "entities": []
            },
            "refinements": {
                "metrics": [],
                "spans": [],
                "events": [],
                "entities": []
            },
            "unknown_field": "unexpected"
        });
        let result: Result<ForgeResolvedRegistry, _> = serde_json::from_value(invalid_json);
        assert!(result.is_err());

        // Test deny_unknown_fields on Registry
        let invalid_registry_json = serde_json::json!({
            "attributes": [],
            "attribute_groups": [],
            "metrics": [],
            "spans": [],
            "events": [],
            "entities": [],
            "unknown_field": "unexpected"
        });
        let result: Result<Registry, _> = serde_json::from_value(invalid_registry_json);
        assert!(result.is_err());

        // Test deny_unknown_fields on Refinements
        let invalid_refinements_json = serde_json::json!({
            "metrics": [],
            "spans": [],
            "events": [],
            "entities": [],
            "unknown_field": "unexpected"
        });
        let result: Result<Refinements, _> = serde_json::from_value(invalid_refinements_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_resolved_schema_recursive_dependencies_and_serialization() {
        let dep_b_url: SchemaUrl = "https://example.com/dep-b".try_into().unwrap();
        let dep_b_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_b_url.clone(),
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let dep_a_url: SchemaUrl = "https://example.com/dep-a".try_into().unwrap();
        let dep_a_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: dep_a_url.clone(),
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_b_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let root_url: SchemaUrl = "https://example.com/root".try_into().unwrap();
        let root_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: root_url.clone(),
            attribute_catalog: vec![],
            dependencies: {
                let mut deps = BTreeSet::new();
                let _ = deps.insert(dep_a_url.clone());
                deps
            },
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(dep_a_schema);
        mock_resolver.add_v2_schema(dep_b_schema);
        // root -> dep-a -> dep-b, as the manifests would declare it.
        mock_resolver.add_direct_dependencies(root_url.clone(), vec![dep_a_url.clone()]);
        mock_resolver.add_direct_dependencies(dep_a_url.clone(), vec![dep_b_url.clone()]);

        let forge_registry = match ForgeResolvedRegistry::try_from_resolved_schema(
            root_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) => r,
            WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert_eq!(forge_registry.schema_url, root_url);
        // The root's own list names dep-a only; dep-b is reached through it.
        assert_eq!(
            forge_registry.dependencies.keys().collect::<Vec<_>>(),
            vec![&dep_a_url, &dep_b_url]
        );
        assert_eq!(
            forge_registry.dependency_graph,
            BTreeMap::from([
                (
                    root_url.clone(),
                    vec![DependencyEdge::new(dep_a_url.clone())]
                ),
                (
                    dep_a_url.clone(),
                    vec![DependencyEdge::new(dep_b_url.clone())]
                ),
            ])
        );

        // Test serde serialization round-trip
        let json_val = serde_json::to_value(&forge_registry).expect("Failed to serialize to JSON");
        assert!(json_val.get("dependencies").is_some());
        let round_trip: ForgeResolvedRegistry =
            serde_json::from_value(json_val).expect("Failed to deserialize from JSON");
        assert_eq!(round_trip, forge_registry);
    }

    /// An entity with no attributes, for the lookup tests.
    fn test_entity(entity_type: &str) -> Entity {
        Entity {
            r#type: entity_type.to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Provenance::default(),
        }
    }

    /// One entity per name, and a refinement of each under the same id.
    fn test_signals(entities: &[&str]) -> (Registry, Refinements) {
        (
            Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: entities.iter().map(|t| test_entity(t)).collect(),
            },
            Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: entities
                    .iter()
                    .map(|t| EntityRefinement {
                        id: (*t).to_owned().into(),
                        entity: test_entity(t),
                    })
                    .collect(),
            },
        )
    }

    /// A dependency defining `entities`.
    fn test_dependency(entities: &[&str]) -> ForgeDependency {
        let (registry, refinements) = test_signals(entities);
        ForgeDependency {
            registry,
            refinements,
        }
    }

    /// The direct and the transitive dependency url, and the registry that
    /// depends on both. `middle` is what depends on `base`.
    fn lookup_fixture() -> (SchemaUrl, SchemaUrl, ForgeResolvedRegistry) {
        let main_url: SchemaUrl = "https://example.com/main/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let middle_url: SchemaUrl = "https://example.com/middle/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let base_url: SchemaUrl = "https://example.com/base/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let (registry, refinements) = test_signals(&["service"]);
        let main = ForgeResolvedRegistry {
            schema_url: main_url.clone(),
            registry,
            refinements,
            dependencies: BTreeMap::from([
                (middle_url.clone(), test_dependency(&["deployment"])),
                (base_url.clone(), test_dependency(&["host"])),
            ]),
            dependency_graph: BTreeMap::from([
                (main_url, vec![DependencyEdge::new(middle_url.clone())]),
                (
                    middle_url.clone(),
                    vec![DependencyEdge::new(base_url.clone())],
                ),
            ]),
        };
        (middle_url, base_url, main)
    }

    /// An empty provenance names an entity of this registry.
    #[test]
    fn test_lookup_entity_local() {
        let (_, _, main) = lookup_fixture();
        let found = main
            .lookup_entity(&EntityRef::local("service".to_owned().into()))
            .expect("the local entity");
        assert_eq!(found.r#type, "service".to_owned().into());
    }

    /// A refinement id is a name like any other, so the refinement list answers
    /// for an entity the registry list does not hold.
    #[test]
    fn test_lookup_entity_refinement() {
        let (_, _, mut main) = lookup_fixture();
        main.refinements.entities.push(EntityRefinement {
            id: "service.linux".to_owned().into(),
            entity: test_entity("service"),
        });
        let found = main
            .lookup_entity(&EntityRef::local("service.linux".to_owned().into()))
            .expect("the refinement");
        assert_eq!(found.r#type, "service".to_owned().into());
    }

    /// A reference into a direct dependency reads that dependency.
    #[test]
    fn test_lookup_entity_from_dependency() {
        let (middle_url, _, main) = lookup_fixture();
        let found = main
            .lookup_entity(&EntityRef {
                r#type: "deployment".to_owned().into(),
                provenance: Provenance {
                    source: Some(middle_url),
                    path: None,
                },
            })
            .expect("the entity of the dependency");
        assert_eq!(found.r#type, "deployment".to_owned().into());
    }

    /// `base` is reached only through `middle`.
    #[test]
    fn test_lookup_entity_from_transitive_dependency() {
        let (_, base_url, main) = lookup_fixture();
        let found = main
            .lookup_entity(&EntityRef {
                r#type: "host".to_owned().into(),
                provenance: Provenance {
                    source: Some(base_url),
                    path: None,
                },
            })
            .expect("the entity of the transitive dependency");
        assert_eq!(found.r#type, "host".to_owned().into());
    }

    /// A name no registry defines names the reference in the error.
    #[test]
    fn test_lookup_entity_not_defined() {
        let (_, _, main) = lookup_fixture();
        let error = main
            .lookup_entity(&EntityRef::local("nothing".to_owned().into()))
            .expect_err("no such entity");
        assert!(
            matches!(&error, Error::EntityNotFound { entity_type, registry }
                if entity_type == "nothing" && registry.is_none()),
            "unexpected error: {error:?}"
        );
    }

    /// A reference into a registry that this one does not depend on names that
    /// registry in the error.
    #[test]
    fn test_lookup_entity_unknown_registry() {
        let (_, _, main) = lookup_fixture();
        let stranger: SchemaUrl = "https://example.com/stranger/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let error = main
            .lookup_entity(&EntityRef {
                r#type: "host".to_owned().into(),
                provenance: Provenance {
                    source: Some(stranger.clone()),
                    path: None,
                },
            })
            .expect_err("not a dependency");
        assert!(
            matches!(&error, Error::EntityNotFound { entity_type, registry }
                if entity_type == "host" && registry.as_deref() == Some(stranger.as_str())),
            "unexpected error: {error:?}"
        );
    }

    /// An entity a dependency holds is not copied into this registry, so a
    /// reference that lost its provenance would find nothing.
    #[test]
    fn test_lookup_entity_of_dependency_is_not_local() {
        let (_, _, main) = lookup_fixture();
        assert!(main
            .lookup_entity(&EntityRef::local("host".to_owned().into()))
            .is_err());
    }

    /// End to end: the conversion turns the dependency index of a leaf into the
    /// schema url of that dependency, and the helper reads the definition back.
    #[test]
    fn test_lookup_entity_after_conversion() {
        let base_url: SchemaUrl = "https://example.com/base/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let host = entity::Entity {
            r#type: "host".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        };
        let base_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: base_url.clone(),
            attribute_catalog: vec![],
            dependencies: BTreeSet::new(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![host.clone()],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![entity::EntityRefinement {
                    id: "host".to_owned().into(),
                    entity: host,
                }],
            },
        };

        // The span names the `host` of `base`, which this registry does not hold.
        let root_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/root/1.0.0"
                .try_into()
                .expect("a valid schema url"),
            attribute_catalog: vec![],
            dependencies: [base_url.clone()].into_iter().collect(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![span::Span {
                    links: vec![],
                    r#type: "my-span".to_owned().into(),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: Some("My Span".to_owned()),
                        ..Default::default()
                    },
                    attributes: vec![],
                    entity_associations: vec![entity::EntityAssociation::Ref(entity::EntityRef {
                        r#type: "host".to_owned().into(),
                        provenance: provenance::Provenance {
                            source: Some(provenance::DependencyRef(0)),
                            path: String::new(),
                        },
                    })],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(base_schema);
        let forge_registry = match ForgeResolvedRegistry::try_from_resolved_schema(
            root_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) => r,
            WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        let leaf = match &forge_registry.registry.spans[0].entity_associations[0] {
            EntityAssociation::Ref(entity_ref) => entity_ref,
            other => panic!("expected a reference, got {other:?}"),
        };
        assert_eq!(leaf.provenance.source.as_ref(), Some(&base_url));
        let found = forge_registry
            .lookup_entity(leaf)
            .expect("the entity of the dependency");
        assert_eq!(found.r#type, "host".to_owned().into());
    }

    #[test]
    fn test_lookup_entity_fallback_transitive_absent_from_cache() {
        let main_url: SchemaUrl = "https://example.com/main/1.0.0".try_into().unwrap();
        let direct_url: SchemaUrl = "https://example.com/direct/1.0.0".try_into().unwrap();
        let absent_transitive_url: SchemaUrl =
            "https://example.com/absent/1.0.0".try_into().unwrap();

        let mut direct_dep = test_dependency(&["host", "local_only"]);
        direct_dep.registry.entities[0].provenance.source = Some(absent_transitive_url.clone());
        direct_dep.refinements.entities[0].entity.provenance.source =
            Some(absent_transitive_url.clone());

        let main = ForgeResolvedRegistry {
            schema_url: main_url.clone(),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: BTreeMap::from([(direct_url.clone(), direct_dep)]),
            dependency_graph: BTreeMap::from([
                (main_url, vec![DependencyEdge::new(direct_url.clone())]),
                (
                    direct_url.clone(),
                    vec![DependencyEdge::new(absent_transitive_url.clone())],
                ),
            ]),
        };

        let entity_ref = EntityRef {
            r#type: "host".to_owned().into(),
            provenance: Provenance {
                source: Some(absent_transitive_url.clone()),
                path: None,
            },
        };

        let found = main
            .lookup_entity(&entity_ref)
            .expect("entity with matching provenance should be found via fallback");
        assert_eq!(found.r#type, "host".to_owned().into());

        // An entity defined locally in `direct_dep` (provenance.source == None) must not
        // satisfy a reference targeting `absent_transitive_url`.
        let wrong_provenance_ref = EntityRef {
            r#type: "local_only".to_owned().into(),
            provenance: Provenance {
                source: Some(absent_transitive_url.clone()),
                path: None,
            },
        };
        let err = main
            .lookup_entity(&wrong_provenance_ref)
            .expect_err("entity with mismatched provenance must not be returned");
        assert!(
            matches!(err, Error::EntityNotFound { entity_type, .. } if entity_type == "local_only")
        );

        let unknown_ref = EntityRef {
            r#type: "nonexistent".to_owned().into(),
            provenance: Provenance {
                source: Some(absent_transitive_url.clone()),
                path: None,
            },
        };
        let err = main
            .lookup_entity(&unknown_ref)
            .expect_err("nonexistent entity should not be found");
        assert!(
            matches!(err, Error::EntityNotFound { entity_type, .. } if entity_type == "nonexistent")
        );
    }

    #[test]
    fn test_lookup_entity_fallback_published_dep_e2e() {
        let root_url: SchemaUrl = "https://example.com/root/1.0.0".try_into().unwrap();
        let b_url: SchemaUrl = "https://example.com/b/1.0.0".try_into().unwrap();
        let c_url: SchemaUrl = "https://example.com/c/1.0.0".try_into().unwrap();

        let host_entity = entity::Entity {
            r#type: "host".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: provenance::Provenance {
                source: Some(provenance::DependencyRef(0)),
                path: String::new(),
            },
        };

        let b_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: b_url.clone(),
            attribute_catalog: vec![],
            dependencies: [c_url.clone()].into_iter().collect(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![host_entity.clone()],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![entity::EntityRefinement {
                    id: "host".to_owned().into(),
                    entity: host_entity,
                }],
            },
        };

        let root_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: root_url.clone(),
            attribute_catalog: vec![],
            dependencies: [b_url.clone(), c_url.clone()].into_iter().collect(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![span::Span {
                    links: vec![],
                    r#type: "root-span".to_owned().into(),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: Some("Root Span".to_owned()),
                        ..Default::default()
                    },
                    attributes: vec![],
                    entity_associations: vec![entity::EntityAssociation::Ref(entity::EntityRef {
                        r#type: "host".to_owned().into(),
                        provenance: provenance::Provenance {
                            source: Some(provenance::DependencyRef(1)),
                            path: String::new(),
                        },
                    })],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                }],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(b_schema);
        mock_resolver.add_fatal(
            c_url.clone(),
            weaver_resolver::Error::SchemaNotInCache {
                schema_url: c_url.clone(),
            },
        );
        mock_resolver.add_direct_dependencies(root_url.clone(), vec![b_url.clone()]);
        mock_resolver.add_direct_dependencies(b_url.clone(), vec![c_url.clone()]);

        let (forge_registry, nfes) = match ForgeResolvedRegistry::try_from_resolved_schema(
            root_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) => (r, vec![]),
            WResult::OkWithNFEs(r, nfes) => (r, nfes),
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert!(
            nfes.iter().any(|e| matches!(
                e,
                Error::MissingDependencyWarning { schema_url } if schema_url == c_url.as_str()
            )),
            "expected MissingDependencyWarning for uncached transitive dep, got {nfes:?}"
        );

        assert!(!forge_registry.dependencies.contains_key(&c_url));
        assert!(forge_registry.dependencies.contains_key(&b_url));
        assert_eq!(
            forge_registry.dependency_graph.get(&b_url),
            Some(&vec![DependencyEdge::new(c_url.clone())])
        );

        let leaf = match &forge_registry.registry.spans[0].entity_associations[0] {
            EntityAssociation::Ref(entity_ref) => entity_ref,
            other => panic!("expected a reference, got {other:?}"),
        };
        assert_eq!(leaf.provenance.source.as_ref(), Some(&c_url));

        let found = forge_registry
            .lookup_entity(leaf)
            .expect("entity should be resolved via fallback in published dependency b");
        assert_eq!(found.r#type, "host".to_owned().into());
    }

    #[test]
    fn test_materialize_sorts_attribute_groups() {
        let schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: "https://example.com/root/1.0.0"
                .try_into()
                .expect("a valid schema url"),
            attribute_catalog: vec![],
            dependencies: Default::default(),
            registry: v2::registry::Registry {
                attributes: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![
                    attribute_group::AttributeGroup {
                        id: "z_group".to_owned().into(),
                        attributes: vec![],
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    attribute_group::AttributeGroup {
                        id: "a_group".to_owned().into(),
                        attributes: vec![],
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
            },
            refinements: refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
        };

        let mut mock_resolver = MockSchemaResolver::new();
        let forge_registry =
            match ForgeResolvedRegistry::try_from_resolved_schema(schema, &mut mock_resolver) {
                WResult::Ok(r) => r,
                WResult::OkWithNFEs(r, _) => r,
                WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
            };

        assert_eq!(forge_registry.registry.attribute_groups.len(), 2);
        assert_eq!(
            forge_registry.registry.attribute_groups[0].id,
            "a_group".into()
        );
        assert_eq!(
            forge_registry.registry.attribute_groups[1].id,
            "z_group".into()
        );
    }

    #[test]
    fn test_dependency_graph_records_upgraded_edges_and_supports_lookups() {
        let main_url = url("https://example.com/main/1.0.0");
        let a_url = url("https://example.com/a/1.0.0");
        let b_url = url("https://example.com/b/1.0.0");
        let c_100 = url("https://example.com/c/1.0.0");
        let c_110 = url("https://example.com/c/1.1.0");
        let c_120 = url("https://example.com/c/1.2.0");

        let c_120_entity = entity::Entity {
            r#type: "host".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            common: CommonFields {
                brief: "host from c 1.2.0".to_owned(),
                ..Default::default()
            },
            provenance: provenance::Provenance::default(),
        };
        let mut c_120_schema = empty_schema(&c_120, BTreeSet::new());
        c_120_schema.registry.entities.push(c_120_entity);

        let a_schema = empty_schema(&a_url, [c_110.clone()].into_iter().collect());
        let b_schema = empty_schema(&b_url, [c_120.clone()].into_iter().collect());
        let main_schema = empty_schema(
            &main_url,
            [a_url.clone(), b_url.clone(), c_120.clone()]
                .into_iter()
                .collect(),
        );

        let mut mock_resolver = MockSchemaResolver::new();
        // Even if superseded versions remain in cache, only the chosen c@1.2.0 should be materialized.
        mock_resolver.add_v2_schema(empty_schema(&c_100, BTreeSet::new()));
        mock_resolver.add_v2_schema(empty_schema(&c_110, BTreeSet::new()));
        mock_resolver.add_v2_schema(c_120_schema);
        mock_resolver.add_v2_schema(a_schema);
        mock_resolver.add_v2_schema(b_schema);
        mock_resolver.add_direct_dependencies(
            main_url.clone(),
            vec![a_url.clone(), b_url.clone(), c_100.clone()],
        );
        mock_resolver.add_direct_dependencies(a_url.clone(), vec![c_110.clone()]);
        mock_resolver.add_direct_dependencies(b_url.clone(), vec![c_120.clone()]);
        mock_resolver.add_direct_dependencies(c_120.clone(), vec![]);

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            main_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert_eq!(
            forge.dependencies.keys().cloned().collect::<Vec<_>>(),
            vec![a_url.clone(), b_url.clone(), c_120.clone()]
        );

        assert_eq!(
            forge.dependency_graph,
            BTreeMap::from([
                (
                    main_url.clone(),
                    vec![
                        DependencyEdge::new(a_url.clone()),
                        DependencyEdge::new(b_url.clone()),
                        DependencyEdge::upgraded(c_120.clone(), c_100.clone()),
                    ],
                ),
                (
                    a_url.clone(),
                    vec![DependencyEdge::upgraded(c_120.clone(), c_110.clone())],
                ),
                (b_url.clone(), vec![DependencyEdge::new(c_120.clone())]),
                (c_120.clone(), vec![]),
            ])
        );

        // Nearest-first traversal follows resolved edges without dangling on c@1.0.0.
        assert_eq!(
            nearest_first(&forge),
            vec![a_url.to_string(), b_url.to_string(), c_120.to_string()]
        );

        // Dependency and entity lookups work with both chosen and pre-upgrade declared URLs.
        assert_eq!(forge.resolve_dependency_url(&c_120), &c_120);
        assert_eq!(forge.resolve_dependency_url(&c_110), &c_120);
        assert_eq!(forge.resolve_dependency_url(&c_100), &c_120);
        assert!(forge.lookup_dependency(&c_120).is_some());
        assert!(forge.lookup_dependency(&c_110).is_some());
        assert!(forge.lookup_dependency(&c_100).is_some());

        for source in [&c_120, &c_110, &c_100] {
            let entity_ref = EntityRef {
                r#type: "host".to_owned().into(),
                provenance: Provenance {
                    source: Some(source.clone()),
                    path: None,
                },
            };
            let found = forge
                .lookup_entity(&entity_ref)
                .expect("entity should resolve through chosen or declared URL");
            assert_eq!(found.common.brief, "host from c 1.2.0");
        }

        // Verify JSON serialization omits `upgraded_from` on non-upgraded edges and includes it on upgraded ones.
        let json_val = serde_json::to_value(&forge).expect("serialize");
        let main_edges = json_val["dependency_graph"][main_url.as_str()]
            .as_array()
            .expect("main edges array");
        assert!(main_edges[0].get("upgraded_from").is_none());
        assert_eq!(main_edges[2]["schema_url"], c_120.as_str());
        assert_eq!(main_edges[2]["upgraded_from"], c_100.as_str());

        let round_trip: ForgeResolvedRegistry =
            serde_json::from_value(json_val).expect("deserialize");
        assert_eq!(round_trip, forge);
    }

    #[test]
    fn test_dependency_graph_records_cross_scheme_upgrade() {
        let main_url = url("https://example.com/main/1.0.0");
        let a_url = url("https://example.com/a/1.0.0");
        let c_https = url("https://example.com/c/1.0.0");
        let c_http = url("http://example.com/c/1.1.0");

        let main_schema = empty_schema(
            &main_url,
            [a_url.clone(), c_http.clone()].into_iter().collect(),
        );
        let a_schema = empty_schema(&a_url, [c_http.clone()].into_iter().collect());

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(a_schema);
        mock_resolver.add_v2_schema(empty_schema(&c_http, BTreeSet::new()));
        mock_resolver
            .add_direct_dependencies(main_url.clone(), vec![a_url.clone(), c_https.clone()]);
        mock_resolver.add_direct_dependencies(a_url.clone(), vec![c_http.clone()]);
        mock_resolver.add_direct_dependencies(c_http.clone(), vec![]);

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            main_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert_eq!(
            forge.dependency_graph.get(&main_url),
            Some(&vec![
                DependencyEdge::new(a_url),
                DependencyEdge::upgraded(c_http.clone(), c_https.clone()),
            ])
        );
        assert_eq!(forge.resolve_dependency_url(&c_https), &c_http);
    }
}
