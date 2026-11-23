// SPDX-License-Identifier: Apache-2.0

//! Resolution of V2 signal refinements: metrics, spans, events, and entities.

use std::collections::{BTreeMap, HashMap, HashSet};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::{
    AttributeRef, AttributeUnref, BasicRequirementLevelSpec, RefinementAttributeOrGroupRef,
    RequirementLevel,
};
use weaver_semconv::v2::entity::{EntityAttributeRefinement, EntityRefinement};
use weaver_semconv::v2::entity_association::EntityAssociation as SemconvEntityAssociation;
use weaver_semconv::v2::event::EventRefinement;
use weaver_semconv::v2::metric::MetricRefinement;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::{SpanRefinement, SpanRefinementAttributeOrGroupRef};
use weaver_semconv::v2::stability::Stability;
use weaver_semconv::v2::CommonFields;
use weaver_semconv::YamlValue;

use super::attributes::Catalog;
use super::dependency::{DepSignal, DepSignalPayload, V2Dependencies};
use super::groups::{self, ExpandedGroups, GroupAttributeItem};
use super::index::{Index, Indexed};
use super::rules;
use super::signals::{
    EntityAssociations, ResolvedEntity, ResolvedEvent, ResolvedMetric, ResolvedSignals,
    ResolvedSpan,
};
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::exclusion::is_excluded;

/// All resolved refinements for a registry.
#[derive(Debug, Default)]
pub(crate) struct ResolvedRefinements {
    pub metrics: Vec<(SignalId, ResolvedMetric)>,
    pub spans: Vec<(SignalId, ResolvedSpan)>,
    pub events: Vec<(SignalId, ResolvedEvent)>,
    pub entities: Vec<(SignalId, ResolvedEntity)>,
}

/// Returns `true` if `req` is `BasicRequirementLevelSpec::Required`.
fn is_required(req: &RequirementLevel) -> bool {
    matches!(
        req,
        RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
    )
}

/// Returns an `InvalidRefinement` error if `parent_ref` matches a local base signal of a different kind.
fn check_wrong_kind_in_base(
    parent_ref: &SignalId,
    stripped_ref: &SignalId,
    raw_id: &SignalId,
    expected_kind: &str,
    base_signals: &ResolvedSignals,
) -> Option<Error> {
    if expected_kind != "Span" {
        if let Some(s) = base_signals
            .spans
            .iter()
            .find(|s| s.r#type == *parent_ref || s.r#type == *stripped_ref)
        {
            return Some(Error::InvalidRefinement {
                refinement_id: raw_id.to_string(),
                r#ref: s.r#type.to_string(),
                refinement_type: expected_kind.to_owned(),
                signal_type: "Span".to_owned(),
            });
        }
    }
    if expected_kind != "Metric" {
        if let Some(m) = base_signals
            .metrics
            .iter()
            .find(|m| m.name == *parent_ref || m.name == *stripped_ref)
        {
            return Some(Error::InvalidRefinement {
                refinement_id: raw_id.to_string(),
                r#ref: m.name.to_string(),
                refinement_type: expected_kind.to_owned(),
                signal_type: "Metric".to_owned(),
            });
        }
    }
    if expected_kind != "Event" {
        if let Some(e) = base_signals
            .events
            .iter()
            .find(|e| e.name == *parent_ref || e.name == *stripped_ref)
        {
            return Some(Error::InvalidRefinement {
                refinement_id: raw_id.to_string(),
                r#ref: e.name.to_string(),
                refinement_type: expected_kind.to_owned(),
                signal_type: "Event".to_owned(),
            });
        }
    }
    if expected_kind != "Entity" {
        if let Some(en) = base_signals
            .entities
            .iter()
            .find(|en| en.r#type == *parent_ref || en.r#type == *stripped_ref)
        {
            return Some(Error::InvalidRefinement {
                refinement_id: raw_id.to_string(),
                r#ref: en.r#type.to_string(),
                refinement_type: expected_kind.to_owned(),
                signal_type: "Entity".to_owned(),
            });
        }
    }
    None
}

/// Categorized view of a single authored refinement attribute entry.
enum RefinementAttrBranch<'a> {
    Unref(&'a AttributeUnref),
    Group(SignalId),
    Attribute(&'a AttributeRef, Option<bool>),
}

/// Adapts metric/event, span, and entity description refinement attribute entries.
trait RefinementAttrEntry {
    /// Classifies this entry as an `unref`, `ref_group`, or inline attribute override.
    fn branch(&self) -> RefinementAttrBranch<'_>;
}

impl RefinementAttrEntry for RefinementAttributeOrGroupRef {
    fn branch(&self) -> RefinementAttrBranch<'_> {
        match self {
            Self::Unref(u) => RefinementAttrBranch::Unref(u),
            Self::Group(g) => RefinementAttrBranch::Group(g.ref_group.clone()),
            Self::Attribute(a) => RefinementAttrBranch::Attribute(a, None),
        }
    }
}

impl RefinementAttrEntry for SpanRefinementAttributeOrGroupRef {
    fn branch(&self) -> RefinementAttrBranch<'_> {
        match self {
            Self::Unref(u) => RefinementAttrBranch::Unref(u),
            Self::Group(g) => RefinementAttrBranch::Group(SignalId::from(g.ref_group.clone())),
            Self::Attribute(s) => RefinementAttrBranch::Attribute(&s.base, s.sampling_relevant),
        }
    }
}

impl RefinementAttrEntry for EntityAttributeRefinement {
    fn branch(&self) -> RefinementAttrBranch<'_> {
        match self {
            Self::Unref(u) => RefinementAttrBranch::Unref(u),
            Self::Ref(a) => RefinementAttrBranch::Attribute(a, None),
        }
    }
}

