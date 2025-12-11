// SPDX-License-Identifier: Apache-2.0

//! Statistics tracking for live check reports.
//!
//! This module provides two modes of operation:
//! - `Cumulative`: Full statistics accumulation for reporting
//! - `Disabled`: No-op mode for long-running sessions to prevent memory growth

use serde::Serialize;
use std::collections::HashMap;

use crate::{FindingLevel, LiveCheckResult, PolicyFinding, VersionedRegistry};
use weaver_semconv::group::GroupType;

/// Cumulative statistics that track all telemetry data
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CumulativeStatistics {
    /// The total number of sample entities
    pub(crate) total_entities: usize,
    /// The total number of sample entities by type
    pub(crate) total_entities_by_type: HashMap<String, usize>,
    /// The total number of advisories
    pub(crate) total_advisories: usize,
    /// The number of each advice level
    pub(crate) advice_level_counts: HashMap<FindingLevel, usize>,
    /// The number of entities with each highest advice level
    pub(crate) highest_advice_level_counts: HashMap<FindingLevel, usize>,
    /// The number of entities with no advice
    pub(crate) no_advice_count: usize,
    /// The number of entities with each advice type
    pub(crate) advice_type_counts: HashMap<String, usize>,
    /// The number of entities with each advice message
    pub(crate) advice_message_counts: HashMap<String, usize>,
    /// The number of each attribute seen from the registry
    pub(crate) seen_registry_attributes: HashMap<String, usize>,
    /// The number of each non-registry attribute seen
    pub(crate) seen_non_registry_attributes: HashMap<String, usize>,
    /// The number of each metric seen from the registry
    pub(crate) seen_registry_metrics: HashMap<String, usize>,
    /// The number of each non-registry metric seen
    pub(crate) seen_non_registry_metrics: HashMap<String, usize>,
    /// The number of each event seen from the registry
    pub(crate) seen_registry_events: HashMap<String, usize>,
    /// The number of each non-registry event seen
    pub(crate) seen_non_registry_events: HashMap<String, usize>,
    /// Fraction of the registry covered by the attributes, metrics, and events
    pub(crate) registry_coverage: f32,
}

impl CumulativeStatistics {
    /// Create a new CumulativeStatistics initialized with registry structure
    #[must_use]
    pub fn new(registry: &VersionedRegistry) -> Self {
        let (seen_attributes, seen_metrics, seen_events) = Self::extract_registry_items(registry);

        CumulativeStatistics {
            total_entities: 0,
            total_entities_by_type: HashMap::new(),
            total_advisories: 0,
            advice_level_counts: HashMap::new(),
            highest_advice_level_counts: HashMap::new(),
            no_advice_count: 0,
            advice_type_counts: HashMap::new(),
            advice_message_counts: HashMap::new(),
            seen_registry_attributes: seen_attributes,
            seen_non_registry_attributes: HashMap::new(),
            seen_registry_metrics: seen_metrics,
            seen_non_registry_metrics: HashMap::new(),
            seen_registry_events: seen_events,
            seen_non_registry_events: HashMap::new(),
            registry_coverage: 0.0,
        }
    }

