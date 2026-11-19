// SPDX-License-Identifier: Apache-2.0

//! Checked dependency upgrades for the V1 resolution algorithm.

use crate::error::Error;
use crate::graph::chosen_versions::{
    chosen_version_if_superseded, lookup_chosen_schema, SchemaCacheLookup,
};
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
        let Some(chosen_url) = chosen_version_if_superseded(schema_url, cache_lookup)? else {
            return Ok(attr_with_source);
        };
        let chosen_schema = lookup_chosen_schema(&chosen_url, cache_lookup)?;
        if let Some(upgraded_attr) =
            chosen_schema.lookup_attribute(&attr_with_source.attribute.name)?
        {
            if upgraded_attr
                .attribute
                .annotations
                .as_ref()
                .is_some_and(is_excluded)
            {
                return Err(Error::AttributeNotFoundInUpgradedSchema {
                    attribute_name: attr_with_source.attribute.name.clone(),
                    original_url: schema_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            }
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
        } else {
            return Err(Error::AttributeNotFoundInUpgradedSchema {
                attribute_name: attr_with_source.attribute.name.clone(),
                original_url: schema_url.to_string(),
                upgraded_url: chosen_url.to_string(),
            });
        }
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
    let Some(chosen_url) = chosen_version_if_superseded(origin_url, cache_lookup)? else {
        return Ok(parent);
    };
    let chosen_schema = lookup_chosen_schema(&chosen_url, cache_lookup)?;
    let Some(mut upgraded_parent) = chosen_schema.lookup_group_summary(id) else {
        return Err(Error::GroupNotFoundInUpgradedSchema {
            group_id: id.to_owned(),
            original_url: origin_url.to_string(),
            upgraded_url: chosen_url.to_string(),
        });
    };
    if upgraded_parent
        .annotations
        .as_ref()
        .is_some_and(is_excluded)
    {
        return Err(Error::GroupNotFoundInUpgradedSchema {
            group_id: id.to_owned(),
            original_url: origin_url.to_string(),
            upgraded_url: chosen_url.to_string(),
        });
    }
    for (entity_name, origin) in upgraded_parent.association_origins.iter_mut() {
        *origin = upgrade_entity_origin(entity_name, origin, cache_lookup)?;
    }
    Ok(upgraded_parent)
}
