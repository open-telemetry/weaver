// SPDX-License-Identifier: Apache-2.0

//! Attribute group expansion and `ref_group` DAG resolution.

use std::collections::{BTreeMap, HashSet};
use std::fmt::{self, Display, Formatter};
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::v2::attribute::{AttributeOrGroupRef, AttributeRef, RequirementLevel};
use weaver_semconv::v2::attribute_group::AttributeGroup;
use weaver_semconv::v2::event::Event;
use weaver_semconv::v2::metric::Metric;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::{Span, SpanAttributeOrGroupRef};
use weaver_semconv::v2::CommonFields;

use super::attributes::Catalog;
use super::index::{Index, Indexed};
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

/// Typed expansion key separating attribute groups from signals whose `<kind>.<id>` string matches a group ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ExpandKey {
    Group(SignalId),
    Span(SignalId),
    Metric(SignalId),
    Event(SignalId),
}

impl Display for ExpandKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Group(id) => write!(f, "{id}"),
            Self::Span(id) => write!(f, "span.{id}"),
            Self::Metric(id) => write!(f, "metric.{id}"),
            Self::Event(id) => write!(f, "event.{id}"),
        }
    }
}

/// Expanded attribute groups collection.
#[derive(Debug, Default)]
pub(crate) struct ExpandedGroups {
    groups: BTreeMap<ExpandKey, ExpandedGroup>,
    public_groups: Vec<ResolvedPublicGroup>,
}

impl ExpandedGroups {
    /// Returns the pre-expanded attribute items for `key`.
    #[must_use]
    pub fn items_for(&self, key: &ExpandKey) -> &[GroupAttributeItem] {
        self.groups.get(key).map_or(&[], |eg| &eg.items)
    }

    /// Resolves a `ref_group` identifier, preferring an `attribute_group` over a deprecated `<kind>.<id>` signal.
    #[must_use]
    pub fn resolve_ref_group(&self, ref_id: &SignalId) -> Option<&ExpandedGroup> {
        if let Some(eg) = self.groups.get(&ExpandKey::Group(ref_id.clone())) {
            return Some(eg);
        }
        let id_str: &str = ref_id.as_ref();
        if let Some(type_id) = id_str.strip_prefix("span.") {
            if let Some(eg) = self.groups.get(&ExpandKey::Span(SignalId::from(type_id))) {
                return Some(eg);
            }
        }
        if let Some(name) = id_str.strip_prefix("metric.") {
            if let Some(eg) = self.groups.get(&ExpandKey::Metric(SignalId::from(name))) {
                return Some(eg);
            }
        }
        if let Some(name) = id_str.strip_prefix("event.") {
            if let Some(eg) = self.groups.get(&ExpandKey::Event(SignalId::from(name))) {
                return Some(eg);
            }
        }
        None
    }

    /// Returns all resolved public attribute groups sorted by ID.
    #[must_use]
    pub fn public_groups(&self) -> &[ResolvedPublicGroup] {
        &self.public_groups
    }
}

