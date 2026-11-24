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

/// Maximum `ref_group` inclusion depth before reporting a cycle error.
const MAX_REF_GROUP_DEPTH: usize = 64;

/// An attribute item within an expanded attribute group.
#[derive(Debug, Clone)]
pub(crate) struct GroupAttributeItem {
    pub key: String,
    pub aref: AttributeRef,
    pub sampling_relevant: Option<bool>,
    /// The `ref_group` (or consumer) from which this attribute entered.
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
    pub is_excluded: bool,
    pub excluded_source: Option<(String, &'static str)>,
    pub signal_type: Option<&'static str>,
}

impl ExpandedGroup {
    /// Returns the exclusion source `(id, kind)` tainting this group, if any.
    fn exclusion_taint(&self, ref_id: &SignalId) -> Option<(String, &'static str)> {
        if self.is_excluded {
            Some(self.excluded_source.clone().unwrap_or_else(|| {
                (
                    ref_id.to_string(),
                    self.signal_type.unwrap_or("attribute_group"),
                )
            }))
        } else {
            self.excluded_source.clone()
        }
    }
}

/// Typed expansion key separating attribute groups from signals whose `<kind>.<id>` string matches a group ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ExpandKey {
    Group(SignalId),
    Span(SignalId),
    Metric(SignalId),
    Event(SignalId),
}