    /// Extract registry items for tracking
    fn extract_registry_items(
        registry: &VersionedRegistry,
    ) -> (
        HashMap<String, usize>,
        HashMap<String, usize>,
        HashMap<String, usize>,
    ) {
        let mut seen_attributes = HashMap::new();
        let mut seen_metrics = HashMap::new();
        let mut seen_events = HashMap::new();

        match registry {
            VersionedRegistry::V1(reg) => {
                for group in &reg.groups {
                    for attribute in &group.attributes {
                        if attribute.deprecated.is_none() {
                            let _ = seen_attributes.insert(attribute.name.clone(), 0);
                        }
                    }
                    if group.r#type == GroupType::Metric && group.deprecated.is_none() {
                        if let Some(metric_name) = &group.metric_name {
                            let _ = seen_metrics.insert(metric_name.clone(), 0);
                        }
                    }
                    if group.r#type == GroupType::Event && group.deprecated.is_none() {
                        if let Some(event_name) = &group.name {
                            let _ = seen_events.insert(event_name.clone(), 0);
                        }
                    }
                }
            }
            VersionedRegistry::V2(reg) => {
                for attribute in &reg.attributes {
                    if attribute.common.deprecated.is_none() {
                        let _ = seen_attributes.insert(attribute.key.clone(), 0);
                    }
                }
                for metric in &reg.signals.metrics {
                    if metric.common.deprecated.is_none() {
                        let _ = seen_metrics.insert(metric.name.to_string(), 0);
                    }
                }
                for event in &reg.signals.events {
                    if event.common.deprecated.is_none() {
                        let _ = seen_events.insert(event.name.to_string(), 0);
                    }
                }
            }
        }

        (seen_attributes, seen_metrics, seen_events)
    }

    /// Increment the total number of entities by type
    pub(crate) fn inc_entity_count(&mut self, entity_type: &str) {
        *self
            .total_entities_by_type
            .entry(entity_type.to_owned())
            .or_insert(0) += 1;
        self.total_entities += 1;
    }

    /// Add an advice to the statistics
    pub(crate) fn add_advice(&mut self, advice: &PolicyFinding) {
        *self
            .advice_level_counts
            .entry(advice.level.clone())
            .or_insert(0) += 1;
        *self
            .advice_type_counts
            .entry(advice.id.clone())
            .or_insert(0) += 1;
        *self
            .advice_message_counts
            .entry(advice.message.clone())
            .or_insert(0) += 1;
        self.total_advisories += 1;
    }

    /// Add a highest advice level to the statistics
    pub(crate) fn add_highest_advice_level(&mut self, advice: &FindingLevel) {
        *self
            .highest_advice_level_counts
            .entry(advice.clone())
            .or_insert(0) += 1;
    }

    /// Increment the no advice count in the statistics
    pub(crate) fn inc_no_advice_count(&mut self) {
        self.no_advice_count += 1;
    }

    /// Add attribute name to coverage
    pub(crate) fn add_attribute_name_to_coverage(&mut self, seen_attribute_name: String) {
        if let Some(count) = self.seen_registry_attributes.get_mut(&seen_attribute_name) {
            // This is a registry attribute
            *count += 1;
        } else {
            // This is a non-registry attribute
            *self
                .seen_non_registry_attributes
                .entry(seen_attribute_name)
                .or_insert(0) += 1;
        }
    }

    /// Add metric name to coverage
    pub(crate) fn add_metric_name_to_coverage(&mut self, seen_metric_name: String) {
        if let Some(count) = self.seen_registry_metrics.get_mut(&seen_metric_name) {
            // This is a registry metric
            *count += 1;
        } else {
            // This is a non-registry metric
            *self
                .seen_non_registry_metrics
                .entry(seen_metric_name)
                .or_insert(0) += 1;
        }
    }

    /// Add event name to coverage
    pub(crate) fn add_event_name_to_coverage(&mut self, seen_event_name: String) {
        if seen_event_name.is_empty() {
            // Empty event_names are not counted
            return;
        }
        if let Some(count) = self.seen_registry_events.get_mut(&seen_event_name) {
            // This is a registry event
            *count += 1;
        } else {
            // This is a non-registry event
            *self
                .seen_non_registry_events
                .entry(seen_event_name)
                .or_insert(0) += 1;
        }
    }

    /// Are there any violations in the statistics?
    pub(crate) fn has_violations(&self) -> bool {
        self.highest_advice_level_counts
            .contains_key(&FindingLevel::Violation)
    }

    /// Finalize the statistics by calculating registry coverage
    pub(crate) fn finalize(&mut self) {
        // Calculate the registry coverage
        // (non-zero attributes + non-zero metrics + non-zero events) / (total attributes + total metrics + total events)
        let non_zero_attributes = self
            .seen_registry_attributes
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_attributes = self.seen_registry_attributes.len();

        let non_zero_metrics = self
            .seen_registry_metrics
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_metrics = self.seen_registry_metrics.len();

        let non_zero_events = self
            .seen_registry_events
            .values()
            .filter(|&&count| count > 0)
            .count();
        let total_registry_events = self.seen_registry_events.len();

        let total_registry_items =
            total_registry_attributes + total_registry_metrics + total_registry_events;

        if total_registry_items > 0 {
            self.registry_coverage = ((non_zero_attributes + non_zero_metrics + non_zero_events)
                as f32)
                / (total_registry_items as f32);
        } else {
            self.registry_coverage = 0.0;
        }
    }
}

/// Disabled statistics that perform no accumulation (for long-running sessions)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisabledStatistics;

/// The statistics for a live check report
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum LiveCheckStatistics {
    /// Cumulative statistics mode - tracks all telemetry data
    Cumulative(CumulativeStatistics),
    /// Disabled statistics mode - no accumulation (for long-running sessions)
    Disabled(DisabledStatistics),
}

