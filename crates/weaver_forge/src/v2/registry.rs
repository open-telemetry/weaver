//! Version two of registry specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_resolved_schema::{attribute::AttributeRef, v2::catalog::AttributeCatalog};
use weaver_semconv::schema_url::SchemaUrl;

use crate::{
    error::Error,
    v2::{
        attribute::Attribute,
        attribute_group::AttributeGroup,
        entity::{Entity, EntityAttribute},
        event::{Event, EventAttribute, EventRefinement},
        metric::{Metric, MetricAttribute, MetricRefinement},
        span::{Span, SpanAttribute, SpanRefinement},
    },
};

/// The file format version for the V2 materialized registry files.
pub const V2_MATERIALIZED_FILE_FORMAT: &str = "materialized/2.0.0";

/// A resolved semantic convention registry used in the context of the template and policy
/// engines.
///
/// This includes all registrys fully fleshed out and ready for codegen.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForgeResolvedRegistry {
    /// Version of the file structure.
    pub file_format: String,
    /// The semantic convention registry url.
    pub schema_url: SchemaUrl,
    // TODO - Attribute Groups
    /// The signals defined in this registry.
    pub registry: Registry,
    /// The set of refinments defined in this registry.
    pub refinements: Refinements,
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
}

/// Conversion from Resolved schema to the "template schema".
impl TryFrom<weaver_resolved_schema::v2::ResolvedTelemetrySchema> for ForgeResolvedRegistry {
    type Error = Error;
    fn try_from(
        value: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
    ) -> Result<Self, Self::Error> {
        ForgeResolvedRegistry::try_from_resolved_schema(value)
    }
}

