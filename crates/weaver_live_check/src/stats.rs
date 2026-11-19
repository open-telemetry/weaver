// SPDX-License-Identifier: Apache-2.0

//! Statistics tracking for live check reports.
//!
//! This module provides two modes of operation:
//! - `Cumulative`: Full statistics accumulation for reporting
//! - `Disabled`: Tracks only the highest finding level, for long-running sessions to prevent memory growth

use serde::{Serialize, Serializer};
use std::collections::HashMap;

use crate::live_checker::LiveChecker;
use crate::{Error, FindingLevel, LiveCheckResult, PolicyFinding, VersionedRegistry};
use weaver_checker::NameMatcher;
use weaver_config::LiveCheckCoverageConfig;
use weaver_semconv::v1::group::GroupType;

/// The registry names that count toward `registry_coverage`.
#[derive(Debug, Default)]
pub struct CoverageScope {
    /// `None` includes every name.
    include: Option<NameMatcher>,
    exclude: Option<NameMatcher>,
}

impl CoverageScope {
    fn contains(&self, name: &str) -> bool {
        self.include
            .as_ref()
            .is_none_or(|include| include.is_match(name))
            && !self
                .exclude
                .as_ref()
                .is_some_and(|exclude| exclude.is_match(name))
    }
}

impl TryFrom<&LiveCheckCoverageConfig> for CoverageScope {
    type Error = Error;

    fn try_from(config: &LiveCheckCoverageConfig) -> Result<Self, Self::Error> {
        let compile = |patterns: &[String]| {
            NameMatcher::compile(patterns).map_err(|error| Error::ConfigError {
                error: error.to_string(),
            })
        };
        Ok(Self {
            include: compile(&config.include)?,
            exclude: compile(&config.exclude)?,
        })
    }
}

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
    /// The number of samples matched to each span in a v2 registry
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub(crate) seen_registry_spans: HashMap<String, usize>,
    /// Fraction of the registry attributes, metrics, events and spans that
    /// were seen
    pub(crate) registry_coverage: f32,
    /// What each configured matcher did, in declaration order
    pub(crate) matchers: Vec<MatcherStatistics>,
}

/// What one matcher did over the run
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatcherStatistics {
    /// The matcher `id` from the config
    pub id: String,
    /// How many samples the matcher applied to
    pub matched: u64,
    /// How many samples had a `when` that failed to evaluate
    pub errors: u64,
    /// The error message from the first `when` failure
    pub first_error: Option<String>,
}

impl CumulativeStatistics {
    /// Create a new CumulativeStatistics initialized with registry structure
    #[must_use]
    pub fn new(registry: &VersionedRegistry) -> Self {
        let mut stats = CumulativeStatistics {
            total_entities: 0,
            total_entities_by_type: HashMap::new(),
            total_advisories: 0,
            advice_level_counts: HashMap::new(),
            highest_advice_level_counts: HashMap::new(),
            no_advice_count: 0,
            advice_type_counts: HashMap::new(),
            advice_message_counts: HashMap::new(),
            seen_registry_attributes: HashMap::new(),
            seen_non_registry_attributes: HashMap::new(),
            seen_registry_metrics: HashMap::new(),
            seen_non_registry_metrics: HashMap::new(),
            seen_registry_events: HashMap::new(),
            seen_non_registry_events: HashMap::new(),
            seen_registry_spans: HashMap::new(),
            registry_coverage: 0.0,
            matchers: Vec::new(),
        };
        stats.track_registry_names(registry);
        stats
    }

    /// Start a zero count for each name the registry declares that is not
    /// deprecated. Spans are counted for a v2 registry only.
    fn track_registry_names(&mut self, registry: &VersionedRegistry) {
        fn track(seen: &mut HashMap<String, usize>, name: String) {
            let _ = seen.insert(name, 0);
        }
        match registry {
            VersionedRegistry::V1(reg) => {
                for group in &reg.groups {
                    for attribute in &group.attributes {
                        if attribute.deprecated.is_none() {
                            track(&mut self.seen_registry_attributes, attribute.name.clone());
                        }
                    }
                    if group.r#type == GroupType::Metric && group.deprecated.is_none() {
                        if let Some(metric_name) = &group.metric_name {
                            track(&mut self.seen_registry_metrics, metric_name.clone());
                        }
                    }
                    if group.r#type == GroupType::Event && group.deprecated.is_none() {
                        if let Some(event_name) = &group.name {
                            track(&mut self.seen_registry_events, event_name.clone());
                        }
                    }
                }
            }
            VersionedRegistry::V2(reg) => {
                for attribute in &reg.registry.attributes {
                    if attribute.common.deprecated.is_none() {
                        track(&mut self.seen_registry_attributes, attribute.key.clone());
                    }
                }
                for metric in &reg.registry.metrics {
                    if metric.common.deprecated.is_none() {
                        track(&mut self.seen_registry_metrics, metric.name.to_string());
                    }
                }
                for event in &reg.registry.events {
                    if event.common.deprecated.is_none() {
                        track(&mut self.seen_registry_events, event.name.to_string());
                    }
                }
                for span in &reg.registry.spans {
                    if span.common.deprecated.is_none() {
                        track(&mut self.seen_registry_spans, span.r#type.to_string());
                    }
                }
            }
        }
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
        *self.advice_level_counts.entry(advice.level).or_insert(0) += 1;
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
        *self.highest_advice_level_counts.entry(*advice).or_insert(0) += 1;
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

