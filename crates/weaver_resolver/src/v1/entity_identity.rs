// SPDX-License-Identifier: Apache-2.0

//! V2 entities require identity, so V1 entities without identity attributes become public
//! attribute groups when resolving for V2.

use std::collections::HashSet;

use weaver_common::result::WResult;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_semconv::v1::attribute::AttributeRole;
use weaver_semconv::v1::entity_association::EntityAssociation;
use weaver_semconv::v1::group::{AttributeGroupVisibilitySpec, GroupType};

use crate::error::Error;
use crate::v1::imports::import_match_keys;
use crate::v1::registry::check_v2_signal_id_collisions;

/// Converts V1 entities without identity into public attribute groups, with a warning for each.
///
/// Runs on every V1 schema resolved for V2, dependencies included, so entity lookups and
/// imports from dependencies no longer see the converted entities. Local refinements and
/// associations resolved before the conversion fail here.
pub(crate) fn convert_entities_without_identity(
    schema: &mut ResolvedTelemetrySchema,
) -> WResult<(), Error> {
    let catalog = &schema.catalog;
    let mut warnings = vec![];
    let mut converted_ids = HashSet::new();
    let mut converted_names = HashSet::new();
    for g in schema
        .registry
        .groups
        .iter_mut()
        .filter(|g| g.r#type == GroupType::Entity && !g.is_v2)
    {
        let has_identity = g
            .attributes
            .iter()
            .filter_map(|ar| catalog.attribute(ar))
            .any(|a| a.role == Some(AttributeRole::Identifying));
        if has_identity {
            continue;
        }
        converted_names.extend(import_match_keys(g).into_iter().map(str::to_owned));
        let _ = converted_ids.insert(g.id.clone());
        warnings.push(Error::EntityConvertedToAttributeGroup {
            group_id: g.id.clone(),
            provenance: g.provenance().map(Box::new),
        });
        g.r#type = GroupType::AttributeGroup;
        g.visibility = Some(AttributeGroupVisibilitySpec::Public);
    }
    if converted_ids.is_empty() {
        return WResult::Ok(());
    }

    let mut collisions = vec![];
    check_v2_signal_id_collisions(&schema.registry, &mut collisions);
    warnings.extend(collisions.into_iter().filter(|e| {
        matches!(e, Error::CollidingV2SignalId { group_ids, .. }
            if group_ids.iter().any(|id| converted_ids.contains(id)))
    }));

    let mut errors = vec![];
    for g in &schema.registry.groups {
        let provenance = || g.provenance().map(Box::new);
        if g.r#type == GroupType::Entity {
            if let Some(parent) = g
                .lineage
                .as_ref()
                .and_then(|l| l.extends_group.as_ref())
                .filter(|p| converted_ids.contains(*p))
            {
                errors.push(Error::EntityRefinementWithoutIdentity {
                    refinement_id: g.id.clone(),
                    r#ref: parent.clone(),
                    provenance: provenance(),
                });
            }
        }
        // Associations to dependency entities already failed: the dependency was converted first.
        let origins = schema.registry.entity_association_origins.get(&g.id);
        let mut names = vec![];
        collect_refs(&g.entity_associations, &mut names);
        for name in names {
            let is_local = origins
                .and_then(|o| o.get(name))
                .is_none_or(|url| url.as_str() == schema.schema_url);
            if is_local && converted_names.contains(name) {
                errors.push(Error::UnresolvedEntityAssociation {
                    group_id: g.id.clone(),
                    entity_type: name.to_owned(),
                    provenance: provenance(),
                });
            }
        }
    }
    if errors.is_empty() {
        WResult::with_non_fatal_errors((), warnings)
    } else {
        WResult::FatalErr(Error::CompoundError(errors))
    }
}

fn collect_refs<'a>(associations: &'a [EntityAssociation], out: &mut Vec<&'a str>) {
    for a in associations {
        match a {
            EntityAssociation::Ref(name) => out.push(name),
            EntityAssociation::OneOf { one_of: nested }
            | EntityAssociation::AllOf { all_of: nested } => {
                collect_refs(nested, out);
            }
        }
    }
}
