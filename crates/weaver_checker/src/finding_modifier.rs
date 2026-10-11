// SPDX-License-Identifier: Apache-2.0

//! Shared ID exclusion and scope matching for findings.

use std::collections::BTreeMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;

use crate::{Error, PolicyFinding};

/// Compiled ID and scope matching shared by policy and live-check filters.
#[derive(Debug, Clone)]
pub struct FindingMatcher {
    ids: Vec<String>,
    signal_type: Option<String>,
    context: BTreeMap<String, Value>,
    signal_names: Option<NameMatcher>,
}

impl FindingMatcher {
    /// Compile ID and scope matching, rejecting invalid signal name patterns.
    pub fn new(
        ids: &[String],
        signal_type: Option<&str>,
        signal_names: &[String],
        context: &BTreeMap<String, Value>,
    ) -> Result<Self, Error> {
        Ok(Self {
            ids: ids.to_vec(),
            signal_type: signal_type.map(str::to_owned),
            signal_names: NameMatcher::compile(signal_names)?,
            context: context.clone(),
        })
    }

    /// Whether the finding matches every configured scope field.
    #[must_use]
    pub fn matches_scope(&self, finding: &PolicyFinding) -> bool {
        self.signal_type
            .as_ref()
            .is_none_or(|signal_type| finding.signal_type.as_ref() == Some(signal_type))
            && self.signal_names.as_ref().is_none_or(|matcher| {
                finding
                    .signal_name
                    .as_deref()
                    .is_some_and(|name| matcher.is_match(name))
            })
            && self.context.iter().all(|(key, value)| {
                finding
                    .context
                    .as_ref()
                    .and_then(|context| context.get(key))
                    == Some(value)
            })
    }

    /// Whether the finding's ID is listed for exclusion.
    #[must_use]
    pub fn matches_id(&self, finding: &PolicyFinding) -> bool {
        self.ids.iter().any(|id| id == &finding.id)
    }
}

/// Matches names against compiled glob patterns.
#[derive(Debug, Clone)]
pub struct NameMatcher(GlobSet);

impl NameMatcher {
    /// Compile name glob patterns, returning no matcher for an empty list.
    pub fn compile(patterns: &[String]) -> Result<Option<Self>, Error> {
        compile_name_patterns(patterns).map(|matcher| matcher.map(Self))
    }

    /// Whether a name matches any configured pattern.
    #[must_use]
    pub fn is_match(&self, name: &str) -> bool {
        self.0.is_match(name)
    }
}

fn compile_name_patterns(patterns: &[String]) -> Result<Option<GlobSet>, Error> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|error| Error::InvalidGlobPattern {
            pattern: pattern.clone(),
            error: error.to_string(),
        })?;
        _ = builder.add(glob);
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| Error::InvalidGlobPattern {
            pattern: patterns.join(", "),
            error: error.to_string(),
        })
}

/// Drops findings that match any configured filter.
#[derive(Debug, Clone)]
pub struct FindingModifier {
    filters: Vec<FindingMatcher>,
}

impl FindingModifier {
    /// Create a modifier from compiled matchers, or none for an empty list.
    #[must_use]
    pub fn from_matchers(filters: Vec<FindingMatcher>) -> Option<Self> {
        if filters.is_empty() {
            None
        } else {
            Some(Self { filters })
        }
    }

    /// Drop a finding if its ID and scope match a filter.
    #[must_use]
    pub fn apply(&self, finding: PolicyFinding) -> Option<PolicyFinding> {
        if self
            .filters
            .iter()
            .any(|filter| filter.matches_scope(&finding) && filter.matches_id(&finding))
        {
            None
        } else {
            Some(finding)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, PolicyStage};
    use serde_json::json;

    #[test]
    fn context_and_names_scope_exclusions() {
        let matcher = FindingMatcher::new(
            &["removed".to_owned(), "deprecated".to_owned()],
            Some("entity"),
            &["dev*".to_owned()],
            &BTreeMap::from([("key".to_owned(), json!("a"))]),
        )
        .unwrap();
        let modifier = FindingModifier::from_matchers(vec![matcher]).unwrap();
        let finding: PolicyFinding = serde_json::from_value(json!({
            "id": "removed", "message": "removed", "level": "violation",
            "signal_type": "entity", "signal_name": "device",
            "context": {"key": "a", "extra": true}
        }))
        .unwrap();
        assert!(modifier.apply(finding.clone()).is_none());
        for signal_name in [Some("host".to_owned()), None] {
            let mut unmatched = finding.clone();
            unmatched.signal_name = signal_name;
            assert!(modifier.apply(unmatched).is_some());
        }
        for context in [None, Some(json!({})), Some(json!({"key": "b"}))] {
            let mut unmatched = finding.clone();
            unmatched.context = context;
            assert!(modifier.apply(unmatched).is_some());
        }
        let mut unmatched = finding.clone();
        unmatched.id = "other".to_owned();
        assert!(modifier.apply(unmatched).is_some());
        let mut unmatched = finding;
        unmatched.signal_type = Some("span".to_owned());
        assert!(modifier.apply(unmatched).is_some());
    }

    #[test]
    fn engine_filters_by_signal_name() {
        let mut engine = Engine::new();
        let _ = engine
            .add_policy(
                "test.rego",
                r#"
                package comparison_after_resolution
                import rego.v1
                deny contains {"id": "removed", "message": name, "level": "violation", "signal_type": "entity", "signal_name": name} if {
                    some name in ["device", "host"]
                }
                "#,
            )
            .unwrap();
        assert_eq!(
            engine
                .check(PolicyStage::ComparisonAfterResolution)
                .unwrap()
                .len(),
            2
        );
        let matcher = FindingMatcher::new(
            &["removed".to_owned()],
            None,
            &["dev*".to_owned()],
            &BTreeMap::new(),
        )
        .unwrap();
        engine.set_finding_filters(vec![matcher]);
        let findings = engine
            .check(PolicyStage::ComparisonAfterResolution)
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].signal_name.as_deref(), Some("host"));
        engine.set_finding_filters(vec![]);
        assert_eq!(
            engine
                .check(PolicyStage::ComparisonAfterResolution)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn invalid_signal_name_pattern_is_rejected() {
        assert!(matches!(
            FindingMatcher::new(&[], None, &["[".to_owned()], &BTreeMap::new()),
            Err(Error::InvalidGlobPattern { .. })
        ));
    }
}