    /// Count a sample matched to a registry span.
    pub(crate) fn add_span_type_to_coverage(&mut self, span_type: &str) {
        if let Some(count) = self.seen_registry_spans.get_mut(span_type) {
            *count += 1;
        }
    }

    /// Returns the most severe level currently recorded, or `None` if no
    /// findings have been accumulated. Severity ordering is defined by
    /// [`FindingLevel`]'s derived `Ord`: Information < Improvement < Violation.
    pub(crate) fn max_level(&self) -> Option<FindingLevel> {
        self.highest_advice_level_counts.keys().copied().max()
    }

    /// Returns true if any recorded finding is at or above `threshold`.
    pub(crate) fn should_fail(&self, threshold: FindingLevel) -> bool {
        self.max_level().is_some_and(|level| level >= threshold)
    }

    /// The registry attributes, metrics, events and spans in `scope`, with
    /// the number of times each was seen.
    fn in_scope<'a>(
        &'a self,
        scope: &'a CoverageScope,
    ) -> impl Iterator<Item = (&'a String, usize)> {
        self.seen_registry_attributes
            .iter()
            .chain(&self.seen_registry_metrics)
            .chain(&self.seen_registry_events)
            .chain(&self.seen_registry_spans)
            .filter(|(name, _)| scope.contains(name))
            .map(|(name, &count)| (name, count))
    }

    /// The names in `scope` that no sample used, sorted.
    fn unseen<'a>(&'a self, scope: &'a CoverageScope) -> Vec<&'a str> {
        let mut names: Vec<&str> = self
            .in_scope(scope)
            .filter(|&(_, count)| count == 0)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Finalize the statistics: calculate registry coverage and collect what
    /// each matcher did
    pub(crate) fn finalize(&mut self, live_checker: &LiveChecker) {
        self.matchers = live_checker
            .matchers()
            .iter()
            .map(|matcher| {
                let (errors, first_error) = match matcher.errors() {
                    Some((count, message)) => (count, Some(message.to_owned())),
                    None => (0, None),
                };
                MatcherStatistics {
                    id: matcher.id.clone(),
                    matched: matcher.matched(),
                    errors,
                    first_error,
                }
            })
            .collect();

        self.registry_coverage = self.coverage(live_checker.coverage_scope());
    }

    /// The fraction of the names in `scope` that were seen.
    fn coverage(&self, scope: &CoverageScope) -> f32 {
        let (seen, total) = self
            .in_scope(scope)
            .fold((0_usize, 0_usize), |(seen, total), (_, count)| {
                (seen + usize::from(count > 0), total + 1)
            });
        if total > 0 {
            seen as f32 / total as f32
        } else {
            0.0
        }
    }

    /// Whether `scope` holds any registry name.
    pub(crate) fn has_names_in(&self, scope: &CoverageScope) -> bool {
        self.in_scope(scope).next().is_some()
    }
}

/// Disabled statistics (for long-running sessions). Tracks only the highest finding level.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisabledStatistics {
    max_level: Option<FindingLevel>,
}

impl Serialize for DisabledStatistics {
    /// Serializes as a unit struct (`null` in JSON).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit_struct("DisabledStatistics")
    }
}

/// The statistics for a live check report
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum LiveCheckStatistics {
    /// Cumulative statistics mode - tracks all telemetry data
    Cumulative(CumulativeStatistics),
    /// Disabled statistics mode - only the highest finding level is tracked (for long-running sessions)
    Disabled(DisabledStatistics),
}