impl ExpandKey {
    /// Returns candidate keys for a `ref_group` identifier, preferring an `attribute_group` over a
    /// deprecated `<kind>.<id>` signal reference.
    fn ref_group_candidates(ref_id: &SignalId) -> [Option<Self>; 2] {
        let id_str: &str = ref_id.as_ref();
        let signal_key = if let Some(type_id) = id_str.strip_prefix("span.") {
            Some(Self::Span(SignalId::from(type_id)))
        } else if let Some(name) = id_str.strip_prefix("metric.") {
            Some(Self::Metric(SignalId::from(name)))
        } else {
            id_str
                .strip_prefix("event.")
                .map(|name| Self::Event(SignalId::from(name)))
        };
        [Some(Self::Group(ref_id.clone())), signal_key]
    }
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
        ExpandKey::ref_group_candidates(ref_id)
            .into_iter()
            .flatten()
            .find_map(|key| self.groups.get(&key))
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
    ExpandKey::ref_group_candidates(ref_id)
        .into_iter()
        .flatten()
        .find_map(|key| lookup_target_by_key(&key, index).map(|target| (key, target)))
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

/// Merges a single inline attribute reference on top of `items_map`.
pub(crate) fn merge_inline_attribute(
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    consumer_id: &SignalId,
    aref: &AttributeRef,
    sampling_relevant: Option<bool>,
) {
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

/// Merges an already-expanded `ref_group` into `items_map`, checking deprecation, exclusion taint,
/// and duplicate attributes across sibling or diamond `ref_group` inclusions.
#[allow(clippy::too_many_arguments)]
fn merge_included_group(
    included: &ExpandedGroup,
    ref_id: &SignalId,
    consumer_id: &str,
    is_consumer_public: bool,
    is_consumer_excluded: bool,
    inherited_excluded_source: Option<&mut Option<(String, &'static str)>>,
    is_span_consumer: bool,
    provenance: Option<&SemconvProvenance>,
    items_map: &mut BTreeMap<String, GroupAttributeItem>,
    warnings: &mut Vec<Error>,
) -> Result<(), Error> {
    if let Some(sig_type) = included.signal_type {
        warnings.push(Error::DeprecatedSignalRefGroup {
            used_in: consumer_id.to_owned(),
            ref_group: ref_id.to_string(),
            signal_type: sig_type.to_owned(),
            provenance: provenance.map(|p| Box::new(p.clone())),
        });
    }

    if let Some((ex_id, ex_type)) = included.exclusion_taint(ref_id) {
        if is_consumer_public && !is_consumer_excluded {
            return Err(Error::ExcludedFromDependencyResolution {
                id: ex_id,
                r#type: ex_type.to_owned(),
                used_in: consumer_id.to_owned(),
                provenance: provenance.map(|p| Box::new(p.clone())),
            });
        }
        if let Some(slot) = inherited_excluded_source {
            if slot.is_none() {
                *slot = Some((ex_id, ex_type));
            }
        }
    }

    let mut dup_errors = Vec::new();
    for item in &included.items {
        if let Some(existing) = items_map.get(&item.key) {
            dup_errors.push(Error::DuplicateAttributeId {
                attribute_id: item.key.clone(),
                group_ids: vec![existing.origin_group.to_string(), ref_id.to_string()],
            });
        } else {
            let mut item = item.clone();
            if !is_span_consumer {
                item.sampling_relevant = None;
            }
            item.origin_group = ref_id.clone();
            _ = items_map.insert(item.key.clone(), item);
        }
    }
    if !dup_errors.is_empty() {
        return Err(Error::CompoundError(dup_errors));
    }
    Ok(())
}

/// Stateful DFS expander for resolving local `ref_group` DAGs.
struct GroupExpander<'a> {
    index: &'a Index,
    expanded: BTreeMap<ExpandKey, ExpandedGroup>,
    visiting_set: HashSet<ExpandKey>,
    visiting_stack: Vec<ExpandKey>,
    warnings: &'a mut Vec<Error>,
}

impl<'a> GroupExpander<'a> {
    /// Creates a new expander over `index`.
    fn new(index: &'a Index, warnings: &'a mut Vec<Error>) -> Self {
        Self {
            index,
            expanded: BTreeMap::new(),
            visiting_set: HashSet::new(),
            visiting_stack: Vec::new(),
            warnings,
        }
    }

    /// Recursively expands `ref_group` inclusions for `key`, detecting cycles and attribute collisions.
    fn expand_target(&mut self, key: &ExpandKey) -> Result<(), Error> {
        if self.expanded.contains_key(key) {
            return Ok(());
        }

        let target =
            lookup_target_by_key(key, self.index).ok_or_else(|| Error::UnresolvedRefGroup {
                used_in: key.to_string(),
                ref_group: key.to_string(),
                provenance: None,
            })?;
        let provenance = target.provenance().clone();

        if self.visiting_set.contains(key) {
            let start = self
                .visiting_stack
                .iter()
                .position(|x| x == key)
                .unwrap_or(0);
            let mut cycle_parts: Vec<String> = self.visiting_stack[start..]
                .iter()
                .map(ToString::to_string)
                .collect();
            cycle_parts.push(key.to_string());
            return Err(Error::AttributeGroupCycle {
                cycle: cycle_parts.join(" -> "),
                provenance: Some(Box::new(provenance)),
            });
        }

        if self.visiting_stack.len() >= MAX_REF_GROUP_DEPTH {
            return Err(Error::AttributeGroupCycle {
                cycle: format!(
                    "maximum ref_group depth of {MAX_REF_GROUP_DEPTH} exceeded at {key}"
                ),
                provenance: Some(Box::new(provenance)),
            });
        }

        _ = self.visiting_set.insert(key.clone());
        self.visiting_stack.push(key.clone());

        let is_public = target.is_public();
        let self_excluded = target.is_excluded();
        let signal_type = target.signal_type();
        let consumer_id = SignalId::from(key.to_string());
        let self_excluded_source = self_excluded.then(|| {
            (
                consumer_id.to_string(),
                signal_type.unwrap_or("attribute_group"),
            )
        });
        let mut inherited_excluded_source = None;
        let mut items_map: BTreeMap<String, GroupAttributeItem> = BTreeMap::new();

        match target {
            TargetKind::AttributeGroup(ag) => {
                let raw_attributes = match &ag.item {
                    AttributeGroup::Internal(i) => &i.attributes,
                    AttributeGroup::Public(p) => &p.attributes,
                };
                self.expand_entries(
                    &consumer_id,
                    raw_attributes,
                    &mut items_map,
                    is_public,
                    self_excluded,
                    &mut inherited_excluded_source,
                    &provenance,
                )?;
            }
            TargetKind::Metric(m) => {
                self.expand_entries(
                    &consumer_id,
                    &m.item.attributes,
                    &mut items_map,
                    is_public,
                    self_excluded,
                    &mut inherited_excluded_source,
                    &provenance,
                )?;
            }
            TargetKind::Event(e) => {
                self.expand_entries(
                    &consumer_id,
                    &e.item.attributes,
                    &mut items_map,
                    is_public,
                    self_excluded,
                    &mut inherited_excluded_source,
                    &provenance,
                )?;
            }
            TargetKind::Span(s) => {
                self.expand_entries(
                    &consumer_id,
                    &s.item.attributes,
                    &mut items_map,
                    is_public,
                    self_excluded,
                    &mut inherited_excluded_source,
                    &provenance,
                )?;
            }
        }

        _ = self.visiting_stack.pop();
        _ = self.visiting_set.remove(key);
        let excluded_source = self_excluded_source.or(inherited_excluded_source);
        _ = self.expanded.insert(
            key.clone(),
            ExpandedGroup {
                items: items_map.into_values().collect(),
                is_excluded: self_excluded,
                excluded_source,
                signal_type,
            },
        );
        Ok(())
    }

    /// Expands `ref_group` entries followed by inline attribute overrides into `items_map`.
    #[allow(clippy::too_many_arguments)]
    fn expand_entries<E: ExpandableAttrEntry>(
        &mut self,
        consumer_id: &SignalId,
        raw_attributes: &[E],
        items_map: &mut BTreeMap<String, GroupAttributeItem>,
        is_consumer_public: bool,
        is_consumer_excluded: bool,
        inherited_excluded_source: &mut Option<(String, &'static str)>,
        provenance: &SemconvProvenance,
    ) -> Result<(), Error> {
        for entry in raw_attributes {
            if let Some(ref_id) = entry.group_ref() {
                let (target_key, _) = lookup_ref_group(&ref_id, self.index).ok_or_else(|| {
                    Error::UnresolvedRefGroup {
                        used_in: consumer_id.to_string(),
                        ref_group: ref_id.to_string(),
                        provenance: Some(Box::new(provenance.clone())),
                    }
                })?;

                self.expand_target(&target_key)?;

                let included = self
                    .expanded
                    .get(&target_key)
                    .expect("target was just expanded");
                merge_included_group(
                    included,
                    &ref_id,
                    consumer_id.as_ref(),
                    is_consumer_public,
                    is_consumer_excluded,
                    Some(inherited_excluded_source),
                    E::IS_SPAN_CONSUMER,
                    Some(provenance),
                    items_map,
                    self.warnings,
                )?;
            }
        }

        for entry in raw_attributes {
            if let Some((aref, sampling_relevant)) = entry.inline_attr() {
                merge_inline_attribute(items_map, consumer_id, aref, sampling_relevant);
            }
        }

        Ok(())
    }
}

/// Expands all attribute groups and signals in topological order, detecting cycles and collisions.
pub(crate) fn expand<'a, C: SchemaCacheLookup>(
    index: &Index,
    catalog: &mut Catalog<'a, C>,
    warnings: &mut Vec<Error>,
) -> Result<ExpandedGroups, Error> {
    let mut expander = GroupExpander::new(index, warnings);

    for id in index.attribute_groups().keys() {
        expander.expand_target(&ExpandKey::Group(id.clone()))?;
    }
    for s in index.spans().keys() {
        expander.expand_target(&ExpandKey::Span(s.clone()))?;
    }
    for m in index.metrics().keys() {
        expander.expand_target(&ExpandKey::Metric(m.clone()))?;
    }
    for e in index.events().keys() {
        expander.expand_target(&ExpandKey::Event(e.clone()))?;
    }

    let expanded = expander.expanded;
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

/// Collects attributes from `ref_groups` for a refinement, rejecting unresolved, excluded, or colliding groups.
pub(crate) fn collect_refinement_group_items(
    ref_groups: &[SignalId],
    expanded_groups: &ExpandedGroups,
    consumer_id: &str,
    is_consumer_excluded: bool,
    is_span_consumer: bool,
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

        merge_included_group(
            eg,
            group_id,
            consumer_id,
            true,
            is_consumer_excluded,
            None,
            is_span_consumer,
            provenance,
            &mut items_map,
            warnings,
        )?;
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

    fn expand_yaml(yaml: &str) -> Result<ExpandedGroups, Error> {
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
        let mut catalog = Catalog::new(
            &empty_deps,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        );
        catalog.define_local(&index).unwrap();
        let mut warnings = Vec::new();
        expand(&index, &mut catalog, &mut warnings)
    }

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
        let err = expand_yaml(yaml).unwrap_err();
        assert!(
            matches!(err, Error::AttributeGroupCycle { ref cycle, .. } if cycle.contains("group_a -> group_b -> group_a") || cycle.contains("group_b -> group_a -> group_b")),
            "Expected AttributeGroupCycle, got: {err:?}"
        );
    }

    #[test]
    fn test_diamond_ref_group_rejected_as_duplicate_attribute() {
        let yaml = r#"
file_format: "definition/2"
attributes:
  - key: shared.attr
    type: string
    brief: shared
    stability: stable
attribute_groups:
  - id: g0
    visibility: internal
    attributes:
      - ref: shared.attr
  - id: g1
    visibility: internal
    attributes:
      - ref_group: g0
  - id: g2
    visibility: internal
    attributes:
      - ref_group: g0
spans:
  - type: test.span
    kind: internal
    brief: test span
    stability: stable
    name:
      note: test
    attributes:
      - ref_group: g1
      - ref_group: g2
"#;
        let err = expand_yaml(yaml).unwrap_err();
        assert!(
            matches!(err, Error::CompoundError(ref errs) if errs.iter().any(|e| matches!(e, Error::DuplicateAttributeId { attribute_id, .. } if attribute_id == "shared.attr"))),
            "Expected DuplicateAttributeId for diamond ref_group, got: {err:?}"
        );
    }

    #[test]
    fn test_same_ref_group_listed_twice_rejected() {
        let yaml = r#"
file_format: "definition/2"
attributes:
  - key: shared.attr
    type: string
    brief: shared
    stability: stable
attribute_groups:
  - id: g0
    visibility: internal
    attributes:
      - ref: shared.attr
spans:
  - type: test.span
    kind: internal
    brief: test span
    stability: stable
    name:
      note: test
    attributes:
      - ref_group: g0
      - ref_group: g0
"#;
        let err = expand_yaml(yaml).unwrap_err();
        assert!(
            matches!(err, Error::CompoundError(ref errs) if errs.iter().any(|e| matches!(e, Error::DuplicateAttributeId { attribute_id, .. } if attribute_id == "shared.attr"))),
            "Expected DuplicateAttributeId when listing same ref_group twice, got: {err:?}"
        );
    }
}