/// Partitioned `unref`s, `ref_group` IDs, and inline `(AttributeRef, sampling_relevant)` pairs.
struct PartitionedAttrs {
    unrefs: Vec<AttributeUnref>,
    ref_groups: Vec<SignalId>,
    inline_refs: Vec<(AttributeRef, Option<bool>)>,
}

/// Splits refinement attribute entries into `unref`s, `ref_group` IDs, and inline overrides.
fn partition_attributes<E: RefinementAttrEntry>(entries: &[E]) -> PartitionedAttrs {
    let mut unrefs = Vec::new();
    let mut ref_groups = Vec::new();
    let mut inline_refs = Vec::new();
    for entry in entries {
        match entry.branch() {
            RefinementAttrBranch::Unref(u) => unrefs.push(u.clone()),
            RefinementAttrBranch::Group(g) => ref_groups.push(g),
            RefinementAttrBranch::Attribute(a, samp) => inline_refs.push((a.clone(), samp)),
        }
    }
    PartitionedAttrs {
        unrefs,
        ref_groups,
        inline_refs,
    }
}

/// Expands `ref_groups` and merges `inline_refs` on top, returning merged items and group-origin map.
#[allow(clippy::too_many_arguments)]
fn prepare_refinement_attributes(
    ref_groups: &[SignalId],
    inline_refs: &[(AttributeRef, Option<bool>)],
    expanded_groups: &ExpandedGroups,
    consumer_id: &str,
    is_consumer_excluded: bool,
    is_span_consumer: bool,
    provenance: Option<&SemconvProvenance>,
    warnings: &mut Vec<Error>,
) -> Result<
    (
        BTreeMap<String, GroupAttributeItem>,
        HashMap<String, String>,
    ),
    Error,
> {
    let mut items_map = groups::collect_refinement_group_items(
        ref_groups,
        expanded_groups,
        consumer_id,
        is_consumer_excluded,
        is_span_consumer,
        provenance,
        warnings,
    )?;

    let mut included_group_attrs = HashMap::new();
    for item in items_map.values() {
        _ = included_group_attrs.insert(item.key.clone(), item.origin_group.to_string());
    }

    let consumer_sig = SignalId::from(consumer_id);
    for (aref, sampling_relevant) in inline_refs {
        groups::merge_inline_attribute(&mut items_map, &consumer_sig, aref, *sampling_relevant);
    }

    Ok((items_map, included_group_attrs))
}

/// Adapts resolved attribute tuples across non-span (`(Attribute, RequirementLevel)`) and span signals.
trait RefinedAttrSlot: Sized {
    /// Whether this attribute slot retains `sampling_relevant` flags.
    const IS_SPAN_SLOT: bool;
    /// Returns the resolved `Attribute` in this slot.
    fn attr(&self) -> &Attribute;
    /// Returns the `RequirementLevel` in this slot.
    fn req(&self) -> &RequirementLevel;
    /// Applies overrides from `item` onto this inherited attribute slot in place.
    fn apply_override(&mut self, item: &GroupAttributeItem);
    /// Constructs a new attribute slot from a newly referenced attribute.
    fn from_new(
        resolved: Attribute,
        req: RequirementLevel,
        sampling_relevant: Option<bool>,
    ) -> Self;
}

impl RefinedAttrSlot for (Attribute, RequirementLevel) {
    const IS_SPAN_SLOT: bool = false;
    fn attr(&self) -> &Attribute {
        &self.0
    }
    fn req(&self) -> &RequirementLevel {
        &self.1
    }
    fn apply_override(&mut self, item: &GroupAttributeItem) {
        self.0 = rules::apply_attribute_ref_overrides(&self.0, &item.aref);
        if item.aref.requirement_level.is_some() {
            self.1 = rules::resolve_requirement_level(&item.aref);
        }
    }
    fn from_new(
        resolved: Attribute,
        req: RequirementLevel,
        _sampling_relevant: Option<bool>,
    ) -> Self {
        (resolved, req)
    }
}

