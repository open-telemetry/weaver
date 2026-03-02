// SPDX-License-Identifier: Apache-2.0

//! Finding modifier engine for live-check.
//!
//! Applies overrides (level changes) and filters (exclusions, min-level)
//! to findings at creation time — before they are stored in `LiveCheckResult`.

use weaver_checker::PolicyFinding;
use weaver_config::{FindingFilter, FindingOverride, LiveCheckConfig};

/// Engine that applies finding overrides and filters.
///
/// Used inline during `add_advice()` to modify or drop findings before
/// they are stored, avoiding collect-then-filter overhead.
pub struct FindingModifier {
    overrides: Vec<FindingOverride>,
    filters: Vec<FindingFilter>,
}

/// Check whether a scope matches a finding's signal_type.
/// A `None` scope matches all findings (global).
fn scope_matches(scope: Option<&String>, signal_type: Option<&String>) -> bool {
    scope.map_or(true, |s| signal_type == Some(s))
}

/// Check whether a finding should be excluded by a given filter.
fn is_excluded_by(finding: &PolicyFinding, filter: &FindingFilter) -> bool {
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
    false
}

impl FindingModifier {
    /// Create a new `FindingModifier` from a `LiveCheckConfig`.
    ///
    /// Returns `None` if the config has no overrides or filters.
    #[must_use]
    pub fn from_config(config: LiveCheckConfig) -> Option<Self> {
        let overrides = config.finding_overrides;
        let filters = config.finding_filters;

        if overrides.is_empty() && filters.is_empty() {
            return None;
        }

        Some(Self { overrides, filters })
    }