impl ForgeResolvedRegistry {
    /// Create a new template registry from a resolved schema registry.
    pub fn try_from_resolved_schema(
        schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
    ) -> Result<Self, Error> {
        let mut errors = Vec::new();

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
                entity_associations: metric.entity_associations,
                common: metric.common,
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
                    entity_associations: metric.metric.entity_associations,
                    common: metric.metric.common,
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
                entity_associations: span.entity_associations,
                common: span.common,
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
                    entity_associations: span.span.entity_associations,
                    common: span.span.common,
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
                entity_associations: event.entity_associations,
                common: event.common,
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
                    entity_associations: event.event.entity_associations,
                    common: event.event.common,
                },
            });
        }
        event_refinements.sort_by(|l, r| l.id.cmp(&r.id));

        let mut entities = Vec::new();
        for e in schema.registry.entities {
            let identity = e
                .identity
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| EntityAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("entity.{}", &e.r#type),
                            attr_ref: AttributeRef(ar.base.0),
                        });
                    }
                    attr
                })
                .collect();

            let description = e
                .description
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(&ar.base).map(|a| EntityAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                        },
                        requirement_level: ar.requirement_level.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("entity.{}", &e.r#type),
                            attr_ref: AttributeRef(ar.base.0),
                        });
                    }
                    attr
                })
                .collect();
            entities.push(Entity {
                r#type: e.r#type,
                identity,
                description,
                common: e.common,
            });
        }
        entities.sort_by(|l, r| l.r#type.cmp(&r.r#type));

        let mut attribute_groups = Vec::new();
        for ag in schema.registry.attribute_groups {
            let attributes = ag
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = attribute_lookup(ar).map(|a| Attribute {
                        key: a.key.clone(),
                        r#type: a.r#type.clone(),
                        examples: a.examples.clone(),
                        common: a.common.clone(),
                    });
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: format!("attribute_group.{}", &ag.id),
                            attr_ref: AttributeRef(ar.0),
                        });
                    }
                    attr
                })
                .collect();
            attribute_groups.push(AttributeGroup {
                id: ag.id,
                attributes,
                common: ag.common.clone(),
            });
        }

        // Now we sort the attributes, since we aren't looking them up anymore.
        attributes.sort_by(|l, r| l.key.cmp(&r.key));

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            file_format: V2_MATERIALIZED_FILE_FORMAT.to_owned(),
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
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use weaver_resolved_schema::v2::{
        attribute, event, metric, span, ResolvedTelemetrySchema, {self},
    };
    use weaver_semconv::{
        attribute::{AttributeType, PrimitiveOrArrayTypeSpec},
        group::{InstrumentSpec, SpanKindSpec},
        v2::{signal_id::SignalId, span::SpanName, CommonFields},
    };

    use super::*;

    #[test]
    fn test_try_from_resolved_schema() {
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: SchemaUrl::try_new("https://example.com/schema".to_owned()).unwrap(),
            attribute_catalog: vec![attribute::Attribute {
                key: "test.attr".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields::default(),
            }],
            registry: v2::registry::Registry {
                attributes: vec![attribute::AttributeRef(0)],
                spans: vec![span::Span {
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "My Span".to_owned(),
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: attribute::AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: Some(true),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                metrics: vec![metric::Metric {
                    name: SignalId::from("my-metric".to_owned()),
                    instrument: InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![metric::MetricAttributeRef {
                        base: attribute::AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                events: vec![event::Event {
                    name: SignalId::from("my-event".to_owned()),
                    attributes: vec![event::EventAttributeRef {
                        base: attribute::AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                entities: vec![v2::entity::Entity {
                    r#type: SignalId::from("my-entity".to_owned()),
                    identity: vec![v2::entity::EntityAttributeRef {
                        base: attribute::AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    description: vec![],
                    common: CommonFields::default(),
                }],
                attribute_groups: vec![],
            },
            refinements: v2::refinements::Refinements {
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
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: Some(false),
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
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
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                            ),
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
                    },
                }],
                events: vec![event::EventRefinement {
                    id: SignalId::from("my-refined-event".to_owned()),
                    event: event::Event {
                        name: SignalId::from("my-event".to_owned()),
                        attributes: vec![event::EventAttributeRef {
                            base: attribute::AttributeRef(0),
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::OptIn,
                            ),
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
                    },
                }],
            },
        };

        let forge_registry =
            ForgeResolvedRegistry::try_from(resolved_schema).expect("Conversion failed");

        assert_eq!(forge_registry.registry.attributes.len(), 1);
        assert_eq!(forge_registry.registry.spans.len(), 1);
        assert_eq!(forge_registry.registry.metrics.len(), 1);
        assert_eq!(forge_registry.registry.events.len(), 1);
        assert_eq!(forge_registry.registry.entities.len(), 1);
        assert_eq!(forge_registry.refinements.spans.len(), 1);
        assert_eq!(forge_registry.refinements.metrics.len(), 1);
        assert_eq!(forge_registry.refinements.events.len(), 1);

        let span = &forge_registry.registry.spans[0];
        assert_eq!(span.r#type, "my-span".to_owned().into());
        assert_eq!(span.attributes.len(), 1);
        assert_eq!(span.attributes[0].base.key, "test.attr");

        let entity = &forge_registry.registry.entities[0];
        assert_eq!(entity.r#type, "my-entity".to_owned().into());
        assert_eq!(entity.identity.len(), 1);
        assert_eq!(entity.identity[0].base.key, "test.attr");

        let refined_span = &forge_registry.refinements.spans[0];
        assert_eq!(refined_span.id, "my-refined-span".to_owned().into());
        assert_eq!(refined_span.span.r#type, "my-span".to_owned().into());
        assert_eq!(refined_span.span.attributes.len(), 1);
        assert_eq!(refined_span.span.attributes[0].base.key, "test.attr");

        let refined_metric = &forge_registry.refinements.metrics[0];
        assert_eq!(refined_metric.id, "my-refined-metric".to_owned().into());
        assert_eq!(refined_metric.metric.name, "my-metric".to_owned().into());
        assert_eq!(refined_metric.metric.attributes.len(), 1);
        assert_eq!(refined_metric.metric.attributes[0].base.key, "test.attr");

        let refined_event = &forge_registry.refinements.events[0];
        assert_eq!(refined_event.id, "my-refined-event".to_owned().into());
        assert_eq!(refined_event.event.name, "my-event".to_owned().into());
        assert_eq!(refined_event.event.attributes.len(), 1);
        assert_eq!(refined_event.event.attributes[0].base.key, "test.attr");
    }

    // This should never happen, but we want a test where "try_from" fails, so we
    // purposely construct a bad registry in case of a logic bug further up in the stack.
    #[test]
    fn test_try_from_resolved_schema_with_missing_attribute() {
        let resolved_schema = ResolvedTelemetrySchema {
            file_format: "2.0.0".to_owned(),
            schema_url: SchemaUrl::try_new("https://example.com/schema".to_owned()).unwrap(),
            attribute_catalog: vec![],
            registry: v2::registry::Registry {
                attributes: vec![], // No attributes - This is the logic bug.
                spans: vec![span::Span {
                    r#type: SignalId::from("my-span".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "My Span".to_owned(),
                    },
                    attributes: vec![span::SpanAttributeRef {
                        base: attribute::AttributeRef(0), // Refers to bad attribute.
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: Some(true),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                metrics: vec![],
                events: vec![],
                entities: vec![],
                attribute_groups: vec![],
            },
            refinements: v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
            },
        };

        let result = ForgeResolvedRegistry::try_from(resolved_schema);
        assert!(result.is_err());

        if let Err(Error::CompoundError(errors)) = result {
            assert_eq!(errors.len(), 1);
            if let Some(Error::AttributeNotFound { group_id, attr_ref }) = errors.first() {
                assert_eq!(group_id, "span.my-span");
                assert_eq!(*attr_ref, AttributeRef(0));
            } else {
                panic!("Expected AttributeNotFound error");
            }
        } else {
            panic!("Expected CompoundError");
        }
    }
}
