// SPDX-License-Identifier: Apache-2.0

//! Attribute group expansion and `ref_group` DAG resolution.

use std::collections::{BTreeMap, HashSet};
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::v2::attribute::{AttributeOrGroupRef, AttributeRef, RequirementLevel};
use weaver_semconv::v2::attribute_group::AttributeGroup;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::SpanAttributeOrGroupRef;
use weaver_semconv::v2::CommonFields;

use super::attributes::Catalog;
use super::index::{Index, IndexedAttributeGroup, IndexedEvent, IndexedMetric, IndexedSpan};
use super::rules;
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::exclusion::is_excluded;

/// An attribute item within an expanded attribute group.
#[derive(Debug, Clone)]
pub(crate) struct GroupAttributeItem {
    pub key: String,
    pub aref: AttributeRef,
    pub sampling_relevant: Option<bool>,
    /// The group from which this attribute entered (used for sibling collision checks).
    pub origin_group: SignalId,
}

/// Resolved public attribute group ready for emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPublicGroup {
    pub id: SignalId,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
    pub common: CommonFields,
    pub provenance: SchemaProvenance,
}

/// Information about an expanded group.
#[derive(Debug, Clone)]
pub(crate) struct ExpandedGroup {
    pub items: Vec<GroupAttributeItem>,
    #[allow(dead_code)]
    pub is_public: bool,
    pub is_excluded: bool,
    pub excluded_source: Option<(String, &'static str)>,
    pub signal_type: Option<&'static str>,
}

/// Expanded attribute groups collection.
#[derive(Debug, Default)]
pub(crate) struct ExpandedGroups {
    groups: BTreeMap<SignalId, ExpandedGroup>,
    public_groups: Vec<ResolvedPublicGroup>,
}

impl ExpandedGroups {
    /// Returns the expanded group or signal entry for `id`, if present.
    #[must_use]
    pub fn get(&self, id: &SignalId) -> Option<&ExpandedGroup> {
        self.groups.get(id)
    }