impl RefinedAttrSlot for (Attribute, RequirementLevel, Option<bool>) {
    const IS_SPAN_SLOT: bool = true;
    fn attr(&self) -> &Attribute {
        &self.0
    }
    fn req(&self) -> &RequirementLevel {
        &self.1
    }
    fn apply_override(&mut self, item: &GroupAttributeItem) {
        self.0 = rules::apply_attribute_ref_overrides(&self.0, &item.aref);
        if item.aref.requirement_level.is_some() {
            self.1 = rules::resolve_requirement_level(&item.aref);
        }
        if item.sampling_relevant.is_some() {
            self.2 = item.sampling_relevant;
        }
    }
    fn from_new(
        resolved: Attribute,
        req: RequirementLevel,
        sampling_relevant: Option<bool>,
    ) -> Self {
        (resolved, req, sampling_relevant)
    }
}

/// Applies merged refinement attributes onto `current_attrs` and registers resulting attributes in `catalog`.
fn apply_attribute_overrides<'a, A: RefinedAttrSlot, C: SchemaCacheLookup>(
    current_attrs: &mut Vec<A>,
    merged_items: BTreeMap<String, GroupAttributeItem>,
    consumer_id: &str,
    is_consumer_excluded: bool,
    catalog: &mut Catalog<'a, C>,
    provenance: Option<&SemconvProvenance>,
) -> Result<(), Error> {
    for (key, item) in merged_items {
        if let Some(existing) = current_attrs.iter_mut().find(|slot| slot.attr().key == key) {
            existing.apply_override(&item);
        } else {
            let resolved = catalog.resolve_attribute_ref(
                &item.aref,
                consumer_id,
                is_consumer_excluded,
                provenance,
            )?;
            let req = rules::resolve_requirement_level(&item.aref);
            current_attrs.push(A::from_new(resolved, req, item.sampling_relevant));
        }
    }
    for slot in current_attrs.iter() {
        catalog.register_attribute(slot.attr().clone());
    }
    Ok(())
}

