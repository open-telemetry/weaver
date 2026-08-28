//! A semantic convention refinements.

use crate::v2::{
    entity::EntityRefinement, event::EventRefinement, metric::MetricRefinement,
    span::SpanRefinement, stats::RefinementStats,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Semantic convention refinements.
///
/// Refinements are a specialization of a signal that can be used to optimise documentation,
/// or code generation. A refinement will *always* match the conventions defined by the
/// signal it refines. Refinements cannot be inferred from signals over the wire (e.g. OTLP).
/// This is because any identifying feature of a refinement is used purely for codegen but has
/// no storage location in OTLP.
///
/// Note: Refinements will always include a "base" refinement for every signal definition.
///       For example, if a Metric signal named `my_metric` is defined, there will be
///       a metric refinement named `my_metric` as well.
///       This allows code generation to *only* interact with refinements, if desired, to
///       provide optimised methods for generating telemetry signals.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Refinements {
    /// A list of span refinements.
    pub spans: Vec<SpanRefinement>,

    /// A list of metric refinements.
    pub metrics: Vec<MetricRefinement>,

    /// A list of event refinements.
    pub events: Vec<EventRefinement>,

    /// A list of entity refinements.
    #[serde(default)]
    pub entities: Vec<EntityRefinement>,
}

impl Refinements {
    /// Refinement statistics.
    #[must_use]
    pub fn stats(&self) -> RefinementStats {
        // TODO - implement.
        RefinementStats {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_semconv::v2::CommonFields;

    #[test]
    fn test_refinements_serde() {
        let refinements = Refinements {
            spans: vec![SpanRefinement {
                id: "http.client.request".to_owned().into(),
                span: crate::v2::span::Span {
                    r#type: "http.client".to_owned().into(),
                    kind: weaver_semconv::v2::span::SpanKindSpec::Client,
                    name: weaver_semconv::v2::span::SpanName {
                        note: "HTTP GET".to_owned(),
                    },
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
            }],
            metrics: vec![MetricRefinement {
                id: "http.server.duration".to_owned().into(),
                metric: crate::v2::metric::Metric {
                    name: "http.server.duration".to_owned().into(),
                    instrument: weaver_semconv::v2::metric::InstrumentSpec::Histogram,
                    unit: "ms".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
            }],
            events: vec![EventRefinement {
                id: "exception".to_owned().into(),
                event: crate::v2::event::Event {
                    name: "exception".to_owned().into(),
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
            }],
            entities: vec![EntityRefinement {
                id: "k8s.pod".to_owned().into(),
                entity: crate::v2::entity::Entity {
                    r#type: "k8s.pod".to_owned().into(),
                    identity: vec![],
                    description: vec![],
                    requirement_level: None,
                    common: CommonFields::default(),
                    provenance: Default::default(),
                },
            }],
        };
        let json = serde_json::to_string(&refinements).expect("Serialization should succeed");
        let deserialized: Refinements =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert_eq!(refinements, deserialized);
        assert_eq!(refinements.spans.len(), 1);
        assert_eq!(refinements.metrics.len(), 1);
        assert_eq!(refinements.events.len(), 1);
        assert_eq!(refinements.entities.len(), 1);
    }
}
