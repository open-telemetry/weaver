//! Version two of registry specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_resolved_schema::attribute::AttributeRef;

use crate::{
    error::Error,
    v2::{
        attribute::Attribute,
        metric::{Metric, MetricAttribute, MetricRefinement}, span::{Span, SpanAttribute, SpanRefinement},
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
    // TODO - Attribute registry?
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
}

impl ResolvedRegistry {
    /// Create a new template registry from a resolved schema registry.
    pub fn try_from_resolved_schema(
        schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema,
    ) -> Result<Self, Error> {
        let mut errors = Vec::new();
        let mut metrics = Vec::new();
        for metric in schema.registry.metrics {
            let attributes = metric
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = schema.catalog.attribute(&ar.base).map(|a| MetricAttribute {
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

        let mut metric_refinements: Vec<MetricRefinement> = Vec::new();
        for metric in schema.registry.metric_refinements {
            let attributes = metric
                .metric
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = schema.catalog.attribute(&ar.base).map(|a| MetricAttribute {
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

        let mut spans = Vec::new();
        for span in schema.registry.spans {
            let attributes = span
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = schema.catalog.attribute(&ar.base).map(|a| SpanAttribute {
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
        let mut span_refinements = Vec::new();
        for span in schema.registry.span_refinements {
            let attributes = span
                .span
                .attributes
                .iter()
                .filter_map(|ar| {
                    let attr = schema.catalog.attribute(&ar.base).map(|a| SpanAttribute {
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
            span_refinements.push(
                SpanRefinement { 
                    id: span.id, 
                    span: Span {
                r#type: span.span.r#type,
                kind: span.span.kind,
                name: span.span.name,
                attributes,
                entity_associations: span.span.entity_associations,
                common: span.span.common,
            }
                });
        }
        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            registry_url: schema.schema_url.clone(),
            signals: Signals { metrics, spans },
            refinements: Refinements {
                metrics: metric_refinements,
                spans: span_refinements,
            },
        })
    }
}