/// Validates `unref`s, filters inherited attributes, applies group and inline overrides, and sorts by key.
#[allow(clippy::too_many_arguments)]
fn resolve_refined_attributes<'a, E, A, C>(
    parent_attrs: Vec<A>,
    entries: &[E],
    parent_identifying_keys: &HashSet<String>,
    refinement_id: &SignalId,
    parent_ref: &SignalId,
    used_in: &str,
    is_ref_excluded: bool,
    provenance: &SemconvProvenance,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    warnings: &mut Vec<Error>,
) -> Result<Vec<A>, Vec<Error>>
where
    E: RefinementAttrEntry,
    A: RefinedAttrSlot,
    C: SchemaCacheLookup,
{
    let partitioned = partition_attributes(entries);
    let (merged_items, included_group_attrs) = prepare_refinement_attributes(
        &partitioned.ref_groups,
        &partitioned.inline_refs,
        expanded_groups,
        used_in,
        is_ref_excluded,
        A::IS_SPAN_SLOT,
        Some(provenance),
        warnings,
    )
    .map_err(|e| vec![e])?;

    let parent_attr_keys: HashSet<String> =
        parent_attrs.iter().map(|s| s.attr().key.clone()).collect();
    let parent_required_keys: HashSet<String> = parent_attrs
        .iter()
        .filter(|s| is_required(s.req()))
        .map(|s| s.attr().key.clone())
        .collect();
    let local_attr_keys: HashSet<String> = partitioned
        .inline_refs
        .iter()
        .map(|(a, _)| a.r#ref.clone())
        .collect();

    let unref_errors = rules::validate_attribute_unrefs(
        refinement_id.as_ref(),
        parent_ref.as_ref(),
        &partitioned.unrefs,
        &parent_attr_keys,
        &parent_required_keys,
        parent_identifying_keys,
        &local_attr_keys,
        &included_group_attrs,
        Some(provenance),
    );
    if !unref_errors.is_empty() {
        return Err(unref_errors);
    }

    let unref_set: HashSet<&str> = partitioned
        .unrefs
        .iter()
        .map(|u| u.unref.as_str())
        .collect();
    let mut current_attrs: Vec<A> = parent_attrs
        .into_iter()
        .filter(|slot| !unref_set.contains(slot.attr().key.as_str()))
        .collect();

    apply_attribute_overrides(
        &mut current_attrs,
        merged_items,
        used_in,
        is_ref_excluded,
        catalog,
        Some(provenance),
    )
    .map_err(|e| vec![e])?;

    current_attrs.sort_by(|a, b| a.attr().key.cmp(&b.attr().key));
    Ok(current_attrs)
}

/// Inherits `CommonFields` from `parent_common` with refinement overrides applied.
fn inherit_common_fields(
    brief: Option<&str>,
    note: &Option<String>,
    stability: &Option<Stability>,
    deprecated: &Option<Deprecated>,
    annotations: &BTreeMap<String, YamlValue>,
    parent_common: &CommonFields,
) -> CommonFields {
    CommonFields {
        brief: rules::inherit_brief(brief, &parent_common.brief),
        note: rules::inherit_note(note, &parent_common.note),
        stability: rules::inherit_stability(stability, &parent_common.stability),
        deprecated: rules::inherit_deprecated(deprecated, &parent_common.deprecated),
        annotations: rules::inherit_annotations(&parent_common.annotations, annotations),
    }
}

/// Replaces parent entity associations when `override_assocs` is `Some`, or inherits parent associations when `None`.
fn inherit_entity_associations(
    override_assocs: Option<&[SemconvEntityAssociation]>,
    parent_assocs: EntityAssociations,
    parent_origins: BTreeMap<String, SchemaUrl>,
) -> (EntityAssociations, BTreeMap<String, SchemaUrl>) {
    match override_assocs {
        None => (parent_assocs, parent_origins),
        Some(list) => (EntityAssociations::Raw(list.to_vec()), BTreeMap::new()),
    }
}

/// Resolved parent signal paired with its dependency-resolution exclusion flag.
struct ParentSnapshot<T> {
    signal: T,
    excluded: bool,
}

/// Per-signal-kind specification driving `resolve_kind_refinements`.
trait RefinementKindSpec {
    /// Lowercase signal kind prefix (`metric`, `span`, `event`, or `entity`).
    const KIND_LOWER: &'static str;
    /// Capitalized signal kind label used in `InvalidRefinement` diagnostics.
    const KIND_TITLE: &'static str;
    /// Authored refinement AST type in `Index`.
    type Raw;
    /// Resolved signal type used for both inherited parent snapshots and refinement outputs.
    type Resolved: Clone;

    /// Returns the local symbol table of refinements for this kind.
    fn indexed_map(index: &Index) -> &BTreeMap<SignalId, Indexed<Self::Raw>>;
    /// Returns the parent `ref` target declared on `raw`.
    fn parent_ref(raw: &Self::Raw) -> &SignalId;
    /// Returns the authored annotations map on `raw`.
    fn annotations(raw: &Self::Raw) -> &BTreeMap<String, YamlValue>;
    /// Looks up a matching parent in local `base_signals`.
    fn find_local_base<'a>(
        base_signals: &'a ResolvedSignals,
        parent_ref: &SignalId,
        stripped_ref: &SignalId,
    ) -> Option<&'a Self::Resolved>;
    /// Returns `true` if `resolved` carries `dependency_resolution.exclude: true`.
    fn is_resolved_excluded(resolved: &Self::Resolved) -> bool;
    /// Extracts a `ParentSnapshot` from a dependency `DepSignal`, returning `None` on kind mismatch.
    fn parent_from_dep<C: SchemaCacheLookup>(
        dep_signal: DepSignal,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<Option<ParentSnapshot<Self::Resolved>>, Error>;

    /// Validates and merges `raw` onto `parent` to produce the resolved refinement.
    #[allow(clippy::too_many_arguments)]
    fn apply_refinement<C: SchemaCacheLookup>(
        raw_id: &SignalId,
        refinement_id: &SignalId,
        raw: &Self::Raw,
        parent: Self::Resolved,
        is_ref_excluded: bool,
        provenance: &SemconvProvenance,
        schema_provenance: SchemaProvenance,
        expanded_groups: &ExpandedGroups,
        catalog: &mut Catalog<'_, C>,
        warnings: &mut Vec<Error>,
    ) -> Result<Self::Resolved, Vec<Error>>;
}

/// Iteratively resolves refinements of kind `S` against local bases, sibling refinements, and dependencies.
fn resolve_kind_refinements<'a, S: RefinementKindSpec, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<Vec<(SignalId, S::Resolved)>, Error> {
    let mut resolved_list: Vec<(SignalId, S::Resolved)> = Vec::new();
    let mut unresolved: Vec<(&SignalId, &Indexed<S::Raw>)> = S::indexed_map(index).iter().collect();
    let mut fatal_errors = Vec::new();

    while !unresolved.is_empty() {
        let mut progress = false;
        let mut still_unresolved: Vec<(&SignalId, &Indexed<S::Raw>)> = Vec::new();

        for (raw_id, indexed) in &unresolved {
            let raw = &indexed.item;
            let refinement_id = rules::normalize_refinement_id(S::KIND_LOWER, raw_id);
            let schema_provenance = SchemaProvenance {
                source: None,
                path: indexed.provenance.path.clone(),
            };
            let is_ref_excluded = is_excluded(S::annotations(raw));
            let parent_ref = S::parent_ref(raw);
            let stripped_parent_ref = rules::normalize_refinement_id(S::KIND_LOWER, parent_ref);

            let parent = if let Some(base) =
                S::find_local_base(base_signals, parent_ref, &stripped_parent_ref)
            {
                ParentSnapshot {
                    excluded: S::is_resolved_excluded(base),
                    signal: base.clone(),
                }
            } else if let Some((_, prev)) = resolved_list
                .iter()
                .find(|(id, _)| id == parent_ref || *id == stripped_parent_ref)
            {
                ParentSnapshot {
                    excluded: S::is_resolved_excluded(prev),
                    signal: prev.clone(),
                }
            } else {
                let is_parent_unresolved = unresolved.iter().any(|(other_id, _)| {
                    if *other_id == *raw_id {
                        return false;
                    }
                    let nid = rules::normalize_refinement_id(S::KIND_LOWER, other_id);
                    nid == *parent_ref || nid == stripped_parent_ref || *other_id == parent_ref
                });
                if is_parent_unresolved {
                    still_unresolved.push((*raw_id, *indexed));
                    continue;
                }

                if let Some(err) = check_wrong_kind_in_base(
                    parent_ref,
                    &stripped_parent_ref,
                    raw_id,
                    S::KIND_TITLE,
                    base_signals,
                ) {
                    fatal_errors.push(err);
                    continue;
                }

                match deps.lookup_signal(
                    S::KIND_LOWER,
                    parent_ref.as_ref(),
                    raw_id.as_ref(),
                    Some(&indexed.provenance),
                    cache_lookup,
                ) {
                    Ok(dep_signal) => {
                        let Some(dep_parent) = S::parent_from_dep(dep_signal, catalog)? else {
                            fatal_errors.push(Error::UnresolvedRefinementParent {
                                refinement_id: raw_id.to_string(),
                                r#ref: parent_ref.to_string(),
                                provenance: Some(Box::new(indexed.provenance.clone())),
                            });
                            continue;
                        };
                        dep_parent
                    }
                    Err(e) => {
                        fatal_errors.push(e);
                        continue;
                    }
                }
            };

            if parent.excluded && !is_ref_excluded {
                fatal_errors.push(Error::ExcludedFromDependencyResolution {
                    id: parent_ref.to_string(),
                    r#type: S::KIND_LOWER.to_owned(),
                    used_in: raw_id.to_string(),
                    provenance: Some(Box::new(indexed.provenance.clone())),
                });
                continue;
            }

            match S::apply_refinement(
                raw_id,
                &refinement_id,
                raw,
                parent.signal,
                is_ref_excluded,
                &indexed.provenance,
                schema_provenance,
                expanded_groups,
                catalog,
                warnings,
            ) {
                Ok(resolved) => {
                    resolved_list.push((refinement_id, resolved));
                    progress = true;
                }
                Err(errs) => {
                    fatal_errors.extend(errs);
                }
            }
        }

        if !progress && !still_unresolved.is_empty() {
            for (raw_id, indexed) in still_unresolved {
                fatal_errors.push(Error::UnresolvedRefinementParent {
                    refinement_id: raw_id.to_string(),
                    r#ref: S::parent_ref(&indexed.item).to_string(),
                    provenance: Some(Box::new(indexed.provenance.clone())),
                });
            }
            break;
        }

        unresolved = still_unresolved;
    }

    handle_errors(fatal_errors)?;
    resolved_list.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(resolved_list)
}

