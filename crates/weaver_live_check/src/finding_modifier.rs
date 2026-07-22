// SPDX-License-Identifier: Apache-2.0

//! Finding filter engine for live-check.
//!
//! Applies filters (ID exclusions, min-level, sample-name exclusions) to
//! findings at creation time — before they are stored in `LiveCheckResult`.

use crate::{Error, SampleRef};
use globset::{Glob, GlobSet, GlobSetBuilder};
use weaver_checker::PolicyFinding;
use weaver_config::{FindingFilter, LiveCheckConfig};

/// Engine that applies finding filters.
///
/// Used inline during `add_advice()` to drop findings before they are stored,
/// avoiding collect-then-filter overhead.
#[derive(Debug)]
pub struct FindingModifier {
    filters: Vec<CompiledFilter>,
}

/// A `FindingFilter` with its glob patterns precompiled once at construction
/// time, so matching a finding never recompiles a pattern.
#[derive(Debug)]
struct CompiledFilter {
    filter: FindingFilter,
    /// Compiled `filter.sample_names` (scope), if non-empty.
    sample_names_matcher: Option<GlobSet>,
    /// Compiled `filter.exclude_samples` (exclusion condition), if non-empty.
    exclude_samples_matcher: Option<GlobSet>,
}

/// Compile a list of glob patterns into a `GlobSet`. Returns `Ok(None)` when
/// `patterns` is empty (nothing to match).
fn compile_globset(patterns: &[String]) -> Result<Option<GlobSet>, Error> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|e| Error::ConfigError {
            error: format!("Invalid sample name pattern '{pattern}': {e}"),
        })?;
        _ = builder.add(glob);
    }
    let set = builder.build().map_err(|e| Error::ConfigError {
        error: format!("Invalid sample name patterns {patterns:?}: {e}"),
    })?;
    Ok(Some(set))
}

/// Check whether a compiled filter's scope (`signal_type` and `sample_names`)
/// matches a finding. Both scopes are optional and combine as AND; an unset
/// scope matches everything.
fn scope_matches(
    compiled: &CompiledFilter,
    finding: &PolicyFinding,
    sample: &SampleRef<'_>,
) -> bool {
    let signal_type_ok = compiled
        .filter
        .signal_type
        .as_ref()
        .is_none_or(|s| finding.signal_type.as_deref() == Some(s.as_str()));
    if !signal_type_ok {
        return false;
    }
    match &compiled.sample_names_matcher {
        None => true,
        Some(matcher) => sample
            .sample_name()
            .is_some_and(|name| matcher.is_match(name)),
    }
}

/// Check whether a finding should be excluded by a given filter.
fn is_excluded_by(
    finding: &PolicyFinding,
    compiled: &CompiledFilter,
    sample: &SampleRef<'_>,
) -> bool {
    // Exclude by ID
    if let Some(ref exclude_ids) = compiled.filter.exclude {
        if exclude_ids.iter().any(|id| id == &finding.id) {
            return true;
        }
    }
    // Exclude by min_level
    if let Some(min_level) = compiled.filter.min_level {
        if finding.level < min_level {
            return true;
        }
    }
    // Exclude by sample name
    if let Some(matcher) = &compiled.exclude_samples_matcher {
        if let Some(name) = sample.sample_name() {
            if matcher.is_match(name) {
                return true;
            }
        }
    }
    false
}