    /// Returns all resolved public attribute groups sorted by ID.
    #[must_use]
    pub fn public_groups(&self) -> &[ResolvedPublicGroup] {
        &self.public_groups
    }
}

/// An index entry that can be targeted by `ref_group` expansion.
enum TargetKind<'a> {
    AttributeGroup(&'a IndexedAttributeGroup),
    Span(&'a IndexedSpan),
    Metric(&'a IndexedMetric),
    Event(&'a IndexedEvent),
}

impl<'a> TargetKind<'a> {
    /// Returns the signal kind name when this target is a signal rather than an attribute group.
    fn signal_type(&self) -> Option<&'static str> {
        match self {
            TargetKind::AttributeGroup(_) => None,
            TargetKind::Span(_) => Some("span"),
            TargetKind::Metric(_) => Some("metric"),
            TargetKind::Event(_) => Some("event"),
        }
    }

    /// Returns `true` if this target is annotated as excluded from dependency resolution.
    fn is_excluded(&self) -> bool {
        match self {
            TargetKind::AttributeGroup(ag) => match &ag.group {
                AttributeGroup::Internal(_) => false,
                AttributeGroup::Public(p) => is_excluded(&p.common.annotations),
            },
            TargetKind::Span(s) => is_excluded(&s.span.common.annotations),
            TargetKind::Metric(m) => is_excluded(&m.metric.common.annotations),
            TargetKind::Event(e) => is_excluded(&e.event.common.annotations),
        }
    }

    /// Returns `true` if this target is publicly visible outside the local registry.
    fn is_public(&self) -> bool {
        match self {
            TargetKind::AttributeGroup(ag) => match &ag.group {
                AttributeGroup::Internal(_) => false,
                AttributeGroup::Public(_) => true,
            },
            TargetKind::Span(_) | TargetKind::Metric(_) | TargetKind::Event(_) => true,
        }
    }

    /// Returns the source file provenance of this target.
    fn provenance(&self) -> &SemconvProvenance {
        match self {
            TargetKind::AttributeGroup(ag) => &ag.provenance,
            TargetKind::Span(s) => &s.provenance,
            TargetKind::Metric(m) => &m.provenance,
            TargetKind::Event(e) => &e.provenance,
        }
    }
}

/// Looks up an attribute group or `<kind>.<id>` signal in `index`.
fn lookup_target<'a>(id: &SignalId, index: &'a Index) -> Option<TargetKind<'a>> {
    if let Some(ag) = index.attribute_groups().get(id) {
        return Some(TargetKind::AttributeGroup(ag));
    }
    let id_str: &str = id.as_ref();
    if let Some(type_id) = id_str.strip_prefix("span.") {
        if let Some(s) = index.spans().get(&SignalId::from(type_id)) {
            return Some(TargetKind::Span(s));
        }
    }
    if let Some(name) = id_str.strip_prefix("metric.") {
        if let Some(m) = index.metrics().get(&SignalId::from(name)) {
            return Some(TargetKind::Metric(m));
        }
    }
    if let Some(name) = id_str.strip_prefix("event.") {
        if let Some(e) = index.events().get(&SignalId::from(name)) {
            return Some(TargetKind::Event(e));
        }
    }
    None
}

/// Expands all attribute groups and signals in topological order, detecting cycles and collisions.
pub(crate) fn expand<'a, C: SchemaCacheLookup>(
    index: &Index,
    catalog: &mut Catalog<'a, C>,
    warnings: &mut Vec<Error>,
) -> Result<ExpandedGroups, Error> {
    let mut expanded = BTreeMap::new();
    let mut visiting_set = HashSet::new();
    let mut visiting_stack = Vec::new();

    for id in index.attribute_groups().keys() {
        expand_target(
            id,
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }

    for s in index.spans().keys() {
        let span_id = SignalId::from(format!("span.{s}"));
        expand_target(
            &span_id,
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }
    for m in index.metrics().keys() {
        let metric_id = SignalId::from(format!("metric.{m}"));
        expand_target(
            &metric_id,
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }
    for e in index.events().keys() {
        let event_id = SignalId::from(format!("event.{e}"));
        expand_target(
            &event_id,
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }

    let mut public_groups = Vec::new();
    for (id, indexed_group) in index.attribute_groups() {
        if let AttributeGroup::Public(p) = &indexed_group.group {
            let items = expanded
                .get(id)
                .map(|g| g.items.clone())
                .unwrap_or_default();
            let is_group_excluded = is_excluded(&p.common.annotations);
            let mut resolved_attributes = Vec::new();

            for item in items {
                let resolved_attr = catalog.resolve_attribute_ref(
                    &item.aref,
                    id.as_ref(),
                    false,
                    is_group_excluded,
                    Some(&indexed_group.provenance),
                )?;
                let req_level = rules::resolve_requirement_level(&item.aref);
                resolved_attributes.push((resolved_attr, req_level));
            }

            resolved_attributes.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

            public_groups.push(ResolvedPublicGroup {
                id: id.clone(),
                attributes: resolved_attributes,
                common: p.common.clone(),
                provenance: SchemaProvenance {
                    source: None,
                    path: indexed_group.provenance.path.clone(),
                },
            });
        }
    }

    public_groups.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(ExpandedGroups {
        groups: expanded,
        public_groups,
    })
}

/// Maximum `ref_group` inclusion depth before reporting a cycle error.
const MAX_REF_GROUP_DEPTH: usize = 64;

/// Recursively expands `ref_group` inclusions for `id`, tracking active ancestors for cycle detection.
fn expand_target(
    id: &SignalId,
    index: &Index,
    expanded: &mut BTreeMap<SignalId, ExpandedGroup>,
    visiting_set: &mut HashSet<SignalId>,
    visiting_stack: &mut Vec<SignalId>,
    warnings: &mut Vec<Error>,
) -> Result<(), Error> {
    if expanded.contains_key(id) {
        return Ok(());
    }

    let target = lookup_target(id, index).ok_or_else(|| Error::UnresolvedRefGroup {
        used_in: id.to_string(),
        ref_group: id.to_string(),
        provenance: None,
    })?;
    let provenance = target.provenance().clone();

    if visiting_set.contains(id) {
        let start = visiting_stack.iter().position(|x| x == id).unwrap_or(0);
        let mut cycle_parts: Vec<String> = visiting_stack[start..]
            .iter()
            .map(|s| s.to_string())
            .collect();
        cycle_parts.push(id.to_string());
        let cycle_str = cycle_parts.join(" -> ");
        return Err(Error::AttributeGroupCycle {
            cycle: cycle_str,
            provenance: Some(Box::new(provenance)),
        });
    }

    if visiting_stack.len() >= MAX_REF_GROUP_DEPTH {
        return Err(Error::AttributeGroupCycle {
            cycle: format!("maximum ref_group depth of {MAX_REF_GROUP_DEPTH} exceeded at {id}"),
            provenance: Some(Box::new(provenance)),
        });
    }

    _ = visiting_set.insert(id.clone());
    visiting_stack.push(id.clone());

    let is_public = target.is_public();
    let self_excluded = target.is_excluded();
    let signal_type = target.signal_type();
    let self_excluded_source = if self_excluded {
        Some((
            id.to_string(),
            target.signal_type().unwrap_or("attribute_group"),
        ))
    } else {
        None
    };
    let mut inherited_excluded_source = None;

    let mut items_map: BTreeMap<String, GroupAttributeItem> = BTreeMap::new();

    match target {
        TargetKind::AttributeGroup(ag) => {
            let raw_attributes = match &ag.group {
                AttributeGroup::Internal(i) => &i.attributes,
                AttributeGroup::Public(p) => &p.attributes,
            };
            expand_attribute_or_group_refs(
                id,
                raw_attributes,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
                &mut items_map,
                is_public,
                self_excluded,
                &mut inherited_excluded_source,
                &provenance,
                false,
            )?;
        }
        TargetKind::Metric(m) => {
            expand_attribute_or_group_refs(
                id,
                &m.metric.attributes,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
                &mut items_map,
                is_public,
                self_excluded,
                &mut inherited_excluded_source,
                &provenance,
                false,
            )?;
        }
        TargetKind::Event(e) => {
            expand_attribute_or_group_refs(
                id,
                &e.event.attributes,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
                &mut items_map,
                is_public,
                self_excluded,
                &mut inherited_excluded_source,
                &provenance,
                false,
            )?;
        }
        TargetKind::Span(s) => {
            expand_span_attribute_or_group_refs(
                id,
                &s.span.attributes,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
                &mut items_map,
                is_public,
                self_excluded,
                &mut inherited_excluded_source,
                &provenance,
            )?;
        }
    }

    _ = visiting_stack.pop();
    _ = visiting_set.remove(id);
    let excluded_source = self_excluded_source.or(inherited_excluded_source);
    _ = expanded.insert(
        id.clone(),
        ExpandedGroup {
            items: items_map.into_values().collect(),
            is_public,
            is_excluded: self_excluded,
            excluded_source,
            signal_type,
        },
    );
    Ok(())
}

/// Expands `ref_group` entries followed by inline attribute overrides for an attribute group, metric, or event.
fn expand_attribute_or_group_refs(
    consumer_id: &SignalId,
    raw_attributes: &[AttributeOrGroupRef],
    index: &Index,
    expanded: &mut BTreeMap<SignalId, ExpandedGroup>,
    visiting_set: &mut HashSet<SignalId>,
    visiting_stack: &mut Vec<SignalId>,
    warnings: &mut Vec<Error>,
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    is_consumer_public: bool,
    is_consumer_excluded: bool,
    inherited_excluded_source: &mut Option<(String, &'static str)>,
    provenance: &SemconvProvenance,
    is_span_consumer: bool,
) -> Result<(), Error> {
    for entry in raw_attributes {
        if let AttributeOrGroupRef::Group(gref) = entry {
            let ref_id = &gref.ref_group;
            let target = lookup_target(ref_id, index).ok_or_else(|| Error::UnresolvedRefGroup {
                used_in: consumer_id.to_string(),
                ref_group: ref_id.to_string(),
                provenance: Some(Box::new(provenance.clone())),
            })?;

            if let Some(sig_type) = target.signal_type() {
                warnings.push(Error::DeprecatedSignalRefGroup {
                    used_in: consumer_id.to_string(),
                    ref_group: ref_id.to_string(),
                    signal_type: sig_type.to_owned(),
                    provenance: Some(Box::new(provenance.clone())),
                });
            }

            expand_target(
                ref_id,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
            )?;

            let included = expanded.get(ref_id).expect("target was just expanded");
            let taint = if included.is_excluded {
                Some(included.excluded_source.clone().unwrap_or_else(|| {
                    (
                        ref_id.to_string(),
                        target.signal_type().unwrap_or("attribute_group"),
                    )
                }))
            } else {
                included.excluded_source.clone()
            };

            if let Some((ex_id, ex_type)) = taint {
                if is_consumer_public && !is_consumer_excluded {
                    return Err(Error::ExcludedFromDependencyResolution {
                        id: ex_id,
                        r#type: ex_type.to_owned(),
                        used_in: consumer_id.to_string(),
                        provenance: Some(Box::new(provenance.clone())),
                    });
                }
                if inherited_excluded_source.is_none() {
                    *inherited_excluded_source = Some((ex_id, ex_type));
                }
            }

            let mut dup_errors = Vec::new();
            for item in &included.items {
                if let Some(existing) = items_map.get(&item.key) {
                    if existing.origin_group != item.origin_group {
                        dup_errors.push(Error::DuplicateAttributeId {
                            attribute_id: item.key.clone(),
                            group_ids: vec![
                                existing.origin_group.to_string(),
                                item.origin_group.to_string(),
                            ],
                        });
                    }
                } else {
                    let mut item = item.clone();
                    if !is_span_consumer {
                        item.sampling_relevant = None;
                    }
                    _ = items_map.insert(item.key.clone(), item);
                }
            }
            if !dup_errors.is_empty() {
                return Err(Error::CompoundError(dup_errors));
            }
        }
    }

    for entry in raw_attributes {
        if let AttributeOrGroupRef::Attribute(aref) = entry {
            if let Some(existing) = items_map.get_mut(&aref.r#ref) {
                existing.aref = rules::merge_attribute_refs(&existing.aref, aref);
                existing.origin_group = consumer_id.clone();
            } else {
                _ = items_map.insert(
                    aref.r#ref.clone(),
                    GroupAttributeItem {
                        key: aref.r#ref.clone(),
                        aref: aref.clone(),
                        sampling_relevant: None,
                        origin_group: consumer_id.clone(),
                    },
                );
            }
        }
    }

    Ok(())
}

/// Expands `ref_group` entries followed by inline span attribute overrides, preserving `sampling_relevant`.
fn expand_span_attribute_or_group_refs(
    consumer_id: &SignalId,
    raw_attributes: &[SpanAttributeOrGroupRef],
    index: &Index,
    expanded: &mut BTreeMap<SignalId, ExpandedGroup>,
    visiting_set: &mut HashSet<SignalId>,
    visiting_stack: &mut Vec<SignalId>,
    warnings: &mut Vec<Error>,
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    is_consumer_public: bool,
    is_consumer_excluded: bool,
    inherited_excluded_source: &mut Option<(String, &'static str)>,
    provenance: &SemconvProvenance,
) -> Result<(), Error> {
    for entry in raw_attributes {
        if let SpanAttributeOrGroupRef::Group(gref) = entry {
            let ref_id = SignalId::from(gref.ref_group.clone());
            let target =
                lookup_target(&ref_id, index).ok_or_else(|| Error::UnresolvedRefGroup {
                    used_in: consumer_id.to_string(),
                    ref_group: gref.ref_group.clone(),
                    provenance: Some(Box::new(provenance.clone())),
                })?;

            if let Some(sig_type) = target.signal_type() {
                warnings.push(Error::DeprecatedSignalRefGroup {
                    used_in: consumer_id.to_string(),
                    ref_group: gref.ref_group.clone(),
                    signal_type: sig_type.to_owned(),
                    provenance: Some(Box::new(provenance.clone())),
                });
            }

            expand_target(
                &ref_id,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
            )?;

            let included = expanded.get(&ref_id).expect("target was just expanded");
            let taint = if included.is_excluded {
                Some(included.excluded_source.clone().unwrap_or_else(|| {
                    (
                        ref_id.to_string(),
                        target.signal_type().unwrap_or("attribute_group"),
                    )
                }))
            } else {
                included.excluded_source.clone()
            };

            if let Some((ex_id, ex_type)) = taint {
                if is_consumer_public && !is_consumer_excluded {
                    return Err(Error::ExcludedFromDependencyResolution {
                        id: ex_id,
                        r#type: ex_type.to_owned(),
                        used_in: consumer_id.to_string(),
                        provenance: Some(Box::new(provenance.clone())),
                    });
                }
                if inherited_excluded_source.is_none() {
                    *inherited_excluded_source = Some((ex_id, ex_type));
                }
            }

            let mut dup_errors = Vec::new();
            for item in &included.items {
                if let Some(existing) = items_map.get(&item.key) {
                    if existing.origin_group != item.origin_group {
                        dup_errors.push(Error::DuplicateAttributeId {
                            attribute_id: item.key.clone(),
                            group_ids: vec![
                                existing.origin_group.to_string(),
                                item.origin_group.to_string(),
                            ],
                        });
                    }
                } else {
                    _ = items_map.insert(item.key.clone(), item.clone());
                }
            }
            if !dup_errors.is_empty() {
                return Err(Error::CompoundError(dup_errors));
            }
        }
    }

    for entry in raw_attributes {
        if let SpanAttributeOrGroupRef::Attribute(span_aref) = entry {
            if let Some(existing) = items_map.get_mut(&span_aref.base.r#ref) {
                existing.aref = rules::merge_attribute_refs(&existing.aref, &span_aref.base);
                existing.sampling_relevant =
                    span_aref.sampling_relevant.or(existing.sampling_relevant);
                existing.origin_group = consumer_id.clone();
            } else {
                _ = items_map.insert(
                    span_aref.base.r#ref.clone(),
                    GroupAttributeItem {
                        key: span_aref.base.r#ref.clone(),
                        aref: span_aref.base.clone(),
                        sampling_relevant: span_aref.sampling_relevant,
                        origin_group: consumer_id.clone(),
                    },
                );
            }
        }
    }

    Ok(())
}

/// Returns the pre-expanded attribute items for a metric or event signal.
pub(crate) fn expand_signal_attributes<'a>(
    _raw_attrs: &[AttributeOrGroupRef],
    expanded_groups: &'a ExpandedGroups,
    signal_id: &SignalId,
    signal_kind: &str,
    _signal_excluded: bool,
    _provenance: &SemconvProvenance,
) -> Result<&'a [GroupAttributeItem], Error> {
    let key = SignalId::from(format!("{signal_kind}.{signal_id}"));
    Ok(expanded_groups.get(&key).map_or(&[], |eg| &eg.items))
}

/// Returns the pre-expanded attribute items for a span signal, including `sampling_relevant`.
pub(crate) fn expand_span_attributes<'a>(
    _raw_attrs: &[SpanAttributeOrGroupRef],
    expanded_groups: &'a ExpandedGroups,
    signal_id: &SignalId,
    _span_excluded: bool,
    _provenance: &SemconvProvenance,
) -> Result<&'a [GroupAttributeItem], Error> {
    let key = SignalId::from(format!("span.{signal_id}"));
    Ok(expanded_groups.get(&key).map_or(&[], |eg| &eg.items))
}

/// Collects attributes from `ref_groups` for a refinement, rejecting unresolved, excluded, or colliding groups.
pub(crate) fn collect_refinement_group_items(
    ref_groups: &[SignalId],
    expanded_groups: &ExpandedGroups,
    consumer_id: &str,
    is_consumer_excluded: bool,
    provenance: Option<&SemconvProvenance>,
    warnings: &mut Vec<Error>,
) -> Result<BTreeMap<String, GroupAttributeItem>, Error> {
    let mut items_map: BTreeMap<String, GroupAttributeItem> = BTreeMap::new();
    for group_id in ref_groups {
        let eg = expanded_groups
            .get(group_id)
            .ok_or_else(|| Error::UnresolvedRefGroup {
                used_in: consumer_id.to_owned(),
                ref_group: group_id.to_string(),
                provenance: provenance.map(|p| Box::new(p.clone())),
            })?;

        if let Some(sig_type) = eg.signal_type {
            warnings.push(Error::DeprecatedSignalRefGroup {
                used_in: consumer_id.to_owned(),
                ref_group: group_id.to_string(),
                signal_type: sig_type.to_owned(),
                provenance: provenance.map(|p| Box::new(p.clone())),
            });
        }

        let taint = if eg.is_excluded {
            Some(eg.excluded_source.clone().unwrap_or_else(|| {
                (
                    group_id.to_string(),
                    eg.signal_type.unwrap_or("attribute_group"),
                )
            }))
        } else {
            eg.excluded_source.clone()
        };

        if let Some((ex_id, ex_type)) = taint {
            if !is_consumer_excluded {
                return Err(Error::ExcludedFromDependencyResolution {
                    id: ex_id,
                    r#type: ex_type.to_owned(),
                    used_in: consumer_id.to_owned(),
                    provenance: provenance.map(|p| Box::new(p.clone())),
                });
            }
        }

        let mut dup_errors = Vec::new();
        for item in &eg.items {
            if let Some(existing) = items_map.get(&item.key) {
                if existing.origin_group != *group_id {
                    dup_errors.push(Error::DuplicateAttributeId {
                        attribute_id: item.key.clone(),
                        group_ids: vec![existing.origin_group.to_string(), group_id.to_string()],
                    });
                }
            } else {
                let mut item = item.clone();
                item.origin_group = group_id.clone();
                _ = items_map.insert(item.key.clone(), item);
            }
        }
        if !dup_errors.is_empty() {
            return Err(Error::CompoundError(dup_errors));
        }
    }
    Ok(items_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use weaver_common::result::WResult;
    use weaver_semconv::schema_url::SchemaUrl;
    use weaver_semconv::semconv::SemConvSpecWithProvenance;

    #[test]
    fn test_attribute_group_cycle_detected() {
        let yaml = r#"
file_format: "definition/2"
attribute_groups:
  - id: group_a
    visibility: internal
    attributes:
      - ref_group: group_b
  - id: group_b
    visibility: internal
    attributes:
      - ref_group: group_a
"#;
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(yaml.as_bytes()).unwrap();

        let spec = match SemConvSpecWithProvenance::from_file(
            SchemaUrl::new_unknown(),
            temp_file.path(),
        ) {
            WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s,
            WResult::FatalErr(e) => panic!("fatal: {e}"),
        };

        let (index, _) = crate::v2::index::build(&[spec]).unwrap();
        let empty_deps =
            crate::v2::dependency::V2Dependencies::new(vec![], std::collections::BTreeSet::new());
        let mut catalog = Catalog::new(&empty_deps, &());
        let mut warnings = Vec::new();
        let err = expand(&index, &mut catalog, &mut warnings).unwrap_err();
        assert!(
            matches!(err, Error::AttributeGroupCycle { ref cycle, .. } if cycle.contains("group_a -> group_b -> group_a") || cycle.contains("group_b -> group_a -> group_b")),
            "Expected AttributeGroupCycle, got: {err:?}"
        );
    }
}
