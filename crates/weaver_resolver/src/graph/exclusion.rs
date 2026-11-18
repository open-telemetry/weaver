// SPDX-License-Identifier: Apache-2.0

//! Detection of items marked excluded from dependency resolution.

use std::collections::BTreeMap;
use weaver_semconv::YamlValue;

/// Annotation key configuring dependency resolution behavior.
pub(crate) const DEPENDENCY_RESOLUTION_KEY: &str = "dependency_resolution";
/// Field under [`DEPENDENCY_RESOLUTION_KEY`] that excludes an item when `true`.
pub(crate) const EXCLUDE_KEY: &str = "exclude";

/// Returns `true` if `annotations` sets `dependency_resolution.exclude: true`.
pub(crate) fn is_excluded(annotations: &BTreeMap<String, YamlValue>) -> bool {
    annotations
        .get(DEPENDENCY_RESOLUTION_KEY)
        .and_then(|v| v.0.as_object())
        .and_then(|m| m.get(EXCLUDE_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> YamlValue {
        YamlValue(serde_saphyr::from_str(s).expect("valid yaml"))
    }

    #[test]
    fn returns_false_when_no_annotations() {
        assert!(!is_excluded(&BTreeMap::new()));
    }

    #[test]
    fn returns_true_when_exclude_set() {
        let mut a = BTreeMap::new();
        _ = a.insert("dependency_resolution".to_owned(), yaml("exclude: true"));
        assert!(is_excluded(&a));
    }

    #[test]
    fn returns_false_when_exclude_false() {
        let mut a = BTreeMap::new();
        _ = a.insert("dependency_resolution".to_owned(), yaml("exclude: false"));
        assert!(!is_excluded(&a));
    }

    #[test]
    fn returns_false_when_unrelated_annotation() {
        let mut a = BTreeMap::new();
        _ = a.insert("code_generation".to_owned(), yaml("exclude: true"));
        assert!(!is_excluded(&a));
    }

    #[test]
    fn returns_false_when_dependency_resolution_not_mapping() {
        let mut a = BTreeMap::new();
        _ = a.insert(
            "dependency_resolution".to_owned(),
            YamlValue(serde_json::Value::Bool(true)),
        );
        assert!(!is_excluded(&a));
    }
}
