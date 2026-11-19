// SPDX-License-Identifier: Apache-2.0

//! Detection of groups marked excluded from dependency resolution.

use std::collections::BTreeMap;
use weaver_semconv::v1::group::{AttributeGroupVisibilitySpec, GroupType};
use weaver_semconv::YamlValue;

use crate::graph::exclusion::is_excluded;

/// Returns `true` if the group carries `dependency_resolution.exclude: true` or is a non-public `attribute_group`.
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
        YamlValue(serde_saphyr::from_str(s).expect("valid yaml"))
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