/// Metric refinement resolution rules.
struct MetricRefinementSpec;

impl RefinementKindSpec for MetricRefinementSpec {
    const KIND_LOWER: &'static str = "metric";
    const KIND_TITLE: &'static str = "Metric";
    type Raw = MetricRefinement;
    type Resolved = ResolvedMetric;

    fn indexed_map(index: &Index) -> &BTreeMap<SignalId, Indexed<Self::Raw>> {
        index.metric_refinements()
    }

    fn parent_ref(raw: &Self::Raw) -> &SignalId {
        &raw.r#ref
    }

    fn annotations(raw: &Self::Raw) -> &BTreeMap<String, YamlValue> {
        &raw.annotations
    }

    fn find_local_base<'a>(
        base_signals: &'a ResolvedSignals,
        parent_ref: &SignalId,
        stripped_ref: &SignalId,
    ) -> Option<&'a Self::Resolved> {
        base_signals
            .metrics
            .iter()
            .find(|m| m.name == *parent_ref || m.name == *stripped_ref)
    }

    fn is_resolved_excluded(m: &Self::Resolved) -> bool {
        is_excluded(&m.common.annotations)
    }

    fn parent_from_dep<C: SchemaCacheLookup>(
        dep_signal: DepSignal,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<Option<ParentSnapshot<Self::Resolved>>, Error> {
        let DepSignalPayload::Metric {
            name,
            instrument,
            unit,
            requirement_level,
            attributes,
        } = dep_signal.payload
        else {
            return Ok(None);
        };
        let mut resolved_attrs = Vec::with_capacity(attributes.len());
        for (attr, req, origin) in attributes {
            let mapped_attr = catalog.register_inherited_dep_attribute(attr, &origin)?;
            resolved_attrs.push((mapped_attr, req));
        }
        Ok(Some(ParentSnapshot {
            signal: ResolvedMetric {
                name,
                instrument,
                unit,
                attributes: resolved_attrs,
                entity_associations: dep_signal.entity_associations,
                association_origins: dep_signal.association_origins,
                requirement_level,
                common: dep_signal.common,
                provenance: SchemaProvenance::default(),
            },
            excluded: dep_signal.excluded,
        }))
    }

    fn apply_refinement<C: SchemaCacheLookup>(
        _raw_id: &SignalId,
        refinement_id: &SignalId,
        m_ref: &Self::Raw,
        parent: Self::Resolved,
        is_ref_excluded: bool,
        provenance: &SemconvProvenance,
        schema_provenance: SchemaProvenance,
        expanded_groups: &ExpandedGroups,
        catalog: &mut Catalog<'_, C>,
        warnings: &mut Vec<Error>,
    ) -> Result<Self::Resolved, Vec<Error>> {
        let used_in = format!("metric.{refinement_id}");
        let empty_ident = HashSet::new();
        let attributes = resolve_refined_attributes(
            parent.attributes,
            &m_ref.attributes,
            &empty_ident,
            refinement_id,
            &m_ref.r#ref,
            &used_in,
            is_ref_excluded,
            provenance,
            expanded_groups,
            catalog,
            warnings,
        )?;
        let (entity_associations, association_origins) = inherit_entity_associations(
            m_ref.entity_associations.as_deref(),
            parent.entity_associations,
            parent.association_origins,
        );
        let common = inherit_common_fields(
            m_ref.brief.as_deref(),
            &m_ref.note,
            &m_ref.stability,
            &m_ref.deprecated,
            &m_ref.annotations,
            &parent.common,
        );
        Ok(ResolvedMetric {
            name: parent.name,
            instrument: parent.instrument,
            unit: parent.unit,
            attributes,
            entity_associations,
            association_origins,
            requirement_level: parent.requirement_level,
            common,
            provenance: schema_provenance,
        })
    }
}

