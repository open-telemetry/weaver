//! A semantic convention registry.

use std::collections::{BTreeMap, HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::attribute::AttributeType;

use crate::v2::{
    attribute::{Attribute, AttributeRef},
    attribute_group::AttributeGroup,
    entity::Entity,
    event::Event,
    metric::Metric,
    span::Span,
    stats::{
        AttributeGroupStats, AttributeStats, CommonSignalStats, EntityStats, EventStats,
        MetricStats, RegistryStats, SpanStats,
    },
};

/// A semantic convention registry.
///
/// The semantic convention is composed of definitions of
/// attributes, metrics, logs, etc. that will be sent over the wire (e.g. OTLP).
///
/// Note: The registry does not include signal refinements.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Catalog of attributes used in the schema.
    pub attributes: Vec<Attribute>,

    /// Catalog of (public) attribute groups.
    pub attribute_groups: Vec<AttributeGroup>,

    /// The semantic convention registry url.
    ///
    /// This is the base URL, under which this registry can be found.
    pub registry_url: String,

    /// A  list of span signal definitions.
    pub spans: Vec<Span>,

    /// A  list of metric signal definitions.
    pub metrics: Vec<Metric>,

    /// A  list of event signal definitions.
    pub events: Vec<Event>,

    /// A  list of entity signal definitions.
    pub entities: Vec<Entity>,
}

