//! Version two of registry specification.

use crate::v2::{attribute_group::AttributeGroupAttribute, provenance::Provenance};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_common::result::WResult;
use weaver_resolved_schema::{
    attribute::AttributeRef,
    v2::{catalog::AttributeCatalog, entity::EntityAttributeRef},
};
use weaver_resolver::SchemaResolver;
use weaver_semconv::schema_url::SchemaUrl;

use crate::{
    error::Error,
    v2::{
        attribute::Attribute,
        attribute_group::AttributeGroup,
        entity::{
            from_resolved_associations, Entity, EntityAttribute, EntityRef, EntityRefinement,
        },
        event::{Event, EventAttribute, EventRefinement},
        metric::{Metric, MetricAttribute, MetricRefinement},
        span::{Span, SpanAttribute, SpanRefinement},
    },
};

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
    /// The registries this one depends on directly, as its manifest declares them,
    /// each carrying its own dependencies in turn.
    ///
    /// Where the manifest is unknown, every direct and transitive dependency is
    /// listed here instead, with no nesting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ForgeResolvedRegistry>,
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

impl ForgeResolvedRegistry {
    /// Returns the entity definition that an association leaf names.
    ///
    /// A registry does not copy the entities of its dependencies, so an
    /// association often names a registry other than this one.
    pub fn lookup_entity(&self, entity_ref: &EntityRef) -> Result<&Entity, Error> {
        let registry = match &entity_ref.provenance.source {
            // Empty provenance: this registry defines it.
            None => self,
            // A leaf may name a transitive dependency, which sits below a
            // direct one in the tree.
            Some(url) => self
                .find_dependency(url)
                .ok_or_else(|| Error::EntityNotFound {
                    entity_type: entity_ref.r#type.to_string(),
                    registry: Some(url.to_string()),
                })?,
        };
        registry
            .defined_entity(&entity_ref.r#type)
            .ok_or_else(|| Error::EntityNotFound {
                entity_type: entity_ref.r#type.to_string(),
                registry: entity_ref.provenance.source.as_ref().map(|u| u.to_string()),
            })
    }

    /// Finds a registry in the dependency tree by schema URL, at any depth.
    fn find_dependency(&self, schema_url: &SchemaUrl) -> Option<&ForgeResolvedRegistry> {
        self.dependencies.iter().find_map(|dep| {
            if &dep.schema_url == schema_url {
                Some(dep)
            } else {
                dep.find_dependency(schema_url)
            }
        })
    }

    /// The entity this registry defines under `name`.
    ///
    /// An association names an entity type or the id of an entity refinement,
    /// which share one namespace, so both lists answer.
    fn defined_entity(&self, name: &str) -> Option<&Entity> {
        self.registry
            .entities
            .iter()
            .find(|entity| &*entity.r#type == name)
            .or_else(|| {
                self.refinements
                    .entities
                    .iter()
                    .find(|refinement| &*refinement.id == name)
                    .map(|refinement| &refinement.entity)
            })
    }

    /// Create a new template registry from a resolved schema registry, resolving
    /// all dependencies via the provided schema resolver.
    ///
    /// Note: Dependencies SHOULD be in cache, following normal resolution.
    pub fn try_from_resolved_schema<R: SchemaResolver>(
        schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
        resolver: &mut R,
    ) -> WResult<Self, Error> {
        Self::build(schema, resolver, true)
    }

    /// Builds the registry, nesting each dependency's own dependencies when
    /// `expand` is set and leaving them empty when it is not.
    fn build<R: SchemaResolver>(
        schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
        resolver: &mut R,
        expand: bool,
    ) -> WResult<Self, Error> {
        let mut errors = Vec::new();

        let deps_list: Vec<_> = schema.dependencies.iter().cloned().collect();
        let resolve_provenance = |prov: &weaver_resolved_schema::v2::provenance::Provenance| {
            Provenance::from_resolved(prov, &deps_list)
        };
        let resolve_associations =
            |assocs: &[weaver_resolved_schema::v2::entity::EntityAssociation]| {
                from_resolved_associations(assocs, &deps_list)
            };

        let attribute_lookup = |r: &weaver_resolved_schema::v2::attribute::AttributeRef| {
            schema.attribute_catalog.attribute(r)
        };
        // We create an attribute lookup map.
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
                            group_id: format!("metric.{}", &metric.name),
                            attr_ref: AttributeRef(ar.base.0),
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
                            group_id: format!("metric.{}", &metric.metric.name),
                            attr_ref: AttributeRef(ar.base.0),
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
                            group_id: format!("span.{}", &span.r#type),
                            attr_ref: AttributeRef(ar.base.0),
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
                            group_id: format!("span.{}", &span.id),
                            attr_ref: AttributeRef(ar.base.0),
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
                            group_id: format!("event.{}", &event.name),
                            attr_ref: AttributeRef(ar.base.0),
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

        // convert event refinements.
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
                            group_id: format!("event.{}", &event.id),
                            attr_ref: AttributeRef(ar.base.0),
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
                            attr_ref: AttributeRef(ar.base.0),
                        });
                    }
                    attr
                })
                .collect()
        };

        let mut entities = Vec::new();
        for e in schema.registry.entities {
            let group_id = format!("entity.{}", &e.r#type);
            let identity = convert_entity_attrs(&e.identity, &group_id, &mut errors);
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
            let group_id = format!("entity.{}", &e.id);
            let identity = convert_entity_attrs(&e.entity.identity, &group_id, &mut errors);
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
                            group_id: format!("attribute_group.{}", &ag.id),
                            attr_ref: AttributeRef(ar.base.0),
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

        // Now we sort the attributes, since we aren't looking them up anymore.
        attributes.sort_by(|l, r| l.key.cmp(&r.key));

        if !errors.is_empty() {
            return WResult::FatalErr(Error::CompoundError(errors));
        }

        let mut non_fatal_errors = Vec::new();
        let mut dependencies = Vec::new();
        // Without the direct dependencies the shape is unknown, so list every
        // dependency at this level and nest none of them.
        let (dep_urls, expand_children) = match resolver.direct_dependencies(&schema.schema_url) {
            _ if !expand => (Vec::new(), false),
            Some(direct) => (direct.to_vec(), true),
            None => (schema.dependencies.iter().cloned().collect(), false),
        };
        for dep_url in &dep_urls {
            let (resolved_bundle, dep_nfes) = match resolver.resolve_schema(dep_url) {
                WResult::Ok(bundle) => (bundle, vec![]),
                WResult::OkWithNFEs(bundle, nfes) => (bundle, nfes),
                WResult::FatalErr(e) => return WResult::FatalErr(e.into()),
            };

            for nfe in dep_nfes {
                non_fatal_errors.push(Error::from(nfe));
            }

            let dep_v2_schema = match &*resolved_bundle {
                weaver_resolver::WeaverResolvedSchema::V2(v2) => v2.clone(),
                weaver_resolver::WeaverResolvedSchema::V1(v1) => {
                    match weaver_resolved_schema::v2::ResolvedTelemetrySchema::try_from(v1.clone())
                    {
                        Ok(v2) => v2,
                        Err(e) => return WResult::FatalErr(Error::from(e)),
                    }
                }
            };

            let dep_forge = match Self::build(dep_v2_schema, resolver, expand_children) {
                WResult::Ok(forge) => forge,
                WResult::OkWithNFEs(forge, nfes) => {
                    non_fatal_errors.extend(nfes);
                    forge
                }
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            };

            dependencies.push(dep_forge);
        }

        let forge_registry = Self {
            schema_url: schema.schema_url.clone(),
            registry: Registry {
                attributes,
                attribute_groups,
                metrics,
                spans,
                events,
                entities,
            },
            refinements: Refinements {
                metrics: metric_refinements,
                spans: span_refinements,
                events: event_refinements,
                entities: entity_refinements,
            },
            dependencies,
        };

        if non_fatal_errors.is_empty() {
            WResult::Ok(forge_registry)
        } else {
            WResult::OkWithNFEs(forge_registry, non_fatal_errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::sync::Arc;

    use crate::v2::entity::EntityAssociation;
    use schemars::schema_for;
    use serde_json::to_string_pretty;
    use weaver_resolved_schema::attribute::AttributeRef;
    use weaver_resolved_schema::v2::{
        attribute, attribute_group, entity, event, metric, provenance, refinements, span,
        ResolvedTelemetrySchema, {self},
    };
    use weaver_resolver::NullSchemaResolver;
    use weaver_semconv::{
        attribute::{
            AttributeType, BasicRequirementLevelSpec, Examples, PrimitiveOrArrayTypeSpec,
            RequirementLevel,
        },
        group::{InstrumentSpec, SpanKindSpec},
        schema_url::SchemaUrl,
        signal_requirement_level::SignalRequirementLevel,
        stability::Stability,
        v2::{signal_id::SignalId, span::SpanName, CommonFields},
    };

    use super::*;

    enum MockResolution {
        Ok(Arc<weaver_resolver::WeaverResolvedSchema>),
        OkWithNFEs(
            Arc<weaver_resolver::WeaverResolvedSchema>,
            Vec<weaver_resolver::Error>,
        ),
        Fatal(weaver_resolver::Error),
    }

    struct MockSchemaResolver {
        schemas: HashMap<SchemaUrl, MockResolution>,
        direct_dependencies: HashMap<SchemaUrl, Vec<SchemaUrl>>,
    }

    impl MockSchemaResolver {
        fn new() -> Self {
            Self {
                schemas: HashMap::new(),
                direct_dependencies: HashMap::new(),
            }
        }

        fn add_v2_schema(&mut self, schema: ResolvedTelemetrySchema) {
            let _ = self.schemas.insert(
                schema.schema_url.clone(),
                MockResolution::Ok(Arc::new(weaver_resolver::WeaverResolvedSchema::V2(schema))),
            );
        }

        fn add_v1_schema(
            &mut self,
            url: SchemaUrl,
            schema: weaver_resolved_schema::ResolvedTelemetrySchema,
        ) {
            let _ = self.schemas.insert(
                url,
                MockResolution::Ok(Arc::new(weaver_resolver::WeaverResolvedSchema::V1(schema))),
            );
        }

        fn add_with_nfes(
            &mut self,
            url: SchemaUrl,
            schema: weaver_resolver::WeaverResolvedSchema,
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
        ) -> WResult<Arc<weaver_resolver::WeaverResolvedSchema>, weaver_resolver::Error> {
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
                        note: "Note description".to_owned(),
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
                attributes: vec![attribute::AttributeRef(0), attribute::AttributeRef(1)],
                spans: vec![span::Span {
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "My Span".to_owned(),
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: attribute::AttributeRef(0),
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
                        base: attribute::AttributeRef(0),
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
                        base: attribute::AttributeRef(0),
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
                    identity: vec![EntityAttributeRef {
                        base: attribute::AttributeRef(0),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    description: vec![EntityAttributeRef {
                        base: attribute::AttributeRef(1),
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
                        base: attribute::AttributeRef(0),
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
                            note: "My Refined Span".to_owned(),
                        },
                        attributes: vec![span::SpanAttributeRef {
                            base: attribute::AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: Some(false),
                        }],
                        entity_associations: vec![],
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
                            base: attribute::AttributeRef(0),
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
                            base: attribute::AttributeRef(0),
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
                        identity: vec![EntityAttributeRef {
                            base: attribute::AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        description: vec![EntityAttributeRef {
                            base: attribute::AttributeRef(1),
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

        assert_eq!(forge_registry.dependencies.len(), 1);
        assert_eq!(forge_registry.dependencies[0].schema_url, dep_url);

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
        assert_eq!(entity.identity[0].base.key, "test.attr");
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
        assert_eq!(refined_entity.entity.identity[0].base.key, "test.attr");
        assert_eq!(refined_entity.entity.description.len(), 1);
        assert_eq!(
            refined_entity.entity.description[0].base.key,
            "test.desc.attr"
        );
    }

    fn forge_refinements_len(forge: &ForgeResolvedRegistry) -> (usize, usize, usize, usize) {
        (
            forge.refinements.spans.len(),
            forge.refinements.metrics.len(),
            forge.refinements.events.len(),
            forge.refinements.entities.len(),
        )
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
                attributes: vec![
                    attribute::AttributeRef(0),
                    attribute::AttributeRef(1),
                    attribute::AttributeRef(2),
                ],
                spans: vec![
                    span::Span {
                        r#type: SignalId::from("z-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "".to_owned(),
                        },
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    span::Span {
                        r#type: SignalId::from("a-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "".to_owned(),
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
                            r#type: SignalId::from("z-span".to_owned()),
                            kind: SpanKindSpec::Internal,
                            name: SpanName {
                                note: "".to_owned(),
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
                            r#type: SignalId::from("a-span".to_owned()),
                            kind: SpanKindSpec::Internal,
                            name: SpanName {
                                note: "".to_owned(),
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
                attributes: vec![attribute::AttributeRef(0), attribute::AttributeRef(1)],
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
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "".to_owned(),
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: attribute::AttributeRef(10),
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
                        base: attribute::AttributeRef(11),
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
                        base: attribute::AttributeRef(12),
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
                    identity: vec![EntityAttributeRef {
                        base: attribute::AttributeRef(13),
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    description: vec![EntityAttributeRef {
                        base: attribute::AttributeRef(14),
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
                        base: attribute::AttributeRef(15),
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
                        r#type: SignalId::from("my-span".to_owned()),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "".to_owned(),
                        },
                        attributes: vec![span::SpanAttributeRef {
                            base: attribute::AttributeRef(16),
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
                            base: attribute::AttributeRef(17),
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
                            base: attribute::AttributeRef(18),
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
                        identity: vec![EntityAttributeRef {
                            base: attribute::AttributeRef(19),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        description: vec![EntityAttributeRef {
                            base: attribute::AttributeRef(20),
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
                ("span.my-span", AttributeRef(10)),
                ("metric.my-metric", AttributeRef(11)),
                ("event.my-event", AttributeRef(12)),
                ("entity.my-entity", AttributeRef(13)),
                ("entity.my-entity", AttributeRef(14)),
                ("attribute_group.my-group", AttributeRef(15)),
                ("span.refined-span", AttributeRef(16)),
                ("metric.my-metric", AttributeRef(17)),
                ("event.refined-event", AttributeRef(18)),
                ("entity.refined-entity", AttributeRef(19)),
                ("entity.refined-entity", AttributeRef(20)),
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

    /// The chain the dependency tree tests expect, nearest first.
    const EXPECTED_CHAIN: &[&str] = &[
        "https://example.com/dependency-tree-middle/1.0.0",
        "https://example.com/dependency-tree-sub/1.0.0",
        "https://example.com/dependency-tree-leaf/1.0.0",
    ];

    /// Walks the dependencies of `registry`, failing if any level holds more than one.
    fn chain(registry: &ForgeResolvedRegistry) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = registry;
        while !current.dependencies.is_empty() {
            assert_eq!(
                current.dependencies.len(),
                1,
                "{} should declare exactly one dependency, found {:?}",
                current.schema_url,
                current
                    .dependencies
                    .iter()
                    .map(|dep| dep.schema_url.to_string())
                    .collect::<Vec<_>>()
            );
            current = &current.dependencies[0];
            out.push(current.schema_url.to_string());
        }
        out
    }

    /// The resolved schema of `root` lists `leaf`, which only `middle` depends on.
    #[test]
    fn dependency_tree_follows_the_manifests() {
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_resolver::{DefaultSchemaVisitor, WeaverResolver, WeaverResolverConfig};
        use weaver_semconv::registry_repo::RegistryRepo;

        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/dependency_tree/root".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create the registry repo");
        let v1 = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => {
                r.into_v1().expect("expected a v1 schema")
            }
            WResult::FatalErr(e) => panic!("failed to resolve the root registry: {e}"),
        };

        assert_eq!(
            v1.dependencies.len(),
            3,
            "the resolved schema lists middle, sub and leaf alike"
        );

        let v2 = ResolvedTelemetrySchema::try_from(v1).expect("failed to convert to v2");
        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(v2, &mut resolver) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(
            chain(&forge),
            EXPECTED_CHAIN,
            "root -> middle -> sub -> leaf"
        );
    }

    /// `leaf` appears under both arms: a tree holds a copy per path, not a shared node.
    #[test]
    fn a_registry_with_two_dependencies_keeps_both_arms() {
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_resolver::{DefaultSchemaVisitor, WeaverResolver, WeaverResolverConfig};
        use weaver_semconv::registry_repo::RegistryRepo;

        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/dependency_tree/fork".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create the registry repo");
        let v1 = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => {
                r.into_v1().expect("expected a v1 schema")
            }
            WResult::FatalErr(e) => panic!("failed to resolve the fork registry: {e}"),
        };
        let v2 = ResolvedTelemetrySchema::try_from(v1).expect("failed to convert to v2");
        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(v2, &mut resolver) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        let names = |registry: &ForgeResolvedRegistry| -> Vec<String> {
            registry
                .dependencies
                .iter()
                .map(|dep| dep.schema_url.name().to_owned())
                .collect()
        };

        // The manifest lists middle before branch, which is not alphabetical.
        assert_eq!(
            names(&forge),
            vec![
                "example.com/dependency-tree-middle".to_owned(),
                "example.com/dependency-tree-branch".to_owned()
            ]
        );
        assert_eq!(chain(&forge.dependencies[0]), EXPECTED_CHAIN[1..].to_vec());
        assert_eq!(
            names(&forge.dependencies[1]),
            vec!["example.com/dependency-tree-leaf".to_owned()],
            "the second arm reaches leaf directly"
        );
    }

    /// The same graph, with `middle` consumed as an already-resolved artifact whose
    /// publication manifest is the only record of what it depends on.
    #[test]
    fn dependency_tree_follows_a_published_manifest() {
        use weaver_common::vdir::VirtualDirectoryPath;
        use weaver_resolver::{DefaultSchemaVisitor, WeaverResolver, WeaverResolverConfig};
        use weaver_semconv::registry_repo::RegistryRepo;

        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        // The published manifest names sub by schema URL alone, so map that URL to
        // the definition files rather than fetching it.
        resolver.add_schema_url_override(
            "https://example.com/dependency-tree-sub/1.0.0"
                .try_into()
                .expect("not a valid schema url"),
            VirtualDirectoryPath::LocalFolder {
                path: "data/dependency_tree/sub".to_owned(),
            },
        );

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/dependency_tree/published/root".to_owned(),
        };
        let repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create the registry repo");
        let v1 = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => {
                r.into_v1().expect("expected a v1 schema")
            }
            WResult::FatalErr(e) => panic!("failed to resolve the root registry: {e}"),
        };
        let v2 = ResolvedTelemetrySchema::try_from(v1).expect("failed to convert to v2");
        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(v2, &mut resolver) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(
            chain(&forge),
            EXPECTED_CHAIN,
            "root -> middle -> sub -> leaf"
        );
    }

    /// An unknown graph is listed flat, so no registry is given children it may not have.
    #[test]
    fn an_unknown_dependency_graph_is_listed_flat() {
        let leaf_url: SchemaUrl = "https://example.com/leaf/1.0.0".try_into().unwrap();
        let middle_url: SchemaUrl = "https://example.com/middle/1.0.0".try_into().unwrap();
        let root_url: SchemaUrl = "https://example.com/root/1.0.0".try_into().unwrap();

        let empty_schema = |url: &SchemaUrl, deps: BTreeSet<SchemaUrl>| ResolvedTelemetrySchema {
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
        };

        let mut middle_deps = BTreeSet::new();
        let _ = middle_deps.insert(leaf_url.clone());
        let mut root_deps = BTreeSet::new();
        let _ = root_deps.insert(leaf_url.clone());
        let _ = root_deps.insert(middle_url.clone());

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(empty_schema(&leaf_url, BTreeSet::new()));
        mock_resolver.add_v2_schema(empty_schema(&middle_url, middle_deps));

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            empty_schema(&root_url, root_deps),
            &mut mock_resolver,
        ) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(forge.dependencies.len(), 2, "every dependency is listed");
        assert!(
            forge
                .dependencies
                .iter()
                .all(|dep| dep.dependencies.is_empty()),
            "a flat listing must not nest, or middle would carry a second copy of leaf"
        );
    }

    /// With the graph known, the same dependencies become a tree.
    #[test]
    fn a_known_dependency_graph_is_nested() {
        let leaf_url: SchemaUrl = "https://example.com/leaf/1.0.0".try_into().unwrap();
        let middle_url: SchemaUrl = "https://example.com/middle/1.0.0".try_into().unwrap();
        let root_url: SchemaUrl = "https://example.com/root/1.0.0".try_into().unwrap();

        let empty_schema = |url: &SchemaUrl, deps: BTreeSet<SchemaUrl>| ResolvedTelemetrySchema {
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
        };

        let mut middle_deps = BTreeSet::new();
        let _ = middle_deps.insert(leaf_url.clone());
        let mut root_deps = BTreeSet::new();
        let _ = root_deps.insert(leaf_url.clone());
        let _ = root_deps.insert(middle_url.clone());

        let mut mock_resolver = MockSchemaResolver::new();
        mock_resolver.add_v2_schema(empty_schema(&leaf_url, BTreeSet::new()));
        mock_resolver.add_v2_schema(empty_schema(&middle_url, middle_deps));
        mock_resolver.add_direct_dependencies(root_url.clone(), vec![middle_url.clone()]);
        mock_resolver.add_direct_dependencies(middle_url.clone(), vec![leaf_url.clone()]);
        mock_resolver.add_direct_dependencies(leaf_url.clone(), vec![]);

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            empty_schema(&root_url, root_deps),
            &mut mock_resolver,
        ) {
            WResult::Ok(f) | WResult::OkWithNFEs(f, _) => f,
            WResult::FatalErr(e) => panic!("failed to build the forge registry: {e}"),
        };

        assert_eq!(forge.dependencies.len(), 1);
        assert_eq!(forge.dependencies[0].schema_url, middle_url);
        assert_eq!(forge.dependencies[0].dependencies.len(), 1);
        assert_eq!(forge.dependencies[0].dependencies[0].schema_url, leaf_url);
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
            weaver_resolver::WeaverResolvedSchema::V2(dep_schema),
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
    fn test_dependency_resolution_v1_schema_success() {
        let dep_url: SchemaUrl = "https://example.com/dep-v1".try_into().unwrap();
        let v1_schema = weaver_resolved_schema::ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/dep-v1".to_owned(),
            registry_id: "test".to_owned(),
            registry: weaver_resolved_schema::registry::Registry {
                registry_url: "https://example.com/dep-v1".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: BTreeSet::new(),
            versions: None,
            registry_manifest: None,
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
        mock_resolver.add_v1_schema(dep_url.clone(), v1_schema);

        let forge = match ForgeResolvedRegistry::try_from_resolved_schema(
            root_schema,
            &mut mock_resolver,
        ) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Conversion failed: {e:?}"),
        };

        assert_eq!(forge.dependencies.len(), 1);
        assert_eq!(forge.dependencies[0].schema_url, dep_url);
    }

    #[test]
    fn test_dependency_resolution_v1_schema_conversion_error() {
        let dep_url: SchemaUrl = "https://example.com/dep-v1".try_into().unwrap();
        let invalid_v1_schema = weaver_resolved_schema::ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "invalid schema url with spaces".to_owned(),
            registry_id: "test".to_owned(),
            registry: weaver_resolved_schema::registry::Registry {
                registry_url: "invalid schema url with spaces".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: BTreeSet::new(),
            versions: None,
            registry_manifest: None,
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
        mock_resolver.add_v1_schema(dep_url, invalid_v1_schema);

        let result =
            ForgeResolvedRegistry::try_from_resolved_schema(root_schema, &mut mock_resolver);
        assert!(result.is_fatal());
        if let WResult::FatalErr(Error::SchemaError(_)) = result {
            // Expected
        } else {
            panic!("Expected FatalErr(SchemaError)");
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
                        note: "".to_owned(),
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: attribute::AttributeRef(0),
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
            dependencies: vec![],
        };

        let json_val = serde_json::to_value(&empty_forge).expect("serialization should succeed");
        assert!(json_val.get("dependencies").is_none()); // skip_serializing_if = "Vec::is_empty"

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
        assert_eq!(forge_registry.dependencies.len(), 1);
        assert_eq!(forge_registry.dependencies[0].schema_url, dep_a_url);
        assert_eq!(forge_registry.dependencies[0].dependencies.len(), 1);
        assert_eq!(
            forge_registry.dependencies[0].dependencies[0].schema_url,
            dep_b_url
        );
        assert!(forge_registry.dependencies[0].dependencies[0]
            .dependencies
            .is_empty());

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

    /// A registry that defines `entities`, a base refinement of each, and holds
    /// `dependencies`.
    fn test_registry(
        url: &str,
        entities: &[&str],
        dependencies: Vec<ForgeResolvedRegistry>,
    ) -> ForgeResolvedRegistry {
        ForgeResolvedRegistry {
            schema_url: url.try_into().expect("a valid schema url"),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: entities.iter().map(|t| test_entity(t)).collect(),
            },
            refinements: Refinements {
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
            dependencies,
        }
    }

    /// The registry a dependency-sourced reference points at, and the tree that
    /// holds it. The dependency list of a resolved schema is the whole closure,
    /// so `base` is a child of `main` even though `middle` is what depends on it.
    fn lookup_fixture() -> (SchemaUrl, SchemaUrl, ForgeResolvedRegistry) {
        let middle_url: SchemaUrl = "https://example.com/middle/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let base_url: SchemaUrl = "https://example.com/base/1.0.0"
            .try_into()
            .expect("a valid schema url");
        let base = test_registry(base_url.as_str(), &["host"], vec![]);
        let middle = test_registry(middle_url.as_str(), &["deployment"], vec![base.clone()]);
        let main = test_registry(
            "https://example.com/main/1.0.0",
            &["service"],
            vec![middle, base],
        );
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

    /// A dependency of a dependency is in the closure, so it needs no walk.
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
                    r#type: "my-span".to_owned().into(),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "My Span".to_owned(),
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
}
