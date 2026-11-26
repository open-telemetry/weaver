// SPDX-License-Identifier: Apache-2.0

//! V2 entities require identity, so V1 entities without identity attributes become public
//! attribute groups when resolving for V2.
//!
//! Only base entities (no `extends`) are converted. Refining a converted entity fails when
//! converting to V2.

use std::collections::HashSet;

use weaver_resolved_schema::v1::registry::Group;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_semconv::v1::attribute::AttributeRole;
use weaver_semconv::v1::group::{AttributeGroupVisibilitySpec, GroupType};

use crate::error::Error;
use crate::v1::registry::check_v2_signal_id_collisions;

/// Converts base V1 entities without identity in a resolved V1 schema.
pub(crate) fn convert_entities_without_identity(
    schema: &mut ResolvedTelemetrySchema,
) -> Vec<Error> {
    let catalog = &schema.catalog;
    let mut warnings = vec![];
    let mut converted = HashSet::new();
    for g in schema
        .registry
        .groups
        .iter_mut()
        .filter(|g| g.r#type == GroupType::Entity && !g.is_v2 && extends_group(g).is_none())
    {
        let has_identity = g
            .attributes
            .iter()
            .filter_map(|ar| catalog.attribute(ar))
            .any(|a| a.role == Some(AttributeRole::Identifying));
        if has_identity {
            continue;
        }
        g.r#type = GroupType::AttributeGroup;
        g.visibility = Some(AttributeGroupVisibilitySpec::Public);
        let _ = converted.insert(g.id.clone());
        warnings.push(Error::EntityConvertedToAttributeGroup {
            group_id: g.id.clone(),
            provenance: g.provenance().map(Box::new),
        });
    }
    let mut collisions = vec![];
    check_v2_signal_id_collisions(&schema.registry, &mut collisions);
    warnings.extend(collisions.into_iter().filter(|e| {
        matches!(e, Error::CollidingV2SignalId { group_ids, .. }
            if group_ids.iter().any(|id| converted.contains(id)))
    }));
    warnings
}

fn extends_group(g: &Group) -> Option<&String> {
    g.lineage.as_ref()?.extends_group.as_ref()
}
