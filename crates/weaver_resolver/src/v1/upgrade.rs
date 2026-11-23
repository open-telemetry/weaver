// SPDX-License-Identifier: Apache-2.0

//! Checked dependency upgrades for the V1 resolution algorithm.

use crate::error::Error;
use crate::graph::chosen_versions::{upgrade_if_superseded, SchemaCacheLookup};
pub(crate) use crate::graph::entity_lookup::upgrade_entity_origin;
use crate::graph::exclusion::is_excluded;
use crate::v1::attribute::{AttributeLookup, AttributeSource, AttributeWithSource};
use crate::v1::dependency::{GroupRefinementLookup, GroupSource, GroupSummary};

/// Upgrades an attribute definition to the chosen version if its dependency registry was superseded,
/// preserving local per-signal overrides (`requirement_level`, `sampling_relevant`, `role`).
pub(crate) fn upgrade_attribute<C: SchemaCacheLookup>(
    mut attr_with_source: AttributeWithSource,
    cache_lookup: &C,
) -> Result<AttributeWithSource, Error> {
    if let AttributeSource::Dependency { schema_url } = &attr_with_source.source {
        let attr_name = attr_with_source.attribute.name.clone();
        let Some((chosen_url, upgraded_attr)) = upgrade_if_superseded(
            schema_url,
            cache_lookup,
            |_, chosen_schema| {
                Ok(chosen_schema
                    .lookup_attribute(&attr_name)?
                    .filter(|a| !a.attribute.annotations.as_ref().is_some_and(is_excluded)))
            },
            |orig, chosen| Error::AttributeNotFoundInUpgradedSchema {
                attribute_name: attr_name.clone(),
                original_url: orig.to_string(),
                upgraded_url: chosen.to_string(),
            },
        )?
        else {
            return Ok(attr_with_source);
        };
        let req = attr_with_source.attribute.requirement_level;
        let sampling = attr_with_source.attribute.sampling_relevant;
        let role = attr_with_source.attribute.role;
        attr_with_source = upgraded_attr;
        attr_with_source.attribute.requirement_level = req;
        attr_with_source.attribute.sampling_relevant = sampling;
        attr_with_source.attribute.role = role;
        attr_with_source.source = AttributeSource::Dependency {
            schema_url: chosen_url,
        };
    }
    Ok(attr_with_source)
}

/// Upgrades a refinement parent signal and its association origins to the chosen version if superseded.
pub(crate) fn upgrade_refinement_parent<C: SchemaCacheLookup>(
    parent: GroupSummary,
    id: &str,
    _used_in: &str,
    cache_lookup: &C,
) -> Result<GroupSummary, Error> {
    if parent.source != GroupSource::Dependency {
        return Ok(parent);
    }
    let Some(origin_url) = &parent.origin else {
        return Ok(parent);
    };
    let Some((_, mut upgraded_parent)) = upgrade_if_superseded(
        origin_url,
        cache_lookup,
        |_, chosen_schema| {
            Ok(chosen_schema
                .lookup_group_summary(id)
                .filter(|g| !g.annotations.as_ref().is_some_and(is_excluded)))
        },
        |orig, chosen| Error::GroupNotFoundInUpgradedSchema {
            group_id: id.to_owned(),
            original_url: orig.to_string(),
            upgraded_url: chosen.to_string(),
        },
    )?
    else {
        return Ok(parent);
    };
    for (entity_name, origin) in upgraded_parent.association_origins.iter_mut() {
        *origin = upgrade_entity_origin(entity_name, origin, cache_lookup)?;
    }
    Ok(upgraded_parent)
}
