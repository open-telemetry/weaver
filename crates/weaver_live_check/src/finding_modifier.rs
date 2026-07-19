// SPDX-License-Identifier: Apache-2.0

//! Finding filter engine for live-check.
//!
//! Applies filters (ID exclusions, min-level, sample-name exclusions) to
//! findings at creation time — before they are stored in `LiveCheckResult`.

use crate::SampleRef;
use weaver_checker::PolicyFinding;
use weaver_config::{FindingFilter, LiveCheckConfig};

/// Engine that applies finding filters.
///
/// Used inline during `add_advice()` to drop findings before they are stored,
/// avoiding collect-then-filter overhead.
pub struct FindingModifier {
    filters: Vec<FindingFilter>,
}

/// Check whether a scope matches a finding's signal_type.
/// A `None` scope matches all findings (global).
fn scope_matches(scope: Option<&String>, signal_type: Option<&String>) -> bool {
    scope.map_or(true, |s| signal_type == Some(s))
}

/// Check whether a finding should be excluded by a given filter.
fn is_excluded_by(finding: &PolicyFinding, filter: &FindingFilter, sample: &SampleRef<'_>) -> bool {
    // Exclude by ID
    if let Some(ref exclude_ids) = filter.exclude {
        if exclude_ids.iter().any(|id| id == &finding.id) {
            return true;
        }
    }
    // Exclude by min_level
    if let Some(min_level) = filter.min_level {
        if finding.level < min_level {
            return true;
        }
    }
    // Exclude by sample name
    if !filter.exclude_samples.is_empty() {
        if let Some(name) = sample.sample_name() {
            if filter.exclude_samples.iter().any(|s| s == name) {
                return true;
            }
        }
    }
    false
}

impl FindingModifier {
    /// Create a new `FindingModifier` from finding filters.
    ///
    /// Returns `None` if the filter list is empty.
    #[must_use]
    pub fn from_filters(filters: &[FindingFilter]) -> Option<Self> {
        if filters.is_empty() {
            return None;
        }
        Some(Self {
            filters: filters.to_vec(),
        })
    }

    /// Create a new `FindingModifier` from a `LiveCheckConfig`.
    ///
    /// Returns `None` if the config has no filters.
    #[must_use]
    pub fn from_config(config: &LiveCheckConfig) -> Option<Self> {
        Self::from_filters(&config.finding_filters)
    }

    /// Apply filters to a finding.
    ///
    /// Returns `None` if the finding should be excluded, or `Some(finding)`
    /// otherwise.
    ///
    /// `sample` is the sample that produced this finding. It is used by
    /// `exclude_samples` filters to suppress findings by sample name (e.g.
    /// attribute key for attribute samples).
    ///
    /// A global filter (no `signal_type`) applies to all findings; a scoped
    /// filter applies only when its `signal_type` matches the finding's
    /// `signal_type`.
    #[must_use]
    pub fn apply(&self, finding: PolicyFinding, sample: &SampleRef<'_>) -> Option<PolicyFinding> {
        for filter in &self.filters {
            if scope_matches(filter.signal_type.as_ref(), finding.signal_type.as_ref())
                && is_excluded_by(&finding, filter, sample)
            {
                return None;
            }
        }
        Some(finding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample_attribute::SampleAttribute;
    use serde_json::json;
    use weaver_checker::FindingLevel;

    fn make_finding(id: &str, level: FindingLevel, signal_type: Option<&str>) -> PolicyFinding {
        PolicyFinding {
            id: id.to_owned(),
            context: Some(json!({})),
            message: format!("Test finding: {id}"),
            level,
            signal_type: signal_type.map(|s| s.to_owned()),
            signal_name: None,
        }
    }

    fn make_attribute(name: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            r#type: None,
            value: None,
            live_check_result: None,
        }
    }

    #[test]
    fn test_no_rules_passthrough() {
        let config = LiveCheckConfig::default();
        let modifier = FindingModifier::from_config(&config);
        assert!(modifier.is_none());
    }

    #[test]
    fn test_global_filter_exclude_by_id() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: Some(vec!["deprecated".to_owned()]),
                min_level: None,
                signal_type: None,
                exclude_samples: vec![],
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config).expect("modifier");
        let attr = make_attribute("some.attr");
        let sample = SampleRef::Attribute(&attr);

        let finding = make_finding("deprecated", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_none());

        let finding = make_finding("not_stable", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_global_filter_min_level() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: None,
                min_level: Some(FindingLevel::Improvement),
                signal_type: None,
                exclude_samples: vec![],
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config).expect("modifier");
        let attr = make_attribute("some.attr");
        let sample = SampleRef::Attribute(&attr);

        let finding = make_finding("foo", FindingLevel::Information, None);
        assert!(modifier.apply(finding, &sample).is_none());

        let finding = make_finding("foo", FindingLevel::Improvement, None);
        assert!(modifier.apply(finding, &sample).is_some());

        let finding = make_finding("foo", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_scoped_filter() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: Some(vec!["not_stable".to_owned()]),
                min_level: None,
                signal_type: Some("span".to_owned()),
                exclude_samples: vec![],
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config).expect("modifier");
        let attr = make_attribute("some.attr");
        let sample = SampleRef::Attribute(&attr);

        let finding = make_finding("not_stable", FindingLevel::Information, Some("span"));
        assert!(modifier.apply(finding, &sample).is_none());

        let finding = make_finding("not_stable", FindingLevel::Information, Some("metric"));
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_exclude_samples_matches_attribute() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: None,
                min_level: None,
                signal_type: None,
                exclude_samples: vec!["trace.parent_id".to_owned(), "trace.span_id".to_owned()],
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config).expect("modifier");

        // Matching attribute name — excluded
        let attr = make_attribute("trace.parent_id");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("span"));
        assert!(modifier.apply(finding, &sample).is_none());

        // Non-matching attribute name — kept
        let attr = make_attribute("http.method");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("span"));
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_exclude_samples_with_signal_type_scope() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: None,
                min_level: None,
                signal_type: Some("span".to_owned()),
                exclude_samples: vec!["trace.parent_id".to_owned()],
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config).expect("modifier");
        let attr = make_attribute("trace.parent_id");
        let sample = SampleRef::Attribute(&attr);

        // Matching signal_type + attribute — excluded
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("span"));
        assert!(modifier.apply(finding, &sample).is_none());

        // Non-matching signal_type — kept even though attribute matches
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("metric"));
        assert!(modifier.apply(finding, &sample).is_some());
    }
}