/// An index entry that can be targeted by `ref_group` expansion.
enum TargetKind<'a> {
    AttributeGroup(&'a Indexed<AttributeGroup>),
    Span(&'a Indexed<Span>),
    Metric(&'a Indexed<Metric>),
    Event(&'a Indexed<Event>),
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
            TargetKind::AttributeGroup(ag) => match &ag.item {
                AttributeGroup::Internal(_) => false,
                AttributeGroup::Public(p) => is_excluded(&p.common.annotations),
            },
            TargetKind::Span(s) => is_excluded(&s.item.common.annotations),
            TargetKind::Metric(m) => is_excluded(&m.item.common.annotations),
            TargetKind::Event(e) => is_excluded(&e.item.common.annotations),
        }
    }

    /// Returns `true` if this target is publicly visible outside the local registry.
    fn is_public(&self) -> bool {
        match self {
            TargetKind::AttributeGroup(ag) => match &ag.item {
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

/// Looks up the index entry corresponding to an explicit `ExpandKey`.
fn lookup_target_by_key<'a>(key: &ExpandKey, index: &'a Index) -> Option<TargetKind<'a>> {
    match key {
        ExpandKey::Group(id) => index
            .attribute_groups()
            .get(id)
            .map(TargetKind::AttributeGroup),
        ExpandKey::Span(id) => index.spans().get(id).map(TargetKind::Span),
        ExpandKey::Metric(id) => index.metrics().get(id).map(TargetKind::Metric),
        ExpandKey::Event(id) => index.events().get(id).map(TargetKind::Event),
    }
}

/// Resolves a `ref_group` string in `index` to its typed `ExpandKey` and `TargetKind`.
fn lookup_ref_group<'a>(
    ref_id: &SignalId,
    index: &'a Index,
) -> Option<(ExpandKey, TargetKind<'a>)> {
    if let Some(ag) = index.attribute_groups().get(ref_id) {
        return Some((
            ExpandKey::Group(ref_id.clone()),
            TargetKind::AttributeGroup(ag),
        ));
    }
    let id_str: &str = ref_id.as_ref();
    if let Some(type_id) = id_str.strip_prefix("span.") {
        let sig_id = SignalId::from(type_id);
        if let Some(s) = index.spans().get(&sig_id) {
            return Some((ExpandKey::Span(sig_id), TargetKind::Span(s)));
        }
    }
    if let Some(name) = id_str.strip_prefix("metric.") {
        let sig_id = SignalId::from(name);
        if let Some(m) = index.metrics().get(&sig_id) {
            return Some((ExpandKey::Metric(sig_id), TargetKind::Metric(m)));
        }
    }
    if let Some(name) = id_str.strip_prefix("event.") {
        let sig_id = SignalId::from(name);
        if let Some(e) = index.events().get(&sig_id) {
            return Some((ExpandKey::Event(sig_id), TargetKind::Event(e)));
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
            &ExpandKey::Group(id.clone()),
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }

    for s in index.spans().keys() {
        expand_target(
            &ExpandKey::Span(s.clone()),
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }
    for m in index.metrics().keys() {
        expand_target(
            &ExpandKey::Metric(m.clone()),
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }
    for e in index.events().keys() {
        expand_target(
            &ExpandKey::Event(e.clone()),
            index,
            &mut expanded,
            &mut visiting_set,
            &mut visiting_stack,
            warnings,
        )?;
    }

    let mut public_groups = Vec::new();
    for (id, indexed_group) in index.attribute_groups() {
        if let AttributeGroup::Public(p) = &indexed_group.item {
            let items = expanded
                .get(&ExpandKey::Group(id.clone()))
                .map(|g| g.items.clone())
                .unwrap_or_default();
            let is_group_excluded = is_excluded(&p.common.annotations);
            let mut resolved_attributes = Vec::new();

            for item in items {
                let resolved_attr = catalog.resolve_attribute_ref(
                    &item.aref,
                    id.as_ref(),
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

/// Recursively expands `ref_group` inclusions for `key`, tracking active ancestors for cycle detection.
fn expand_target(
    key: &ExpandKey,
    index: &Index,
    expanded: &mut BTreeMap<ExpandKey, ExpandedGroup>,
    visiting_set: &mut HashSet<ExpandKey>,
    visiting_stack: &mut Vec<ExpandKey>,
    warnings: &mut Vec<Error>,
) -> Result<(), Error> {
    if expanded.contains_key(key) {
        return Ok(());
    }

    let target = lookup_target_by_key(key, index).ok_or_else(|| Error::UnresolvedRefGroup {
        used_in: key.to_string(),
        ref_group: key.to_string(),
        provenance: None,
    })?;
    let provenance = target.provenance().clone();

    if visiting_set.contains(key) {
        let start = visiting_stack.iter().position(|x| x == key).unwrap_or(0);
        let mut cycle_parts: Vec<String> = visiting_stack[start..]
            .iter()
            .map(ToString::to_string)
            .collect();
        cycle_parts.push(key.to_string());
        let cycle_str = cycle_parts.join(" -> ");
        return Err(Error::AttributeGroupCycle {
            cycle: cycle_str,
            provenance: Some(Box::new(provenance)),
        });
    }

    if visiting_stack.len() >= MAX_REF_GROUP_DEPTH {
        return Err(Error::AttributeGroupCycle {
            cycle: format!("maximum ref_group depth of {MAX_REF_GROUP_DEPTH} exceeded at {key}"),
            provenance: Some(Box::new(provenance)),
        });
    }

    _ = visiting_set.insert(key.clone());
    visiting_stack.push(key.clone());

    let is_public = target.is_public();
    let self_excluded = target.is_excluded();
    let signal_type = target.signal_type();
    let consumer_id = SignalId::from(key.to_string());
    let self_excluded_source = if self_excluded {
        Some((
            consumer_id.to_string(),
            target.signal_type().unwrap_or("attribute_group"),
        ))
    } else {
        None
    };
    let mut inherited_excluded_source = None;

    let mut items_map: BTreeMap<String, GroupAttributeItem> = BTreeMap::new();

    match target {
        TargetKind::AttributeGroup(ag) => {
            let raw_attributes = match &ag.item {
                AttributeGroup::Internal(i) => &i.attributes,
                AttributeGroup::Public(p) => &p.attributes,
            };
            expand_attribute_or_group_refs(
                &consumer_id,
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
            )?;
        }
        TargetKind::Metric(m) => {
            expand_attribute_or_group_refs(
                &consumer_id,
                &m.item.attributes,
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
        TargetKind::Event(e) => {
            expand_attribute_or_group_refs(
                &consumer_id,
                &e.item.attributes,
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
        TargetKind::Span(s) => {
            expand_attribute_or_group_refs(
                &consumer_id,
                &s.item.attributes,
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
    _ = visiting_set.remove(key);
    let excluded_source = self_excluded_source.or(inherited_excluded_source);
    _ = expanded.insert(
        key.clone(),
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

/// Expands a single `ref_group` inclusion into `items_map`, checking deprecation, exclusion, and sibling collisions.
#[allow(clippy::too_many_arguments)]
fn expand_included_ref_group(
    consumer_id: &SignalId,
    ref_id: &SignalId,
    index: &Index,
    expanded: &mut BTreeMap<ExpandKey, ExpandedGroup>,
    visiting_set: &mut HashSet<ExpandKey>,
    visiting_stack: &mut Vec<ExpandKey>,
    warnings: &mut Vec<Error>,
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    is_consumer_public: bool,
    is_consumer_excluded: bool,
    inherited_excluded_source: &mut Option<(String, &'static str)>,
    provenance: &SemconvProvenance,
    is_span_consumer: bool,
) -> Result<(), Error> {
    let (target_key, target) =
        lookup_ref_group(ref_id, index).ok_or_else(|| Error::UnresolvedRefGroup {
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
        &target_key,
        index,
        expanded,
        visiting_set,
        visiting_stack,
        warnings,
    )?;

    let included = expanded.get(&target_key).expect("target was just expanded");
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
    Ok(())
}

/// Adapts authored attribute-or-group entries across span and non-span containers.
trait ExpandableAttrEntry {
    /// Whether the consumer preserves `sampling_relevant` flags on included attributes.
    const IS_SPAN_CONSUMER: bool;
    /// Returns the referenced `ref_group` ID when this entry is a group inclusion.
    fn group_ref(&self) -> Option<SignalId>;
    /// Returns the inline `AttributeRef` and optional `sampling_relevant` flag when this entry is an attribute.
    fn inline_attr(&self) -> Option<(&AttributeRef, Option<bool>)>;
}

impl ExpandableAttrEntry for AttributeOrGroupRef {
    const IS_SPAN_CONSUMER: bool = false;

    fn group_ref(&self) -> Option<SignalId> {
        match self {
            Self::Group(g) => Some(g.ref_group.clone()),
            Self::Attribute(_) => None,
        }
    }

    fn inline_attr(&self) -> Option<(&AttributeRef, Option<bool>)> {
        match self {
            Self::Attribute(a) => Some((a, None)),
            Self::Group(_) => None,
        }
    }
}

impl ExpandableAttrEntry for SpanAttributeOrGroupRef {
    const IS_SPAN_CONSUMER: bool = true;

    fn group_ref(&self) -> Option<SignalId> {
        match self {
            Self::Group(g) => Some(SignalId::from(g.ref_group.clone())),
            Self::Attribute(_) => None,
        }
    }

    fn inline_attr(&self) -> Option<(&AttributeRef, Option<bool>)> {
        match self {
            Self::Attribute(s) => Some((&s.base, s.sampling_relevant)),
            Self::Group(_) => None,
        }
    }
}

/// Expands `ref_group` entries followed by inline attribute overrides into `items_map`.
#[allow(clippy::too_many_arguments)]
fn expand_attribute_or_group_refs<E: ExpandableAttrEntry>(
    consumer_id: &SignalId,
    raw_attributes: &[E],
    index: &Index,
    expanded: &mut BTreeMap<ExpandKey, ExpandedGroup>,
    visiting_set: &mut HashSet<ExpandKey>,
    visiting_stack: &mut Vec<ExpandKey>,
    warnings: &mut Vec<Error>,
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    is_consumer_public: bool,
    is_consumer_excluded: bool,
    inherited_excluded_source: &mut Option<(String, &'static str)>,
    provenance: &SemconvProvenance,
) -> Result<(), Error> {
    for entry in raw_attributes {
        if let Some(ref_id) = entry.group_ref() {
            expand_included_ref_group(
                consumer_id,
                &ref_id,
                index,
                expanded,
                visiting_set,
                visiting_stack,
                warnings,
                items_map,
                is_consumer_public,
                is_consumer_excluded,
                inherited_excluded_source,
                provenance,
                E::IS_SPAN_CONSUMER,
            )?;
        }
    }

    for entry in raw_attributes {
        if let Some((aref, sampling_relevant)) = entry.inline_attr() {
            if let Some(existing) = items_map.get_mut(&aref.r#ref) {
                existing.aref = rules::merge_attribute_refs(&existing.aref, aref);
                existing.sampling_relevant = sampling_relevant.or(existing.sampling_relevant);
                existing.origin_group = consumer_id.clone();
            } else {
                _ = items_map.insert(
                    aref.r#ref.clone(),
                    GroupAttributeItem {
                        key: aref.r#ref.clone(),
                        aref: aref.clone(),
                        sampling_relevant,
                        origin_group: consumer_id.clone(),
                    },
                );
            }
        }
    }

    Ok(())
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
        let eg = expanded_groups.resolve_ref_group(group_id).ok_or_else(|| {
            Error::UnresolvedRefGroup {
                used_in: consumer_id.to_owned(),
                ref_group: group_id.to_string(),
                provenance: provenance.map(|p| Box::new(p.clone())),
            }
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

        let index = crate::v2::index::build(&[spec]).unwrap();
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
