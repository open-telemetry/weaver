// SPDX-License-Identifier: Apache-2.0

//! Detection of items marked excluded from dependency resolution.
//!
//! An item (attribute, group, signal) annotated with
//! `dependency_resolution.exclude: true` is invisible when the registry that
//! owns it is loaded as a dependency. References from dependents fail, and
//! references from non-excluded siblings in the same registry also fail to
//! prevent transitive leaks through resolved output.

use std::collections::BTreeMap;
use weaver_semconv::group::GroupType;
use weaver_semconv::v2::attribute_group::AttributeGroupVisibilitySpec;
use weaver_semconv::YamlValue;

/// Annotation key recognized by the resolver.
pub(crate) const DEPENDENCY_RESOLUTION_KEY: &str = "dependency_resolution";
/// Sub-key signalling exclusion.
pub(crate) const EXCLUDE_KEY: &str = "exclude";

/// Returns true if the annotations carry `dependency_resolution.exclude: true`.
///
/// Signature targets the v2 shape (bare `BTreeMap`). For v1 callers whose
/// annotations are `Option<BTreeMap<…>>`, use
/// `annotations.as_ref().is_some_and(is_excluded)`.
pub(crate) fn is_excluded(annotations: &BTreeMap<String, YamlValue>) -> bool {
    annotations
        .get(DEPENDENCY_RESOLUTION_KEY)
        .and_then(|v| v.0.as_mapping())
        .and_then(|m| m.get(serde_yaml::Value::String(EXCLUDE_KEY.to_owned())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Returns true if the group is effectively can't be used by dependents.
/// It happens in two cases:
///
/// - the group carries `dependency_resolution.exclude: true`;
/// - it's an `attribute_group` whose visibility is anything other than
///   `Some(Public)`.
pub(crate) fn is_group_excluded(
    annotations: &Option<BTreeMap<String, YamlValue>>,
    visibility: Option<&AttributeGroupVisibilitySpec>,
    group_type: &GroupType,
) -> bool {
    annotations.as_ref().is_some_and(is_excluded)
        || (matches!(group_type, GroupType::AttributeGroup)
            && !matches!(visibility, Some(AttributeGroupVisibilitySpec::Public)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> YamlValue {
        YamlValue(serde_yaml::from_str(s).expect("valid yaml"))
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
            YamlValue(serde_yaml::Value::Bool(true)),
        );
        assert!(!is_excluded(&a));
    }

    #[test]
    fn is_group_excluded_picks_up_each_signal() {
        let mut excluded = BTreeMap::new();
        _ = excluded.insert("dependency_resolution".to_owned(), yaml("exclude: true"));

        // Signal groups: only the exclude annotation exempts them. Visibility
        // is irrelevant — signals always emit to v2 output.
        assert!(!is_group_excluded(&None, None, &GroupType::Span));
        assert!(!is_group_excluded(
            &None,
            Some(&AttributeGroupVisibilitySpec::Public),
            &GroupType::Metric,
        ));
        assert!(is_group_excluded(
            &Some(excluded.clone()),
            None,
            &GroupType::Span,
        ));

        // Attribute groups: exempt unless visibility is explicitly Public.
        // Covers v2 internal user groups, the v2 synthetic wrapper (None),
        // and v1 attribute_groups (also None — dropped from v2 output).
        assert!(is_group_excluded(
            &None,
            Some(&AttributeGroupVisibilitySpec::Internal),
            &GroupType::AttributeGroup,
        ));
        assert!(is_group_excluded(&None, None, &GroupType::AttributeGroup,));
        assert!(!is_group_excluded(
            &None,
            Some(&AttributeGroupVisibilitySpec::Public),
            &GroupType::AttributeGroup,
        ));
        // Even a public attribute_group is exempt if it carries the exclude
        // annotation.
        assert!(is_group_excluded(
            &Some(excluded),
            Some(&AttributeGroupVisibilitySpec::Public),
            &GroupType::AttributeGroup,
        ));
    }
}
