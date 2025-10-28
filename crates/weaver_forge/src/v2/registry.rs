//! Version two of registry specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_resolved_schema::attribute::AttributeRef;

use crate::{
    error::Error,
    v2::{
        attribute::Attribute,
        entity::{Entity, EntityAttribute},
        event::{Event, EventAttribute, EventRefinement},
        metric::{Metric, MetricAttribute, MetricRefinement},
        span::{Span, SpanAttribute, SpanRefinement},
    },
};

/// A resolved semantic convention registry used in the context of the template and policy
/// engines.
///
/// This includes all registrys fully fleshed out and ready for codegen.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRegistry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    /// The raw attributes in this registry.
    pub attributes: Vec<Attribute>,
    /// The signals defined in this registry.
    pub signals: Signals,
    /// The set of refinments defined in this registry.
    pub refinements: Refinements,
}

/// The set of all defined signals for a given semantic convention registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Signals {
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

impl ResolvedRegistry {
    /// Create a new template registry from a resolved schema registry.
    pub fn try_from_resolved_schema(
        schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
    ) -> Result<Self, Error> {
        let mut errors = Vec::new();
        let mut attributes: Vec<Attribute> = schema
            .registry
            .attributes
            .iter()
            .map(|a| Attribute {
                key: a.key.clone(),
                r#type: a.r#type.clone(),
                examples: a.examples.clone(),
                common: a.common.clone(),
            })
            .collect();
        attributes.sort_by(|l, r| l.key.cmp(&r.key));

        let mut metrics = Vec::new();
        for metric in schema.registry.metrics {
            let attributes = metric
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = schema
                        .registry
                        .attribute(&ar.base)
                        .map(|a| MetricAttribute {
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
                    let attr = schema
                        .registry
                        .attribute(&ar.base)
                        .map(|a| MetricAttribute {
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
                    let attr = schema.registry.attribute(&ar.base).map(|a| SpanAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                        },
                        requirement_level: ar.requirement_level.clone(),
                        sampling_relevant: ar.sampling_relevant.clone(),
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
                    let attr = schema.registry.attribute(&ar.base).map(|a| SpanAttribute {
                        base: Attribute {
                            key: a.key.clone(),
                            r#type: a.r#type.clone(),
                            examples: a.examples.clone(),
                            common: a.common.clone(),
                        },
                        requirement_level: ar.requirement_level.clone(),
                        sampling_relevant: ar.sampling_relevant.clone(),
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
                    let attr = schema.registry.attribute(&ar.base).map(|a| EventAttribute {
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
                attributes: attributes,
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
                    let attr = schema.registry.attribute(&ar.base).map(|a| EventAttribute {
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
                    attributes: attributes,
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
                    let attr = schema
                        .registry
                        .attribute(&ar.base)
                        .map(|a| EntityAttribute {
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
                    let attr = schema
                        .registry
                        .attribute(&ar.base)
                        .map(|a| EntityAttribute {
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

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            registry_url: schema.schema_url.clone(),
            attributes,
            signals: Signals {
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