impl FindingModifier {
    /// Create a new `FindingModifier` from finding filters.
    ///
    /// Returns `Ok(None)` if the filter list is empty. Returns `Err` if any
    /// filter's `sample_names` or `exclude_samples` contains an invalid glob
    /// pattern.
    pub fn from_filters(filters: &[FindingFilter]) -> Result<Option<Self>, Error> {
        if filters.is_empty() {
            return Ok(None);
        }
        let filters = filters
            .iter()
            .map(|filter| {
                Ok(CompiledFilter {
                    sample_names_matcher: compile_globset(&filter.sample_names)?,
                    exclude_samples_matcher: compile_globset(&filter.exclude_samples)?,
                    filter: filter.clone(),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Some(Self { filters }))
    }

    /// Create a new `FindingModifier` from a `LiveCheckConfig`.
    ///
    /// Returns `Ok(None)` if the config has no filters.
    pub fn from_config(config: &LiveCheckConfig) -> Result<Option<Self>, Error> {
        Self::from_filters(&config.finding_filters)
    }

    /// Apply filters to a finding.
    ///
    /// Returns `None` if the finding should be excluded, or `Some(finding)`
    /// otherwise.
    ///
    /// `sample` is the sample that produced this finding. It is used by
    /// `sample_names` to scope a filter to matching samples, and by
    /// `exclude_samples` to suppress findings by sample name (e.g. attribute
    /// key for attribute samples). Both support glob wildcards (e.g.
    /// `"http.*"`).
    ///
    /// A global filter (no `signal_type` or `sample_names`) applies to all
    /// findings; a scoped filter applies only when its `signal_type` and/or
    /// `sample_names` match the finding's signal type and sample name.
    #[must_use]
    pub fn apply(&self, finding: PolicyFinding, sample: &SampleRef<'_>) -> Option<PolicyFinding> {
        for compiled in &self.filters {
            if scope_matches(compiled, &finding, sample)
                && is_excluded_by(&finding, compiled, sample)
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
    use crate::sample_resource::SampleResource;
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

    fn make_filter(
        exclude: Option<Vec<String>>,
        min_level: Option<FindingLevel>,
        signal_type: Option<String>,
        exclude_samples: Vec<String>,
        sample_names: Vec<String>,
    ) -> FindingFilter {
        FindingFilter {
            exclude,
            min_level,
            signal_type,
            exclude_samples,
            sample_names,
        }
    }

    #[test]
    fn test_no_rules_passthrough() {
        let config = LiveCheckConfig::default();
        let modifier = FindingModifier::from_config(&config).expect("valid config");
        assert!(modifier.is_none());
    }

    #[test]
    fn test_global_filter_exclude_by_id() {
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                Some(vec!["deprecated".to_owned()]),
                None,
                None,
                vec![],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");
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
            finding_filters: vec![make_filter(
                None,
                Some(FindingLevel::Improvement),
                None,
                vec![],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");
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
            finding_filters: vec![make_filter(
                Some(vec!["not_stable".to_owned()]),
                None,
                Some("span".to_owned()),
                vec![],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");
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
            finding_filters: vec![make_filter(
                None,
                None,
                None,
                vec!["trace.parent_id".to_owned(), "trace.span_id".to_owned()],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

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
            finding_filters: vec![make_filter(
                None,
                None,
                Some("span".to_owned()),
                vec!["trace.parent_id".to_owned()],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");
        let attr = make_attribute("trace.parent_id");
        let sample = SampleRef::Attribute(&attr);

        // Matching signal_type + attribute — excluded
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("span"));
        assert!(modifier.apply(finding, &sample).is_none());

        // Non-matching signal_type — kept even though attribute matches
        let finding = make_finding("missing_attribute", FindingLevel::Violation, Some("metric"));
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_exclude_samples_wildcard() {
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                None,
                None,
                None,
                vec!["trace.*".to_owned()],
                vec![],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

        let attr = make_attribute("trace.span_id");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("missing_attribute", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_none());

        let attr = make_attribute("http.method");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("missing_attribute", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_sample_names_scopes_exclude_and_semantics() {
        // exclude only applies where sample_names also matches (AND, not OR).
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                Some(vec!["illegal_namespace".to_owned()]),
                None,
                None,
                vec![],
                vec!["server.address".to_owned(), "server.port".to_owned()],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

        // Matching id + matching sample name — excluded
        let attr = make_attribute("server.address");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("illegal_namespace", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_none());

        // Matching id but non-matching sample name — kept (this is the case
        // exclude_samples/exclude alone could not express: the same advice on
        // a different sample still surfaces).
        let attr = make_attribute("db.statement");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("illegal_namespace", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_sample_names_wildcard() {
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                Some(vec!["illegal_namespace".to_owned()]),
                None,
                None,
                vec![],
                vec!["http.*".to_owned()],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

        let attr = make_attribute("http.method");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("illegal_namespace", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_none());

        let attr = make_attribute("server.address");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("illegal_namespace", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_sample_names_scopes_min_level_and_exclude_samples_too() {
        // sample_names gates the whole filter block, same as signal_type.
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                None,
                Some(FindingLevel::Improvement),
                None,
                vec![],
                vec!["http.*".to_owned()],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

        let attr = make_attribute("http.method");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("foo", FindingLevel::Information, None);
        assert!(modifier.apply(finding, &sample).is_none());

        let attr = make_attribute("server.address");
        let sample = SampleRef::Attribute(&attr);
        let finding = make_finding("foo", FindingLevel::Information, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_sample_names_scope_never_matches_nameless_sample() {
        // Resources don't have a sample name, so a sample_names-scoped filter
        // never applies to them.
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(
                Some(vec!["foo".to_owned()]),
                None,
                None,
                vec![],
                vec!["*".to_owned()],
            )],
            ..Default::default()
        };
        let modifier = FindingModifier::from_config(&config)
            .expect("valid config")
            .expect("modifier");

        let resource = SampleResource {
            attributes: vec![],
            live_check_result: None,
        };
        let sample = SampleRef::Resource(&resource);
        let finding = make_finding("foo", FindingLevel::Violation, None);
        assert!(modifier.apply(finding, &sample).is_some());
    }

    #[test]
    fn test_invalid_sample_names_pattern_errors() {
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(None, None, None, vec![], vec!["[".to_owned()])],
            ..Default::default()
        };
        let err = FindingModifier::from_config(&config).expect_err("invalid glob pattern");
        assert!(matches!(err, Error::ConfigError { .. }));
    }

    #[test]
    fn test_invalid_exclude_samples_pattern_errors() {
        let config = LiveCheckConfig {
            finding_filters: vec![make_filter(None, None, None, vec!["[".to_owned()], vec![])],
            ..Default::default()
        };
        let err = FindingModifier::from_config(&config).expect_err("invalid glob pattern");
        assert!(matches!(err, Error::ConfigError { .. }));
    }
}
