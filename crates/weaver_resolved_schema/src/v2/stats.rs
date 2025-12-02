//! Version 2 of semantic convention schema stats

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use weaver_semconv::{
    group::{InstrumentSpec, SpanKindSpec},
    stability::Stability,
};

/// Statistics on a resolved telemetry schema.
#[derive(Debug, Serialize)]
#[must_use]
pub struct Stats {
    /// Registry statistics.
    pub registry: RegistryStats,
    /// Refinement statistics.
    pub refinements: RefinementStats,
}

/// Statistics about V2 registries.
#[derive(Debug, Serialize)]
pub struct RegistryStats {
    /// Statistics about attribute registry.
    pub attributes: AttributeStats,
    /// Statistics about metric registry.
    pub metrics: MetricStats,
    /// Statistics about span registry.
    pub spans: SpanStats,
    /// Statistics about event registry.
    pub events: EventStats,
    /// Statistics about entity registry.
    pub entities: EntityStats,
    /// Statistics about attribute_group registry.
    pub attribute_groups: AttributeGroupStats,
}

/// Statistics on the attribute.
#[derive(Debug, Serialize)]
#[must_use]
pub struct AttributeStats {
    /// Total number of attributes.
    pub attribute_count: usize,
    /// Breakdown of attribute types.
    pub attribute_type_breakdown: BTreeMap<String, usize>,
    /// Breakdown of stability levels.
    pub stability_breakdown: HashMap<Stability, usize>,
    /// Number of deprecated attributes.
    pub deprecated_count: usize,
}
/// Common statistics for a group.
#[derive(Debug, Serialize, Default)]
pub struct CommonSignalStats {
    /// Number of instances in this type of group.
    pub count: usize,
    /// Stability breakdown.
    pub stability_breakdown: HashMap<Stability, usize>,
    /// Number of deprecated signals.
    pub deprecated_count: usize,
    /// Total number of groups with a note.
    pub total_with_note: usize,
}

/// Statistics for public attribute groups.
#[derive(Debug, Serialize)]
pub struct AttributeGroupStats {
    /// Common statistics for every signal.
    pub common: CommonSignalStats,
}

/// Statistics for a metric.
#[derive(Debug, Serialize)]
pub struct MetricStats {
    /// Common statistics for every signal.
    pub common: CommonSignalStats,
    /// Metric names.
    pub metric_names: HashSet<String>,
    /// Instrument breakdown.
    pub instrument_breakdown: HashMap<InstrumentSpec, usize>,
    /// Unit breakdown.
    pub unit_breakdown: HashMap<String, usize>,
}

/// Statistics about Spans.
#[derive(Debug, Serialize)]
pub struct SpanStats {
    /// Common statistics for every signal.
    pub common: CommonSignalStats,
    /// Span kind breakdown.
    pub span_kind_breakdown: HashMap<SpanKindSpec, usize>,
}

/// Statistics about events.
#[derive(Debug, Serialize)]
pub struct EventStats {
    /// Common statistics for every signal.
    pub common: CommonSignalStats,
    /// Event names.
    pub event_names: HashSet<String>,
}

/// Statistics about entities.
#[derive(Debug, Serialize)]
pub struct EntityStats {
    /// Common statistics for every signal.
    pub common: CommonSignalStats,
    /// Entity types.
    pub entity_types: HashSet<String>,
    /// A map of the "length" of identity (number of attributes)
    /// to the number of entities with that length.
    pub entity_identity_length_distribution: HashMap<usize, usize>,
}

/// Statistics about V2 refinements.
#[derive(Debug, Serialize)]
pub struct RefinementStats {}
