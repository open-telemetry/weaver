// SPDX-License-Identifier: Apache-2.0

//! Shared, version-neutral refinement validation rules for attribute `unref`s and entity identity preservation.

use std::collections::{HashMap, HashSet};
use weaver_semconv::provenance::Provenance;

use crate::error::{AttributeRole, Error};

/// Validates attribute `unref` entries against parent and refinement constraints.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub(crate) fn validate_attribute_unrefs<'a>(
    refinement_id: &str,
    parent_ref: &str,
    unrefs: impl IntoIterator<Item = &'a str>,
    parent_attr_keys: &HashSet<String>,
    parent_required_keys: &HashSet<String>,
    parent_identifying_keys: &HashSet<String>,
    local_attr_keys: &HashSet<String>,
    included_group_attrs: &HashMap<String, String>,
    provenance: Option<&Provenance>,
) -> Vec<Error> {
    let mut seen = HashSet::new();
    let mut errors = Vec::new();

    for key in unrefs {
        let reason = if !seen.insert(key) {
            "duplicate unref entry".to_owned()
        } else if local_attr_keys.contains(key) {
            "the attribute is also explicitly referenced by this refinement".to_owned()
        } else if let Some(source) = included_group_attrs.get(key) {
            format!("the attribute is also included by ref_group `{source}`")
        } else if parent_identifying_keys.contains(key) {
            format!("the attribute identifies entity `{parent_ref}`; refinements must preserve identity")
        } else if parent_required_keys.contains(key) {
            format!("the attribute is required by `{parent_ref}`")
        } else if !parent_attr_keys.contains(key) {
            format!("the attribute is not inherited from `{parent_ref}`")
        } else {
            continue;
        };

        errors.push(Error::InvalidAttributeUnref {
            refinement_id: refinement_id.to_owned(),
            attribute_key: key.to_owned(),
            reason,
            provenance: provenance.cloned().map(Box::new),
        });
    }

    errors
}

/// Validates that an entity refinement preserves its parent's identifying attribute set.
#[must_use]
pub(crate) fn validate_entity_identity_refinements<'a>(
    refinement_id: &str,
    parent_ref: &str,
    parent_identifying_keys: &HashSet<String>,
    refined_roles: impl IntoIterator<Item = (&'a str, AttributeRole)>,
    provenance: Option<&Provenance>,
) -> Vec<Error> {
    if parent_identifying_keys.is_empty() {
        return vec![Error::EntityRefinementWithoutIdentity {
            refinement_id: refinement_id.to_owned(),
            r#ref: parent_ref.to_owned(),
            provenance: provenance.cloned().map(Box::new),
        }];
    }

    refined_roles
        .into_iter()
        .filter_map(|(key, role)| {
            let changes_identity = match role {
                AttributeRole::Identifying => !parent_identifying_keys.contains(key),
                AttributeRole::Descriptive => parent_identifying_keys.contains(key),
            };
            changes_identity.then(|| Error::EntityRefinementChangedIdentity {
                refinement_id: refinement_id.to_owned(),
                r#ref: parent_ref.to_owned(),
                attribute_id: key.to_owned(),
                role,
                provenance: provenance.cloned().map(Box::new),
            })
        })
        .collect()
}