/// Span refinement resolution rules.
struct SpanRefinementSpec;

impl RefinementKindSpec for SpanRefinementSpec {
    const KIND_LOWER: &'static str = "span";
    const KIND_TITLE: &'static str = "Span";
    type Raw = SpanRefinement;
    type Resolved = ResolvedSpan;

    fn indexed_map(index: &Index) -> &BTreeMap<SignalId, Indexed<Self::Raw>> {
        index.span_refinements()
    }

    fn parent_ref(raw: &Self::Raw) -> &SignalId {
        &raw.r#ref
    }

    fn annotations(raw: &Self::Raw) -> &BTreeMap<String, YamlValue> {
        &raw.annotations
    }

    fn find_local_base<'a>(
        base_signals: &'a ResolvedSignals,
        parent_ref: &SignalId,
        stripped_ref: &SignalId,
    ) -> Option<&'a Self::Resolved> {
        base_signals
            .spans
            .iter()
            .find(|s| s.r#type == *parent_ref || s.r#type == *stripped_ref)
    }

    fn is_resolved_excluded(s: &Self::Resolved) -> bool {
        is_excluded(&s.common.annotations)
    }

    fn parent_from_dep<C: SchemaCacheLookup>(
        dep_signal: DepSignal,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<Option<ParentSnapshot<Self::Resolved>>, Error> {
        let DepSignalPayload::Span {
            r#type,
            kind,
            name,
            requirement_level,
            attributes,
        } = dep_signal.payload
        else {
            return Ok(None);
        };
        let mut resolved_attrs = Vec::with_capacity(attributes.len());
        for (attr, req, samp, origin) in attributes {
            let mapped_attr = catalog.register_inherited_dep_attribute(attr, &origin)?;
            resolved_attrs.push((mapped_attr, req, samp));
        }
        Ok(Some(ParentSnapshot {
            signal: ResolvedSpan {
                r#type,
                kind,
                name,
                attributes: resolved_attrs,
                entity_associations: dep_signal.entity_associations,
                association_origins: dep_signal.association_origins,
                requirement_level,
                common: dep_signal.common,
                provenance: SchemaProvenance::default(),
            },
            excluded: dep_signal.excluded,
        }))
    }

    fn apply_refinement<C: SchemaCacheLookup>(
        raw_id: &SignalId,
        refinement_id: &SignalId,
        s_ref: &Self::Raw,
        parent: Self::Resolved,
        is_ref_excluded: bool,
        provenance: &SemconvProvenance,
        schema_provenance: SchemaProvenance,
        expanded_groups: &ExpandedGroups,
        catalog: &mut Catalog<'_, C>,
        warnings: &mut Vec<Error>,
    ) -> Result<Self::Resolved, Vec<Error>> {
        let used_in = format!("span.{refinement_id}");
        let empty_ident = HashSet::new();
        let attributes = resolve_refined_attributes(
            parent.attributes,
            &s_ref.attributes,
            &empty_ident,
            refinement_id,
            &s_ref.r#ref,
            &used_in,
            is_ref_excluded,
            provenance,
            expanded_groups,
            catalog,
            warnings,
        )?;

        let name = rules::inherit_span_name(&s_ref.name, &parent.name);
        let span_attr_keys: HashSet<&str> =
            attributes.iter().map(|(a, _, _)| a.key.as_str()).collect();
        for template in &name.templates {
            for attr_key in &template.attributes {
                if !span_attr_keys.contains(attr_key.as_str()) {
                    return Err(vec![Error::SpanNameAttributeNotOnSpan {
                        span_id: raw_id.to_string(),
                        attribute_key: attr_key.clone(),
                        provenance: Some(Box::new(provenance.clone())),
                    }]);
                }
            }
        }

        let (entity_associations, association_origins) = inherit_entity_associations(
            s_ref.entity_associations.as_deref(),
            parent.entity_associations,
            parent.association_origins,
        );
        let common = inherit_common_fields(
            s_ref.brief.as_deref(),
            &s_ref.note,
            &s_ref.stability,
            &s_ref.deprecated,
            &s_ref.annotations,
            &parent.common,
        );
        Ok(ResolvedSpan {
            r#type: parent.r#type,
            kind: parent.kind,
            name,
            attributes,
            entity_associations,
            association_origins,
            requirement_level: parent.requirement_level,
            common,
            provenance: schema_provenance,
        })
    }
}