    /// Apply overrides and filters to a finding.
    ///
    /// Returns `None` if the finding should be excluded, or `Some(finding)`
    /// with the (possibly modified) level.
    ///
    /// Override matching: first matching override wins (by ID + optional signal_type).
    /// Filter matching: global filter applies to all; scoped filters apply when
    /// their `signal_type` matches the finding's `signal_type`.
    #[must_use]
    pub fn apply(&self, mut finding: PolicyFinding) -> Option<PolicyFinding> {
        // 1. Apply first matching override
        for ov in &self.overrides {
            if !ov.id.iter().any(|id| id == &finding.id) {
                continue;
            }
            if !scope_matches(ov.signal_type.as_ref(), finding.signal_type.as_ref()) {
                continue;
            }
            // First match wins — apply level override
            finding.level = ov.level;
            break;
        }

        // 2. Apply filters
        for filter in &self.filters {
            if scope_matches(filter.signal_type.as_ref(), finding.signal_type.as_ref())
                && is_excluded_by(&finding, filter)
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
    use serde_json::json;
    use weaver_checker::FindingLevel;

    fn make_finding(id: &str, level: FindingLevel, signal_type: Option<&str>) -> PolicyFinding {
        PolicyFinding {
            id: id.to_owned(),
            context: json!({}),
            message: format!("Test finding: {id}"),
            level,
            signal_type: signal_type.map(|s| s.to_owned()),
            signal_name: None,
        }
    }

    #[test]
    fn test_no_rules_passthrough() {
        let config = LiveCheckConfig::default();
        let modifier = FindingModifier::from_config(config);
        assert!(modifier.is_none());
    }

    #[test]
    fn test_override_level() {
        let config = LiveCheckConfig {
            finding_overrides: vec![FindingOverride {
                id: vec!["not_stable".to_owned()],
                level: FindingLevel::Violation,
                signal_type: None,
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("not_stable", FindingLevel::Information, None);
        let result = modifier.apply(finding).expect("should not be excluded");
        assert_eq!(result.level, FindingLevel::Violation);
    }

    #[test]
    fn test_override_scoped_by_signal_type() {
        let config = LiveCheckConfig {
            finding_overrides: vec![FindingOverride {
                id: vec!["not_stable".to_owned()],
                level: FindingLevel::Information,
                signal_type: Some("span".to_owned()),
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        // Matching signal_type — override applies
        let finding = make_finding("not_stable", FindingLevel::Violation, Some("span"));
        let result = modifier.apply(finding).expect("should not be excluded");
        assert_eq!(result.level, FindingLevel::Information);

        // Non-matching signal_type — override does not apply
        let finding = make_finding("not_stable", FindingLevel::Violation, Some("metric"));
        let result = modifier.apply(finding).expect("should not be excluded");
        assert_eq!(result.level, FindingLevel::Violation);
    }

    #[test]
    fn test_override_with_multiple_ids() {
        let config = LiveCheckConfig {
            finding_overrides: vec![FindingOverride {
                id: vec!["a".to_owned(), "b".to_owned()],
                level: FindingLevel::Improvement,
                signal_type: None,
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding_a = make_finding("a", FindingLevel::Information, None);
        assert_eq!(
            modifier.apply(finding_a).expect("result").level,
            FindingLevel::Improvement
        );

        let finding_b = make_finding("b", FindingLevel::Violation, None);
        assert_eq!(
            modifier.apply(finding_b).expect("result").level,
            FindingLevel::Improvement
        );

        let finding_c = make_finding("c", FindingLevel::Violation, None);
        assert_eq!(
            modifier.apply(finding_c).expect("result").level,
            FindingLevel::Violation
        );
    }

    #[test]
    fn test_first_match_wins() {
        let config = LiveCheckConfig {
            finding_overrides: vec![
                FindingOverride {
                    id: vec!["not_stable".to_owned()],
                    level: FindingLevel::Violation,
                    signal_type: None,
                },
                FindingOverride {
                    id: vec!["not_stable".to_owned()],
                    level: FindingLevel::Information,
                    signal_type: None,
                },
            ],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("not_stable", FindingLevel::Improvement, None);
        let result = modifier.apply(finding).expect("should not be excluded");
        assert_eq!(result.level, FindingLevel::Violation);
    }

    #[test]
    fn test_global_filter_exclude_by_id() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: Some(vec!["deprecated".to_owned()]),
                min_level: None,
                signal_type: None,
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("deprecated", FindingLevel::Violation, None);
        assert!(modifier.apply(finding).is_none());

        let finding = make_finding("not_stable", FindingLevel::Violation, None);
        assert!(modifier.apply(finding).is_some());
    }

    #[test]
    fn test_global_filter_min_level() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: None,
                min_level: Some(FindingLevel::Improvement),
                signal_type: None,
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("foo", FindingLevel::Information, None);
        assert!(modifier.apply(finding).is_none());

        let finding = make_finding("foo", FindingLevel::Improvement, None);
        assert!(modifier.apply(finding).is_some());

        let finding = make_finding("foo", FindingLevel::Violation, None);
        assert!(modifier.apply(finding).is_some());
    }

    #[test]
    fn test_scoped_filter() {
        let config = LiveCheckConfig {
            finding_filters: vec![FindingFilter {
                exclude: Some(vec!["not_stable".to_owned()]),
                min_level: None,
                signal_type: Some("span".to_owned()),
            }],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("not_stable", FindingLevel::Information, Some("span"));
        assert!(modifier.apply(finding).is_none());

        let finding = make_finding("not_stable", FindingLevel::Information, Some("metric"));
        assert!(modifier.apply(finding).is_some());
    }

    #[test]
    fn test_override_then_filter() {
        let config = LiveCheckConfig {
            finding_overrides: vec![FindingOverride {
                id: vec!["foo".to_owned()],
                level: FindingLevel::Violation,
                signal_type: None,
            }],
            finding_filters: vec![FindingFilter {
                exclude: Some(vec!["foo".to_owned()]),
                min_level: None,
                signal_type: None,
            }],
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("foo", FindingLevel::Information, None);
        assert!(modifier.apply(finding).is_none());
    }

    #[test]
    fn test_override_level_then_min_level_filter() {
        let config = LiveCheckConfig {
            finding_overrides: vec![FindingOverride {
                id: vec!["foo".to_owned()],
                level: FindingLevel::Information,
                signal_type: None,
            }],
            finding_filters: vec![FindingFilter {
                exclude: None,
                min_level: Some(FindingLevel::Improvement),
                signal_type: None,
            }],
        };
        let modifier = FindingModifier::from_config(config).expect("modifier");

        let finding = make_finding("foo", FindingLevel::Violation, None);
        assert!(modifier.apply(finding).is_none());
    }
}