impl LiveCheckStatistics {
    /// Add a live check result to the stats
    pub fn maybe_add_live_check_result(&mut self, live_check_result: Option<&LiveCheckResult>) {
        if let Self::Cumulative(stats) = self {
            if let Some(result) = live_check_result {
                for advice in &result.all_advice {
                    // Count of total advisories
                    stats.add_advice(advice);
                }
                // Count of samples with the highest advice level
                if let Some(highest_advice_level) = &result.highest_advice_level {
                    stats.add_highest_advice_level(highest_advice_level);
                }

                // Count of samples with no advice
                if result.all_advice.is_empty() {
                    stats.inc_no_advice_count();
                }
            } else {
                // Count of samples with no advice
                stats.inc_no_advice_count();
            }
        }
    }

    /// Increment the total number of entities by type
    pub fn inc_entity_count(&mut self, entity_type: &str) {
        if let Self::Cumulative(stats) = self {
            stats.inc_entity_count(entity_type);
        }
    }

    /// Add attribute name to coverage
    pub fn add_attribute_name_to_coverage(&mut self, seen_attribute_name: String) {
        if let Self::Cumulative(stats) = self {
            stats.add_attribute_name_to_coverage(seen_attribute_name);
        }
    }

    /// Add metric name to coverage
    pub fn add_metric_name_to_coverage(&mut self, seen_metric_name: String) {
        if let Self::Cumulative(stats) = self {
            stats.add_metric_name_to_coverage(seen_metric_name);
        }
    }

    /// Add event name to coverage
    pub fn add_event_name_to_coverage(&mut self, seen_event_name: String) {
        if let Self::Cumulative(stats) = self {
            stats.add_event_name_to_coverage(seen_event_name);
        }
    }

    /// Are there any violations in the statistics?
    #[must_use]
    pub fn has_violations(&self) -> bool {
        match self {
            Self::Cumulative(stats) => stats.has_violations(),
            Self::Disabled(_) => false,
        }
    }

    /// Finalize the statistics
    pub fn finalize(&mut self) {
        if let Self::Cumulative(stats) = self {
            stats.finalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_forge::registry::ResolvedRegistry;

    #[test]
    fn test_disabled_statistics() {
        let registry = ResolvedRegistry {
            groups: vec![],
            registry_url: String::new(),
        };
        let versioned_registry = VersionedRegistry::V1(registry);

        let mut disabled_stats = LiveCheckStatistics::Disabled(DisabledStatistics);
        let mut normal_stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&versioned_registry));

        // Try to add data to both
        disabled_stats.inc_entity_count("test_entity");
        normal_stats.inc_entity_count("test_entity");

        disabled_stats.add_attribute_name_to_coverage("test.attribute".to_owned());
        normal_stats.add_attribute_name_to_coverage("test.attribute".to_owned());
        disabled_stats.add_metric_name_to_coverage("test.metric".to_owned());
        normal_stats.add_metric_name_to_coverage("test.metric".to_owned());
        disabled_stats.add_event_name_to_coverage("test.event".to_owned());
        normal_stats.add_event_name_to_coverage("test.event".to_owned());

        // Verify disabled stats don't accumulate (all operations are no-ops)
        // Note: In practice, disabled stats are never serialized due to the
        // `!args.no_stats && !no_output` guard in live_check.rs:332
        assert!(matches!(disabled_stats, LiveCheckStatistics::Disabled(_)));
        assert!(matches!(normal_stats, LiveCheckStatistics::Cumulative(_)));

        // Verify normal stats accumulate properly
        if let LiveCheckStatistics::Cumulative(cumulative) = &normal_stats {
            assert_eq!(cumulative.total_entities, 1);
            assert!(cumulative
                .total_entities_by_type
                .contains_key("test_entity"));
            assert!(cumulative
                .seen_non_registry_attributes
                .contains_key("test.attribute"));
            assert!(cumulative
                .seen_non_registry_metrics
                .contains_key("test.metric"));
            assert!(cumulative
                .seen_non_registry_events
                .contains_key("test.event"));
        } else {
            panic!("Expected Cumulative statistics");
        }

        // Verify has_violations works for both
        assert!(!disabled_stats.has_violations()); // Always false for disabled
        assert!(!normal_stats.has_violations()); // No violations added yet
    }
}