/// Event refinement resolution rules.
struct EventRefinementSpec;

impl RefinementKindSpec for EventRefinementSpec {
    const KIND_LOWER: &'static str = "event";
    const KIND_TITLE: &'static str = "Event";
    type Raw = EventRefinement;
    type Resolved = ResolvedEvent;

    fn indexed_map(index: &Index) -> &BTreeMap<SignalId, Indexed<Self::Raw>> {
        index.event_refinements()
    }

    fn parent_ref(raw: &Self::Raw) -> &SignalId {
        &raw.r#ref
    }

    fn annotations(raw: &Self::Raw) -> &BTreeMap<String, YamlValue> {
        &raw.annotations
    }

    fn find_local_base<'a>(
        base_signals: &'a ResolvedSignals,
        parent_ref: &SignalId,
        stripped_ref: &SignalId,
    ) -> Option<&'a Self::Resolved> {
        base_signals
            .events
            .iter()
            .find(|e| e.name == *parent_ref || e.name == *stripped_ref)
    }

    fn is_resolved_excluded(e: &Self::Resolved) -> bool {
        is_excluded(&e.common.annotations)
    }

    fn parent_from_dep<C: SchemaCacheLookup>(
        dep_signal: DepSignal,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<Option<ParentSnapshot<Self::Resolved>>, Error> {
        let DepSignalPayload::Event {
            name,
            requirement_level,
            attributes,
        } = dep_signal.payload
        else {
            return Ok(None);
        };
        let mut resolved_attrs = Vec::with_capacity(attributes.len());
        for (attr, req, origin) in attributes {
            let mapped_attr = catalog.register_inherited_dep_attribute(attr, &origin)?;
            resolved_attrs.push((mapped_attr, req));
        }
        Ok(Some(ParentSnapshot {
            signal: ResolvedEvent {
                name,
                attributes: resolved_attrs,
                entity_associations: dep_signal.entity_associations,
                association_origins: dep_signal.association_origins,
                requirement_level,
                common: dep_signal.common,
                provenance: SchemaProvenance::default(),
            },
            excluded: dep_signal.excluded,
        }))
    }

    fn apply_refinement<C: SchemaCacheLookup>(
        _raw_id: &SignalId,
        refinement_id: &SignalId,
        e_ref: &Self::Raw,
        parent: Self::Resolved,
        is_ref_excluded: bool,
        provenance: &SemconvProvenance,
        schema_provenance: SchemaProvenance,
        expanded_groups: &ExpandedGroups,
        catalog: &mut Catalog<'_, C>,
        warnings: &mut Vec<Error>,
    ) -> Result<Self::Resolved, Vec<Error>> {
        let used_in = format!("event.{refinement_id}");
        let empty_ident = HashSet::new();
        let attributes = resolve_refined_attributes(
            parent.attributes,
            &e_ref.attributes,
            &empty_ident,
            refinement_id,
            &e_ref.r#ref,
            &used_in,
            is_ref_excluded,
            provenance,
            expanded_groups,
            catalog,
            warnings,
        )?;
        let (entity_associations, association_origins) = inherit_entity_associations(
            e_ref.entity_associations.as_deref(),
            parent.entity_associations,
            parent.association_origins,
        );
        let common = inherit_common_fields(
            e_ref.brief.as_deref(),
            &e_ref.note,
            &e_ref.stability,
            &e_ref.deprecated,
            &e_ref.annotations,
            &parent.common,
        );
        Ok(ResolvedEvent {
            name: parent.name,
            attributes,
            entity_associations,
            association_origins,
            requirement_level: parent.requirement_level,
            common,
            provenance: schema_provenance,
        })
    }
}

/// Entity refinement resolution rules, enforcing identity attribute preservation.
struct EntityRefinementSpec;

impl RefinementKindSpec for EntityRefinementSpec {
    const KIND_LOWER: &'static str = "entity";
    const KIND_TITLE: &'static str = "Entity";
    type Raw = EntityRefinement;
    type Resolved = ResolvedEntity;

    fn indexed_map(index: &Index) -> &BTreeMap<SignalId, Indexed<Self::Raw>> {
        index.entity_refinements()
    }

    fn parent_ref(raw: &Self::Raw) -> &SignalId {
        &raw.r#ref
    }

    fn annotations(raw: &Self::Raw) -> &BTreeMap<String, YamlValue> {
        &raw.annotations
    }