impl LiveCheckStatistics {
    /// Add a live check result to the stats
    pub fn maybe_add_live_check_result(&mut self, live_check_result: Option<&LiveCheckResult>) {
        match self {
            Self::Cumulative(stats) => {
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
            Self::Disabled(stats) => {
                let level = live_check_result.and_then(|result| result.highest_advice_level);
                stats.max_level = stats.max_level.max(level);
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

    /// Count a sample matched to a registry span.
    pub fn add_span_type_to_coverage(&mut self, span_type: &str) {
        if let Self::Cumulative(stats) = self {
            stats.add_span_type_to_coverage(span_type);
        }
    }

    /// Returns true if any recorded finding is at or above `threshold`.
    #[must_use]
    pub fn should_fail(&self, threshold: FindingLevel) -> bool {
        match self {
            Self::Cumulative(stats) => stats.should_fail(threshold),
            Self::Disabled(stats) => stats.max_level.is_some_and(|level| level >= threshold),
        }
    }

    /// Finalize the statistics
    pub fn finalize(&mut self, live_checker: &LiveChecker) {
        if let Self::Cumulative(stats) = self {
            stats.finalize(live_checker);
        }
    }

    /// The fraction of the in-scope registry names that were seen. `None`
    /// when statistics are disabled.
    #[must_use]
    pub fn registry_coverage(&self) -> Option<f32> {
        match self {
            Self::Cumulative(stats) => Some(stats.registry_coverage),
            Self::Disabled(_) => None,
        }
    }

    /// The registry names in `scope` that no sample used, sorted.
    #[must_use]
    pub fn unseen<'a>(&'a self, scope: &'a CoverageScope) -> Vec<&'a str> {
        match self {
            Self::Cumulative(stats) => stats.unseen(scope),
            Self::Disabled(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_forge::v1::registry::ResolvedRegistry;

    #[test]
    fn test_disabled_statistics() {
        let registry = ResolvedRegistry {
            groups: vec![],
            registry_url: String::new(),
        };
        let versioned_registry = VersionedRegistry::V1(Box::new(registry));

        let mut disabled_stats = LiveCheckStatistics::Disabled(DisabledStatistics::default());
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

        assert!(!disabled_stats.should_fail(FindingLevel::Violation));
        assert!(!normal_stats.should_fail(FindingLevel::Violation));
    }

    fn empty_cumulative() -> CumulativeStatistics {
        let registry = ResolvedRegistry {
            groups: vec![],
            registry_url: String::new(),
        };
        let versioned_registry = VersionedRegistry::V1(Box::new(registry));
        CumulativeStatistics::new(&versioned_registry)
    }

    #[test]
    fn test_max_level_empty() {
        let stats = empty_cumulative();
        assert_eq!(stats.max_level(), None);
        assert!(!stats.should_fail(FindingLevel::Information));
        assert!(!stats.should_fail(FindingLevel::Improvement));
        assert!(!stats.should_fail(FindingLevel::Violation));
    }

    #[test]
    fn test_max_level_information_only() {
        let mut stats = empty_cumulative();
        stats.add_highest_advice_level(&FindingLevel::Information);
        assert_eq!(stats.max_level(), Some(FindingLevel::Information));
        assert!(stats.should_fail(FindingLevel::Information));
        assert!(!stats.should_fail(FindingLevel::Improvement));
        assert!(!stats.should_fail(FindingLevel::Violation));
    }

    #[test]
    fn test_max_level_improvement_only() {
        let mut stats = empty_cumulative();
        stats.add_highest_advice_level(&FindingLevel::Improvement);
        assert_eq!(stats.max_level(), Some(FindingLevel::Improvement));
        assert!(stats.should_fail(FindingLevel::Information));
        assert!(stats.should_fail(FindingLevel::Improvement));
        assert!(!stats.should_fail(FindingLevel::Violation));
    }

    #[test]
    fn test_max_level_violation_dominates() {
        let mut stats = empty_cumulative();
        stats.add_highest_advice_level(&FindingLevel::Information);
        stats.add_highest_advice_level(&FindingLevel::Improvement);
        stats.add_highest_advice_level(&FindingLevel::Violation);
        assert_eq!(stats.max_level(), Some(FindingLevel::Violation));
        assert!(stats.should_fail(FindingLevel::Violation));
        assert!(stats.should_fail(FindingLevel::Improvement));
        assert!(stats.should_fail(FindingLevel::Information));
    }

    fn result_with_level(level: FindingLevel) -> LiveCheckResult {
        LiveCheckResult {
            highest_advice_level: Some(level),
            ..LiveCheckResult::new()
        }
    }

    #[test]
    fn test_disabled_should_fail_without_findings() {
        let mut disabled = LiveCheckStatistics::Disabled(DisabledStatistics::default());
        disabled.maybe_add_live_check_result(None);
        disabled.maybe_add_live_check_result(Some(&LiveCheckResult::new()));
        assert!(!disabled.should_fail(FindingLevel::Information));
        assert!(!disabled.should_fail(FindingLevel::Improvement));
        assert!(!disabled.should_fail(FindingLevel::Violation));
    }

    fn scope(include: &[&str], exclude: &[&str]) -> CoverageScope {
        let patterns = |names: &[&str]| names.iter().map(|&name| name.to_owned()).collect();
        CoverageScope::try_from(&LiveCheckCoverageConfig {
            include: patterns(include),
            exclude: patterns(exclude),
            fail_below: None,
        })
        .expect("valid patterns")
    }

    /// Four attributes, a metric and an event, with `http.request.method`,
    /// `url.path` and the event seen.
    fn partly_seen() -> CumulativeStatistics {
        let mut stats = empty_cumulative();
        for name in [
            "http.request.method",
            "http.response.status_code",
            "url.path",
            "db.name",
        ] {
            let _ = stats.seen_registry_attributes.insert(name.to_owned(), 0);
        }
        let _ = stats
            .seen_registry_metrics
            .insert("http.server.duration".to_owned(), 0);
        let _ = stats.seen_registry_events.insert("db.query".to_owned(), 0);
        stats.add_attribute_name_to_coverage("http.request.method".to_owned());
        stats.add_attribute_name_to_coverage("url.path".to_owned());
        stats.add_event_name_to_coverage("db.query".to_owned());
        stats
    }

    #[test]
    fn coverage_counts_every_name_without_a_scope() {
        let stats = partly_seen();
        let everything = CoverageScope::default();
        assert_eq!(stats.coverage(&everything), 0.5);
        assert_eq!(
            stats.unseen(&everything),
            [
                "db.name",
                "http.response.status_code",
                "http.server.duration"
            ]
        );
    }

    #[test]
    fn coverage_counts_only_the_included_names() {
        let stats = partly_seen();
        let http = scope(&["http.*"], &[]);
        assert_eq!(stats.coverage(&http), 1.0 / 3.0);
        assert_eq!(
            stats.unseen(&http),
            ["http.response.status_code", "http.server.duration"]
        );
        assert_eq!(stats.coverage(&scope(&["url.*", "db.query"], &[])), 1.0);
    }

    #[test]
    fn excluded_names_are_removed_from_the_included_ones() {
        let stats = partly_seen();
        let scope = scope(&["http.*", "url.*"], &["http.response.*", "http.server.*"]);
        assert_eq!(stats.coverage(&scope), 1.0);
        assert!(stats.unseen(&scope).is_empty());
    }

    #[test]
    fn spans_count_toward_coverage() {
        let mut stats = empty_cumulative();
        for name in ["myapp.checkout", "myapp.refund"] {
            let _ = stats.seen_registry_spans.insert(name.to_owned(), 0);
        }
        stats.add_span_type_to_coverage("myapp.checkout");
        stats.add_span_type_to_coverage("not.in.the.registry");

        let everything = CoverageScope::default();
        assert_eq!(stats.coverage(&everything), 0.5);
        assert_eq!(stats.unseen(&everything), ["myapp.refund"]);
        assert_eq!(stats.coverage(&scope(&["myapp.*"], &["myapp.refund"])), 1.0);
        assert!(!stats
            .seen_registry_spans
            .contains_key("not.in.the.registry"));
    }

    #[test]
    fn a_scope_that_matches_no_name_has_no_coverage() {
        let stats = partly_seen();
        let scope = scope(&["absent.*"], &[]);
        assert!(!stats.has_names_in(&scope));
        assert_eq!(stats.coverage(&scope), 0.0);
    }

    #[test]
    fn test_disabled_should_fail_tracks_highest_level() {
        let mut disabled = LiveCheckStatistics::Disabled(DisabledStatistics::default());
        disabled.maybe_add_live_check_result(Some(&result_with_level(FindingLevel::Improvement)));
        disabled.maybe_add_live_check_result(Some(&result_with_level(FindingLevel::Information)));
        disabled.maybe_add_live_check_result(None);
        assert!(disabled.should_fail(FindingLevel::Information));
        assert!(disabled.should_fail(FindingLevel::Improvement));
        assert!(!disabled.should_fail(FindingLevel::Violation));

        disabled.maybe_add_live_check_result(Some(&result_with_level(FindingLevel::Violation)));
        assert!(disabled.should_fail(FindingLevel::Violation));
    }

    #[test]
    fn test_disabled_serializes_as_null() {
        let mut disabled = LiveCheckStatistics::Disabled(DisabledStatistics::default());
        disabled.maybe_add_live_check_result(Some(&result_with_level(FindingLevel::Violation)));
        let json = serde_json::to_string(&disabled).expect("failed to serialize");
        assert_eq!(json, "null");
    }
}