impl Registry {
    /// Returns the attribute from an attribute ref if it exists.
    #[must_use]
    pub fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.attributes.get(attribute_ref.0 as usize)
    }
    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    #[must_use]
    pub fn attribute_key(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.key.as_ref())
    }

    /// Returns the statistics for this registry.
    #[must_use] 
    pub fn stats(&self) -> RegistryStats {
        let attributes = {
            let mut attribute_type_breakdown = BTreeMap::new();
            let mut stability_breakdown = HashMap::new();
            let mut deprecated_count = 0;
            for attribute in &self.attributes {
                let attribute_type = if let AttributeType::Enum { members, .. } = &attribute.r#type
                {
                    format!("enum(card:{:03})", members.len())
                } else {
                    format!("{:#}", &attribute.r#type)
                };
                if attribute.common.deprecated.is_some() {
                    deprecated_count += 1;
                }
                *attribute_type_breakdown
                    .entry(attribute_type)
                    .or_insert(0_usize) += 1;
                *stability_breakdown
                    .entry(attribute.common.stability.clone())
                    .or_default() += 1;
            }
            AttributeStats {
                attribute_count: self.attributes.len(),
                attribute_type_breakdown,
                stability_breakdown,
                deprecated_count,
            }
        };

        let metrics = {
            let mut stability_breakdown = HashMap::new();
            let mut deprecated_count = 0;
            let mut total_with_note = 0;
            let mut metric_names = HashSet::new();
            let mut instrument_breakdown = HashMap::new();
            let mut unit_breakdown = HashMap::new();
            for metric in &self.metrics {
                if metric.common.deprecated.is_some() {
                    deprecated_count += 1;
                }
                if !metric.common.note.is_empty() {
                    total_with_note += 1;
                }
                let _ = metric_names.insert(metric.name.to_string());
                *instrument_breakdown
                    .entry(metric.instrument.clone())
                    .or_insert(0) += 1;
                *unit_breakdown.entry(metric.unit.clone()).or_insert(0) += 1;
                *stability_breakdown
                    .entry(metric.common.stability.clone())
                    .or_default() += 1;
            }

            MetricStats {
                common: CommonSignalStats {
                    count: self.metrics.len(),
                    stability_breakdown,
                    deprecated_count,
                    total_with_note,
                },
                metric_names,
                instrument_breakdown,
                unit_breakdown,
            }
        };

        let spans = {
            let mut span_kind_breakdown = HashMap::new();
            let mut stability_breakdown = HashMap::new();
            let mut deprecated_count = 0;
            let mut total_with_note = 0;
            for span in &self.spans {
                if span.common.deprecated.is_some() {
                    deprecated_count += 1;
                }
                if !span.common.note.is_empty() {
                    total_with_note += 1;
                }
                *span_kind_breakdown.entry(span.kind.clone()).or_default() += 1;
                *stability_breakdown
                    .entry(span.common.stability.clone())
                    .or_default() += 1;
            }
            SpanStats {
                common: CommonSignalStats {
                    count: self.spans.len(),
                    stability_breakdown,
                    deprecated_count,
                    total_with_note,
                },
                span_kind_breakdown,
            }
        };

        let events = {
            let mut event_names = HashSet::new();
            let mut stability_breakdown = HashMap::new();
            let mut deprecated_count = 0;
            let mut total_with_note = 0;
            for event in &self.events {
                if event.common.deprecated.is_some() {
                    deprecated_count += 1;
                }
                if !event.common.note.is_empty() {
                    total_with_note += 1;
                }
                let _ = event_names.insert(event.name.to_string());
                *stability_breakdown
                    .entry(event.common.stability.clone())
                    .or_default() += 1;
            }
            EventStats {
                common: CommonSignalStats {
                    count: self.events.len(),
                    stability_breakdown,
                    deprecated_count,
                    total_with_note,
                },
                event_names,
            }
        };

        let entities = {
            let mut entity_types = HashSet::new();
            let mut entity_identity_length_distribution = HashMap::new();
            let mut stability_breakdown = HashMap::new();
            let mut deprecated_count = 0;
            let mut total_with_note = 0;
            for entity in &self.entities {
                if entity.common.deprecated.is_some() {
                    deprecated_count += 1;
                }
                if !entity.common.note.is_empty() {
                    total_with_note += 1;
                }
                *stability_breakdown
                    .entry(entity.common.stability.clone())
                    .or_default() += 1;
                let _ = entity_types.insert(entity.r#type.to_string());
                *entity_identity_length_distribution
                    .entry(entity.identity.len())
                    .or_insert(0) += 1;
            }
            EntityStats {
                common: CommonSignalStats {
                    count: self.entities.len(),
                    stability_breakdown,
                    deprecated_count,
                    total_with_note,
                },
                entity_types,
                entity_identity_length_distribution,
            }
        };

        let attribute_groups = {
            AttributeGroupStats {
                common: CommonSignalStats {
                    count: self.attribute_groups.len(),
                    stability_breakdown: HashMap::new(),
                    deprecated_count: 0,
                    total_with_note: 0,
                },
            }
        };
        RegistryStats {
            attributes,
            metrics,
            spans,
            events,
            entities,
            attribute_groups,
        }
    }
}

#[cfg(test)]
mod test {
    use weaver_semconv::{
        group::{InstrumentSpec, SpanKindSpec},
        stability::Stability,
        v2::{span::SpanName, CommonFields},
    };

    use crate::v2::entity::EntityAttributeRef;

    use super::*;

    #[test]
    fn test_stats() {
        let registry = Registry {
            attribute_groups: vec![],
            registry_url: "https://opentelemetry.io/schemas/1.23.0".to_owned(),
            spans: vec![Span {
                r#type: "test.span".to_owned().into(),
                kind: SpanKindSpec::Client,
                name: SpanName {
                    note: "test".to_owned(),
                },
                attributes: vec![],
                entity_associations: vec![],
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            }],
            metrics: vec![Metric {
                name: "test.metric".to_owned().into(),
                instrument: InstrumentSpec::Counter,
                unit: "{tests}".to_owned(),
                attributes: vec![],
                entity_associations: vec![],
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            }],
            events: vec![],
            entities: vec![Entity {
                r#type: "test.entity".to_owned().into(),
                identity: vec![EntityAttributeRef {
                    base: AttributeRef(0),
                    requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                        weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                    ),
                }],
                description: vec![],
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            }],
            attributes: vec![Attribute {
                key: "key".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            }],
        };
        let stats = registry.stats();
        assert_eq!(stats.attributes.attribute_count, 1);
        assert_eq!(
            stats.attributes.attribute_type_breakdown.get("string"),
            Some(&1)
        );

        assert_eq!(stats.entities.common.count, 1);
        assert_eq!(stats.entities.entity_types.len(), 1);
        assert_eq!(
            stats.entities.entity_identity_length_distribution.get(&1),
            Some(&1)
        );
        assert_eq!(
            stats.entities.entity_identity_length_distribution.get(&0),
            None
        );

        assert_eq!(stats.metrics.common.count, 1);
        assert_eq!(
            stats
                .metrics
                .common
                .stability_breakdown
                .get(&Stability::Stable),
            Some(&1)
        );
        assert_eq!(stats.metrics.common.deprecated_count, 0);
        assert_eq!(stats.metrics.common.total_with_note, 0);
        assert_eq!(stats.metrics.metric_names.len(), 1);
        assert_eq!(
            stats
                .metrics
                .instrument_breakdown
                .get(&InstrumentSpec::Counter),
            Some(&1)
        );
        assert_eq!(stats.metrics.unit_breakdown.get("{tests}"), Some(&1));

        assert_eq!(stats.spans.common.count, 1);
        assert_eq!(
            stats
                .spans
                .common
                .stability_breakdown
                .get(&Stability::Stable),
            Some(&1)
        );
        assert_eq!(stats.spans.common.deprecated_count, 0);
        assert_eq!(stats.spans.common.total_with_note, 0);
        assert_eq!(
            stats.spans.span_kind_breakdown.get(&SpanKindSpec::Client),
            Some(&1)
        );

        assert_eq!(stats.events.common.count, 0);
        assert_eq!(
            stats
                .events
                .common
                .stability_breakdown
                .get(&Stability::Stable),
            None
        );
        assert_eq!(stats.events.common.deprecated_count, 0);
        assert_eq!(stats.events.common.total_with_note, 0);
        assert_eq!(stats.events.event_names.len(), 0);
    }
}