    fn find_local_base<'a>(
        base_signals: &'a ResolvedSignals,
        parent_ref: &SignalId,
        stripped_ref: &SignalId,
    ) -> Option<&'a Self::Resolved> {
        base_signals
            .entities
            .iter()
            .find(|en| en.r#type == *parent_ref || en.r#type == *stripped_ref)
    }

    fn is_resolved_excluded(en: &Self::Resolved) -> bool {
        is_excluded(&en.common.annotations)
    }

    fn parent_from_dep<C: SchemaCacheLookup>(
        dep_signal: DepSignal,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<Option<ParentSnapshot<Self::Resolved>>, Error> {
        let DepSignalPayload::Entity {
            r#type,
            identity,
            description,
            requirement_level,
        } = dep_signal.payload
        else {
            return Ok(None);
        };
        let mut resolved_ident = Vec::with_capacity(identity.len());
        for (attr, origin) in identity {
            let mapped_attr = catalog.register_inherited_dep_attribute(attr, &origin)?;
            resolved_ident.push(mapped_attr);
        }
        let mut resolved_desc = Vec::with_capacity(description.len());
        for (attr, req, origin) in description {
            let mapped_attr = catalog.register_inherited_dep_attribute(attr, &origin)?;
            resolved_desc.push((mapped_attr, req));
        }
        Ok(Some(ParentSnapshot {
            signal: ResolvedEntity {
                r#type,
                identity: resolved_ident,
                description: resolved_desc,
                requirement_level,
                common: dep_signal.common,
                provenance: SchemaProvenance::default(),
            },
            excluded: dep_signal.excluded,
        }))
    }

    fn apply_refinement<C: SchemaCacheLookup>(
        _raw_id: &SignalId,
        refinement_id: &SignalId,
        en_ref: &Self::Raw,
        parent: Self::Resolved,
        is_ref_excluded: bool,
        provenance: &SemconvProvenance,
        schema_provenance: SchemaProvenance,
        expanded_groups: &ExpandedGroups,
        catalog: &mut Catalog<'_, C>,
        warnings: &mut Vec<Error>,
    ) -> Result<Self::Resolved, Vec<Error>> {
        let identity_ref_keys: Vec<String> =
            en_ref.identity.iter().map(|i| i.r#ref.clone()).collect();
        let desc_keys: Vec<String> = en_ref
            .description
            .iter()
            .filter_map(|item| match item {
                EntityAttributeRefinement::Ref(a) => Some(a.r#ref.clone()),
                EntityAttributeRefinement::Unref(_) => None,
            })
            .collect();

        let parent_ident_keys: HashSet<String> =
            parent.identity.iter().map(|a| a.key.clone()).collect();
        let identity_errors = rules::validate_entity_identity_refinements(
            refinement_id.as_ref(),
            en_ref.r#ref.as_ref(),
            &parent_ident_keys,
            &identity_ref_keys,
            &desc_keys,
            Some(provenance),
        );
        if !identity_errors.is_empty() {
            return Err(identity_errors);
        }

        let mut identity_attrs = parent.identity;
        for id_ref in &en_ref.identity {
            let fake_aref = AttributeRef {
                r#ref: id_ref.r#ref.clone(),
                brief: id_ref.brief.clone(),
                examples: id_ref.examples.clone(),
                note: id_ref.note.clone(),
                requirement_level: None,
                annotations: id_ref.annotations.clone(),
            };
            if let Some(existing) = identity_attrs.iter_mut().find(|a| a.key == id_ref.r#ref) {
                *existing = rules::apply_attribute_ref_overrides(existing, &fake_aref);
            }
        }
        for attr in &identity_attrs {
            catalog.register_attribute(attr.clone());
        }

        let used_in = format!("entity.{refinement_id}");
        let description = resolve_refined_attributes(
            parent.description,
            &en_ref.description,
            &parent_ident_keys,
            refinement_id,
            &en_ref.r#ref,
            &used_in,
            is_ref_excluded,
            provenance,
            expanded_groups,
            catalog,
            warnings,
        )?;

        identity_attrs.sort_by(|a, b| a.key.cmp(&b.key));

        let common = inherit_common_fields(
            en_ref.brief.as_deref(),
            &en_ref.note,
            &en_ref.stability,
            &en_ref.deprecated,
            &en_ref.annotations,
            &parent.common,
        );
        Ok(ResolvedEntity {
            r#type: parent.r#type,
            identity: identity_attrs,
            description,
            requirement_level: parent.requirement_level,
            common,
            provenance: schema_provenance,
        })
    }
}

/// Resolves all metric, span, event, and entity refinements in `index`.
pub(crate) fn resolve<'a, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<ResolvedRefinements, Error> {
    let metrics = resolve_kind_refinements::<MetricRefinementSpec, _>(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let spans = resolve_kind_refinements::<SpanRefinementSpec, _>(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let events = resolve_kind_refinements::<EventRefinementSpec, _>(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let entities = resolve_kind_refinements::<EntityRefinementSpec, _>(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;

    Ok(ResolvedRefinements {
        metrics,
        spans,
        events,
        entities,
    })
}
