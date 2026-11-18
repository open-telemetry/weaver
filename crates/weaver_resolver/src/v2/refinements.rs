// SPDX-License-Identifier: Apache-2.0

//! Resolution of V2 signal refinements: metrics, spans, events, and entities.

use std::collections::{BTreeMap, HashMap, HashSet};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::{
    AttributeRef, AttributeUnref, BasicRequirementLevelSpec, RefinementAttributeOrGroupRef,
    RequirementLevel,
};
use weaver_semconv::v2::entity::EntityAttributeRefinement;
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::span::{
    SpanAttributeRef, SpanKindSpec, SpanName, SpanRefinementAttributeOrGroupRef,
};
use weaver_semconv::v2::CommonFields;

use super::attributes::Catalog;
use super::dependency::{DepSignalPayload, V2Dependencies};
use super::groups::{self, ExpandedGroups, GroupAttributeItem};
use super::index::{
    Index, IndexedEntityRefinement, IndexedEventRefinement, IndexedMetricRefinement,
    IndexedSpanRefinement,
};
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

/// Splits metric or event refinement attribute entries into `unref`s, `ref_group` IDs, and inline `AttributeRef`s.
fn partition_metric_attributes(
    attrs: &[RefinementAttributeOrGroupRef],
) -> (Vec<&AttributeUnref>, Vec<SignalId>, Vec<AttributeRef>) {
    let mut unrefs = Vec::new();
    let mut ref_groups = Vec::new();
    let mut inline_refs = Vec::new();
    for entry in attrs {
        match entry {
            RefinementAttributeOrGroupRef::Unref(u) => unrefs.push(u),
            RefinementAttributeOrGroupRef::Group(g) => ref_groups.push(g.ref_group.clone()),
            RefinementAttributeOrGroupRef::Attribute(a) => inline_refs.push(a.clone()),
        }
    }
    (unrefs, ref_groups, inline_refs)
}

/// Splits span refinement attribute entries into `unref`s, `ref_group` IDs, and inline `SpanAttributeRef`s.
fn partition_span_attributes(
    attrs: &[SpanRefinementAttributeOrGroupRef],
) -> (Vec<&AttributeUnref>, Vec<SignalId>, Vec<SpanAttributeRef>) {
    let mut unrefs = Vec::new();
    let mut ref_groups = Vec::new();
    let mut inline_refs = Vec::new();
    for entry in attrs {
        match entry {
            SpanRefinementAttributeOrGroupRef::Unref(u) => unrefs.push(u),
            SpanRefinementAttributeOrGroupRef::Group(g) => {
                ref_groups.push(SignalId::from(g.ref_group.clone()));
            }
            SpanRefinementAttributeOrGroupRef::Attribute(a) => inline_refs.push(a.clone()),
        }
    }
    (unrefs, ref_groups, inline_refs)
}

/// Expands `ref_groups` and merges `inline_refs` on top, returning merged items and group-origin map.
fn prepare_refinement_attributes(
    ref_groups: &[SignalId],
    inline_refs: &[AttributeRef],
    expanded_groups: &ExpandedGroups,
    consumer_id: &str,
    is_consumer_excluded: bool,
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
        provenance,
        warnings,
    )?;

    let mut included_group_attrs = HashMap::new();
    for item in items_map.values() {
        _ = included_group_attrs.insert(item.key.clone(), item.origin_group.to_string());
    }

    for aref in inline_refs {
        if let Some(existing) = items_map.get_mut(&aref.r#ref) {
            existing.aref = rules::merge_attribute_refs(&existing.aref, aref);
            existing.origin_group = SignalId::from(consumer_id);
        } else {
            _ = items_map.insert(
                aref.r#ref.clone(),
                GroupAttributeItem {
                    key: aref.r#ref.clone(),
                    aref: aref.clone(),
                    sampling_relevant: None,
                    origin_group: SignalId::from(consumer_id),
                },
            );
        }
    }

    Ok((items_map, included_group_attrs))
}

/// Expands `ref_groups` and merges span `inline_refs` on top, preserving `sampling_relevant`.
fn prepare_span_refinement_attributes(
    ref_groups: &[SignalId],
    inline_refs: &[SpanAttributeRef],
    expanded_groups: &ExpandedGroups,
    consumer_id: &str,
    is_consumer_excluded: bool,
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
        provenance,
        warnings,
    )?;

    let mut included_group_attrs = HashMap::new();
    for item in items_map.values() {
        _ = included_group_attrs.insert(item.key.clone(), item.origin_group.to_string());
    }

    for sref in inline_refs {
        if let Some(existing) = items_map.get_mut(&sref.base.r#ref) {
            existing.aref = rules::merge_attribute_refs(&existing.aref, &sref.base);
            existing.sampling_relevant = sref.sampling_relevant.or(existing.sampling_relevant);
            existing.origin_group = SignalId::from(consumer_id);
        } else {
            _ = items_map.insert(
                sref.base.r#ref.clone(),
                GroupAttributeItem {
                    key: sref.base.r#ref.clone(),
                    aref: sref.base.clone(),
                    sampling_relevant: sref.sampling_relevant,
                    origin_group: SignalId::from(consumer_id),
                },
            );
        }
    }

    Ok((items_map, included_group_attrs))
}

/// Applies merged refinement attributes onto `current_attrs` and registers resulting attributes in `catalog`.
fn apply_attribute_overrides<'a, C: SchemaCacheLookup>(
    current_attrs: &mut Vec<(Attribute, RequirementLevel)>,
    merged_items: BTreeMap<String, GroupAttributeItem>,
    consumer_id: &str,
    is_consumer_excluded: bool,
    catalog: &mut Catalog<'a, C>,
    provenance: Option<&SemconvProvenance>,
) -> Result<(), Error> {
    for (key, item) in merged_items {
        if let Some((existing_attr, existing_req)) =
            current_attrs.iter_mut().find(|(a, _)| a.key == key)
        {
            *existing_attr = rules::apply_attribute_ref_overrides(existing_attr, &item.aref);
            if item.aref.requirement_level.is_some() {
                *existing_req = rules::resolve_requirement_level(&item.aref);
            }
        } else {
            let resolved = catalog.resolve_attribute_ref(
                &item.aref,
                consumer_id,
                false,
                is_consumer_excluded,
                provenance,
            )?;
            let req = rules::resolve_requirement_level(&item.aref);
            current_attrs.push((resolved, req));
        }
    }
    for (attr, _) in current_attrs.iter() {
        catalog.register_attribute(attr.clone());
    }
    Ok(())
}

/// Applies merged span refinement attributes onto `current_attrs` and registers resulting attributes in `catalog`.
fn apply_span_attribute_overrides<'a, C: SchemaCacheLookup>(
    current_attrs: &mut Vec<(Attribute, RequirementLevel, Option<bool>)>,
    merged_items: BTreeMap<String, GroupAttributeItem>,
    consumer_id: &str,
    is_consumer_excluded: bool,
    catalog: &mut Catalog<'a, C>,
    provenance: Option<&SemconvProvenance>,
) -> Result<(), Error> {
    for (key, item) in merged_items {
        if let Some((existing_attr, existing_req, existing_samp)) =
            current_attrs.iter_mut().find(|(a, _, _)| a.key == key)
        {
            *existing_attr = rules::apply_attribute_ref_overrides(existing_attr, &item.aref);
            if item.aref.requirement_level.is_some() {
                *existing_req = rules::resolve_requirement_level(&item.aref);
            }
            if item.sampling_relevant.is_some() {
                *existing_samp = item.sampling_relevant;
            }
        } else {
            let resolved = catalog.resolve_attribute_ref(
                &item.aref,
                consumer_id,
                false,
                is_consumer_excluded,
                provenance,
            )?;
            let req = rules::resolve_requirement_level(&item.aref);
            current_attrs.push((resolved, req, item.sampling_relevant));
        }
    }
    for (attr, _, _) in current_attrs.iter() {
        catalog.register_attribute(attr.clone());
    }
    Ok(())
}

/// Snapshot of a parent metric resolved for refinement inheritance.
struct MetricParent {
    name: SignalId,
    instrument: InstrumentSpec,
    unit: String,
    attributes: Vec<(Attribute, RequirementLevel)>,
    entity_associations: EntityAssociations,
    association_origins: BTreeMap<String, SchemaUrl>,
    requirement_level: Option<SignalRequirementLevel>,
    common: CommonFields,
    excluded: bool,
}

/// Iteratively resolves metric refinements against local bases, prior refinements, and dependencies.
fn resolve_metric_refinements<'a, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<Vec<(SignalId, ResolvedMetric)>, Error> {
    let mut resolved_metrics: Vec<(SignalId, ResolvedMetric)> = Vec::new();
    let mut unresolved: Vec<(&SignalId, &IndexedMetricRefinement)> =
        index.metric_refinements().iter().collect();
    let mut fatal_errors = Vec::new();

    while !unresolved.is_empty() {
        let mut progress = false;
        let mut still_unresolved: Vec<(&SignalId, &IndexedMetricRefinement)> = Vec::new();

        for (raw_id, indexed_m) in &unresolved {
            let m_ref = &indexed_m.refinement;
            let refinement_id = rules::normalize_refinement_id("metric", raw_id);
            let provenance = SchemaProvenance {
                source: None,
                path: indexed_m.provenance.path.clone(),
            };
            let is_ref_excluded = is_excluded(&m_ref.annotations);
            let parent_ref = &m_ref.r#ref;
            let stripped_parent_ref = rules::normalize_refinement_id("metric", parent_ref);

            let parent: MetricParent = if let Some(base) = base_signals
                .metrics
                .iter()
                .find(|m| m.name == *parent_ref || m.name == stripped_parent_ref)
            {
                MetricParent {
                    name: base.name.clone(),
                    instrument: base.instrument,
                    unit: base.unit.clone(),
                    attributes: base.attributes.clone(),
                    entity_associations: base.entity_associations.clone(),
                    association_origins: base.association_origins.clone(),
                    requirement_level: base.requirement_level.clone(),
                    common: base.common.clone(),
                    excluded: is_excluded(&base.common.annotations),
                }
            } else if let Some((_, prev)) = resolved_metrics
                .iter()
                .find(|(id, _)| id == parent_ref || *id == stripped_parent_ref)
            {
                MetricParent {
                    name: prev.name.clone(),
                    instrument: prev.instrument,
                    unit: prev.unit.clone(),
                    attributes: prev.attributes.clone(),
                    entity_associations: prev.entity_associations.clone(),
                    association_origins: prev.association_origins.clone(),
                    requirement_level: prev.requirement_level.clone(),
                    common: prev.common.clone(),
                    excluded: is_excluded(&prev.common.annotations),
                }
            } else {
                let is_parent_unresolved = unresolved.iter().any(|(other_id, _)| {
                    if *other_id == *raw_id {
                        return false;
                    }
                    let nid = rules::normalize_refinement_id("metric", other_id);
                    nid == *parent_ref || nid == stripped_parent_ref || *other_id == parent_ref
                });
                if is_parent_unresolved {
                    still_unresolved.push((*raw_id, *indexed_m));
                    continue;
                }

                if let Some(err) = check_wrong_kind_in_base(
                    parent_ref,
                    &stripped_parent_ref,
                    raw_id,
                    "Metric",
                    base_signals,
                ) {
                    fatal_errors.push(err);
                    continue;
                }

                match deps.lookup_signal(
                    "metric",
                    parent_ref.as_ref(),
                    raw_id.as_ref(),
                    Some(&indexed_m.provenance),
                    cache_lookup,
                ) {
                    Ok(dep_signal) => {
                        if let DepSignalPayload::Metric {
                            name,
                            instrument,
                            unit,
                            requirement_level,
                            attributes,
                        } = dep_signal.payload
                        {
                            let mut resolved_attrs = Vec::new();
                            for (attr, req, origin) in attributes {
                                let mapped_attr =
                                    catalog.register_inherited_dep_attribute(attr, &origin)?;
                                resolved_attrs.push((mapped_attr, req));
                            }
                            MetricParent {
                                name,
                                instrument,
                                unit,
                                attributes: resolved_attrs,
                                entity_associations: dep_signal.entity_associations,
                                association_origins: dep_signal.association_origins,
                                requirement_level,
                                common: dep_signal.common,
                                excluded: dep_signal.excluded,
                            }
                        } else {
                            fatal_errors.push(Error::UnresolvedRefinementParent {
                                refinement_id: raw_id.to_string(),
                                r#ref: parent_ref.to_string(),
                                provenance: Some(Box::new(indexed_m.provenance.clone())),
                            });
                            continue;
                        }
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
                    r#type: "metric".to_owned(),
                    used_in: raw_id.to_string(),
                    provenance: Some(Box::new(indexed_m.provenance.clone())),
                });
                continue;
            }

            let (unrefs, ref_groups, inline_refs) = partition_metric_attributes(&m_ref.attributes);
            let used_in = format!("metric.{refinement_id}");

            let (merged_items, included_group_attrs) = match prepare_refinement_attributes(
                &ref_groups,
                &inline_refs,
                expanded_groups,
                &used_in,
                is_ref_excluded,
                Some(&indexed_m.provenance),
                warnings,
            ) {
                Ok(res) => res,
                Err(e) => {
                    fatal_errors.push(e);
                    continue;
                }
            };

            let parent_attr_keys: HashSet<String> = parent
                .attributes
                .iter()
                .map(|(a, _)| a.key.clone())
                .collect();
            let parent_required_keys: HashSet<String> = parent
                .attributes
                .iter()
                .filter(|(_, req)| is_required(req))
                .map(|(a, _)| a.key.clone())
                .collect();
            let parent_identifying_keys = HashSet::new();
            let local_attr_keys: HashSet<String> =
                inline_refs.iter().map(|a| a.r#ref.clone()).collect();

            let unref_errors = rules::validate_attribute_unrefs(
                refinement_id.as_ref(),
                parent_ref.as_ref(),
                &unrefs.iter().map(|u| (*u).clone()).collect::<Vec<_>>(),
                &parent_attr_keys,
                &parent_required_keys,
                &parent_identifying_keys,
                &local_attr_keys,
                &included_group_attrs,
                Some(&indexed_m.provenance),
            );
            if !unref_errors.is_empty() {
                fatal_errors.extend(unref_errors);
                continue;
            }

            let unref_set: HashSet<&str> = unrefs.iter().map(|u| u.unref.as_str()).collect();
            let mut current_attrs: Vec<(Attribute, RequirementLevel)> = parent
                .attributes
                .into_iter()
                .filter(|(a, _)| !unref_set.contains(a.key.as_str()))
                .collect();

            if let Err(e) = apply_attribute_overrides(
                &mut current_attrs,
                merged_items,
                &used_in,
                is_ref_excluded,
                catalog,
                Some(&indexed_m.provenance),
            ) {
                fatal_errors.push(e);
                continue;
            }

            current_attrs.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

            let brief = rules::inherit_brief(m_ref.brief.as_deref(), &parent.common.brief);
            let note = rules::inherit_note(&m_ref.note, &parent.common.note);
            let stability = rules::inherit_stability(&m_ref.stability, &parent.common.stability);
            let deprecated =
                rules::inherit_deprecated(&m_ref.deprecated, &parent.common.deprecated);
            let annotations =
                rules::inherit_annotations(&parent.common.annotations, &m_ref.annotations);
            let (entity_associations, association_origins) = if m_ref.entity_associations.is_empty()
            {
                (parent.entity_associations, parent.association_origins)
            } else {
                (
                    EntityAssociations::Raw(m_ref.entity_associations.clone()),
                    BTreeMap::new(),
                )
            };

            let resolved = ResolvedMetric {
                name: parent.name,
                instrument: parent.instrument,
                unit: parent.unit,
                attributes: current_attrs,
                entity_associations,
                association_origins,
                requirement_level: parent.requirement_level,
                common: CommonFields {
                    brief,
                    note,
                    stability,
                    deprecated,
                    annotations,
                },
                provenance,
            };

            resolved_metrics.push((refinement_id, resolved));
            progress = true;
        }

        if !progress && !still_unresolved.is_empty() {
            for (raw_id, indexed_m) in still_unresolved {
                fatal_errors.push(Error::UnresolvedRefinementParent {
                    refinement_id: raw_id.to_string(),
                    r#ref: indexed_m.refinement.r#ref.to_string(),
                    provenance: Some(Box::new(indexed_m.provenance.clone())),
                });
            }
            break;
        }

        unresolved = still_unresolved;
    }

    handle_errors(fatal_errors)?;

    resolved_metrics.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(resolved_metrics)
}

/// Snapshot of a parent span resolved for refinement inheritance.
struct SpanParent {
    r#type: SignalId,
    kind: SpanKindSpec,
    name: SpanName,
    attributes: Vec<(Attribute, RequirementLevel, Option<bool>)>,
    entity_associations: EntityAssociations,
    association_origins: BTreeMap<String, SchemaUrl>,
    requirement_level: Option<SignalRequirementLevel>,
    common: CommonFields,
    excluded: bool,
}

/// Iteratively resolves span refinements against local bases, prior refinements, and dependencies.
fn resolve_span_refinements<'a, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<Vec<(SignalId, ResolvedSpan)>, Error> {
    let mut resolved_spans: Vec<(SignalId, ResolvedSpan)> = Vec::new();
    let mut unresolved: Vec<(&SignalId, &IndexedSpanRefinement)> =
        index.span_refinements().iter().collect();
    let mut fatal_errors = Vec::new();

    while !unresolved.is_empty() {
        let mut progress = false;
        let mut still_unresolved: Vec<(&SignalId, &IndexedSpanRefinement)> = Vec::new();

        for (raw_id, indexed_s) in &unresolved {
            let s_ref = &indexed_s.refinement;
            let refinement_id = rules::normalize_refinement_id("span", raw_id);
            let provenance = SchemaProvenance {
                source: None,
                path: indexed_s.provenance.path.clone(),
            };
            let is_ref_excluded = is_excluded(&s_ref.annotations);
            let parent_ref = &s_ref.r#ref;
            let stripped_parent_ref = rules::normalize_refinement_id("span", parent_ref);

            let parent: SpanParent = if let Some(base) = base_signals
                .spans
                .iter()
                .find(|s| s.r#type == *parent_ref || s.r#type == stripped_parent_ref)
            {
                SpanParent {
                    r#type: base.r#type.clone(),
                    kind: base.kind,
                    name: base.name.clone(),
                    attributes: base.attributes.clone(),
                    entity_associations: base.entity_associations.clone(),
                    association_origins: base.association_origins.clone(),
                    requirement_level: base.requirement_level.clone(),
                    common: base.common.clone(),
                    excluded: is_excluded(&base.common.annotations),
                }
            } else if let Some((_, prev)) = resolved_spans
                .iter()
                .find(|(id, _)| id == parent_ref || *id == stripped_parent_ref)
            {
                SpanParent {
                    r#type: prev.r#type.clone(),
                    kind: prev.kind,
                    name: prev.name.clone(),
                    attributes: prev.attributes.clone(),
                    entity_associations: prev.entity_associations.clone(),
                    association_origins: prev.association_origins.clone(),
                    requirement_level: prev.requirement_level.clone(),
                    common: prev.common.clone(),
                    excluded: is_excluded(&prev.common.annotations),
                }
            } else {
                let is_parent_unresolved = unresolved.iter().any(|(other_id, _)| {
                    if *other_id == *raw_id {
                        return false;
                    }
                    let nid = rules::normalize_refinement_id("span", other_id);
                    nid == *parent_ref || nid == stripped_parent_ref || *other_id == parent_ref
                });
                if is_parent_unresolved {
                    still_unresolved.push((*raw_id, *indexed_s));
                    continue;
                }

                if let Some(err) = check_wrong_kind_in_base(
                    parent_ref,
                    &stripped_parent_ref,
                    raw_id,
                    "Span",
                    base_signals,
                ) {
                    fatal_errors.push(err);
                    continue;
                }

                match deps.lookup_signal(
                    "span",
                    parent_ref.as_ref(),
                    raw_id.as_ref(),
                    Some(&indexed_s.provenance),
                    cache_lookup,
                ) {
                    Ok(dep_signal) => {
                        if let DepSignalPayload::Span {
                            r#type,
                            kind,
                            name,
                            requirement_level,
                            attributes,
                        } = dep_signal.payload
                        {
                            let mut resolved_attrs = Vec::new();
                            for (attr, req, samp, origin) in attributes {
                                let mapped_attr =
                                    catalog.register_inherited_dep_attribute(attr, &origin)?;
                                resolved_attrs.push((mapped_attr, req, samp));
                            }
                            SpanParent {
                                r#type,
                                kind,
                                name,
                                attributes: resolved_attrs,
                                entity_associations: dep_signal.entity_associations,
                                association_origins: dep_signal.association_origins,
                                requirement_level,
                                common: dep_signal.common,
                                excluded: dep_signal.excluded,
                            }
                        } else {
                            fatal_errors.push(Error::UnresolvedRefinementParent {
                                refinement_id: raw_id.to_string(),
                                r#ref: parent_ref.to_string(),
                                provenance: Some(Box::new(indexed_s.provenance.clone())),
                            });
                            continue;
                        }
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
                    r#type: "span".to_owned(),
                    used_in: raw_id.to_string(),
                    provenance: Some(Box::new(indexed_s.provenance.clone())),
                });
                continue;
            }

            let (unrefs, ref_groups, inline_refs) = partition_span_attributes(&s_ref.attributes);
            let used_in = format!("span.{refinement_id}");

            let (merged_items, included_group_attrs) = match prepare_span_refinement_attributes(
                &ref_groups,
                &inline_refs,
                expanded_groups,
                &used_in,
                is_ref_excluded,
                Some(&indexed_s.provenance),
                warnings,
            ) {
                Ok(res) => res,
                Err(e) => {
                    fatal_errors.push(e);
                    continue;
                }
            };

            let parent_attr_keys: HashSet<String> = parent
                .attributes
                .iter()
                .map(|(a, _, _)| a.key.clone())
                .collect();
            let parent_required_keys: HashSet<String> = parent
                .attributes
                .iter()
                .filter(|(_, req, _)| is_required(req))
                .map(|(a, _, _)| a.key.clone())
                .collect();
            let parent_identifying_keys = HashSet::new();
            let local_attr_keys: HashSet<String> =
                inline_refs.iter().map(|a| a.base.r#ref.clone()).collect();

            let unref_errors = rules::validate_attribute_unrefs(
                refinement_id.as_ref(),
                parent_ref.as_ref(),
                &unrefs.iter().map(|u| (*u).clone()).collect::<Vec<_>>(),
                &parent_attr_keys,
                &parent_required_keys,
                &parent_identifying_keys,
                &local_attr_keys,
                &included_group_attrs,
                Some(&indexed_s.provenance),
            );
            if !unref_errors.is_empty() {
                fatal_errors.extend(unref_errors);
                continue;
            }

            let unref_set: HashSet<&str> = unrefs.iter().map(|u| u.unref.as_str()).collect();
            let mut current_attrs: Vec<(Attribute, RequirementLevel, Option<bool>)> = parent
                .attributes
                .into_iter()
                .filter(|(a, _, _)| !unref_set.contains(a.key.as_str()))
                .collect();

            if let Err(e) = apply_span_attribute_overrides(
                &mut current_attrs,
                merged_items,
                &used_in,
                is_ref_excluded,
                catalog,
                Some(&indexed_s.provenance),
            ) {
                fatal_errors.push(e);
                continue;
            }

            current_attrs.sort_by(|(a, _, _), (b, _, _)| a.key.cmp(&b.key));

            let name = rules::inherit_span_name(&s_ref.name, &parent.name);

            let span_attr_keys: HashSet<String> = current_attrs
                .iter()
                .map(|(a, _, _)| a.key.clone())
                .collect();
            let mut span_name_err = false;
            for template in &name.templates {
                for attr_key in &template.attributes {
                    if !span_attr_keys.contains(attr_key) {
                        fatal_errors.push(Error::SpanNameAttributeNotOnSpan {
                            span_id: raw_id.to_string(),
                            attribute_key: attr_key.clone(),
                            provenance: Some(Box::new(indexed_s.provenance.clone())),
                        });
                        span_name_err = true;
                        break;
                    }
                }
                if span_name_err {
                    break;
                }
            }
            if span_name_err {
                continue;
            }

            let brief = rules::inherit_brief(s_ref.brief.as_deref(), &parent.common.brief);
            let note = rules::inherit_note(&s_ref.note, &parent.common.note);
            let stability = rules::inherit_stability(&s_ref.stability, &parent.common.stability);
            let deprecated =
                rules::inherit_deprecated(&s_ref.deprecated, &parent.common.deprecated);
            let annotations =
                rules::inherit_annotations(&parent.common.annotations, &s_ref.annotations);
            let (entity_associations, association_origins) = if s_ref.entity_associations.is_empty()
            {
                (parent.entity_associations, parent.association_origins)
            } else {
                (
                    EntityAssociations::Raw(s_ref.entity_associations.clone()),
                    BTreeMap::new(),
                )
            };

            let resolved = ResolvedSpan {
                r#type: parent.r#type,
                kind: parent.kind,
                name,
                attributes: current_attrs,
                entity_associations,
                association_origins,
                requirement_level: parent.requirement_level,
                common: CommonFields {
                    brief,
                    note,
                    stability,
                    deprecated,
                    annotations,
                },
                provenance,
            };

            resolved_spans.push((refinement_id, resolved));
            progress = true;
        }

        if !progress && !still_unresolved.is_empty() {
            for (raw_id, indexed_s) in still_unresolved {
                fatal_errors.push(Error::UnresolvedRefinementParent {
                    refinement_id: raw_id.to_string(),
                    r#ref: indexed_s.refinement.r#ref.to_string(),
                    provenance: Some(Box::new(indexed_s.provenance.clone())),
                });
            }
            break;
        }

        unresolved = still_unresolved;
    }

    handle_errors(fatal_errors)?;

    resolved_spans.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(resolved_spans)
}

/// Snapshot of a parent event resolved for refinement inheritance.
struct EventParent {
    name: SignalId,
    attributes: Vec<(Attribute, RequirementLevel)>,
    entity_associations: EntityAssociations,
    association_origins: BTreeMap<String, SchemaUrl>,
    requirement_level: Option<SignalRequirementLevel>,
    common: CommonFields,
    excluded: bool,
}

/// Iteratively resolves event refinements against local bases, prior refinements, and dependencies.
fn resolve_event_refinements<'a, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<Vec<(SignalId, ResolvedEvent)>, Error> {
    let mut resolved_events: Vec<(SignalId, ResolvedEvent)> = Vec::new();
    let mut unresolved: Vec<(&SignalId, &IndexedEventRefinement)> =
        index.event_refinements().iter().collect();
    let mut fatal_errors = Vec::new();

    while !unresolved.is_empty() {
        let mut progress = false;
        let mut still_unresolved: Vec<(&SignalId, &IndexedEventRefinement)> = Vec::new();

        for (raw_id, indexed_e) in &unresolved {
            let e_ref = &indexed_e.refinement;
            let refinement_id = rules::normalize_refinement_id("event", raw_id);
            let provenance = SchemaProvenance {
                source: None,
                path: indexed_e.provenance.path.clone(),
            };
            let is_ref_excluded = is_excluded(&e_ref.annotations);
            let parent_ref = &e_ref.r#ref;
            let stripped_parent_ref = rules::normalize_refinement_id("event", parent_ref);

            let parent: EventParent = if let Some(base) = base_signals
                .events
                .iter()
                .find(|e| e.name == *parent_ref || e.name == stripped_parent_ref)
            {
                EventParent {
                    name: base.name.clone(),
                    attributes: base.attributes.clone(),
                    entity_associations: base.entity_associations.clone(),
                    association_origins: base.association_origins.clone(),
                    requirement_level: base.requirement_level.clone(),
                    common: base.common.clone(),
                    excluded: is_excluded(&base.common.annotations),
                }
            } else if let Some((_, prev)) = resolved_events
                .iter()
                .find(|(id, _)| id == parent_ref || *id == stripped_parent_ref)
            {
                EventParent {
                    name: prev.name.clone(),
                    attributes: prev.attributes.clone(),
                    entity_associations: prev.entity_associations.clone(),
                    association_origins: prev.association_origins.clone(),
                    requirement_level: prev.requirement_level.clone(),
                    common: prev.common.clone(),
                    excluded: is_excluded(&prev.common.annotations),
                }
            } else {
                let is_parent_unresolved = unresolved.iter().any(|(other_id, _)| {
                    if *other_id == *raw_id {
                        return false;
                    }
                    let nid = rules::normalize_refinement_id("event", other_id);
                    nid == *parent_ref || nid == stripped_parent_ref || *other_id == parent_ref
                });
                if is_parent_unresolved {
                    still_unresolved.push((*raw_id, *indexed_e));
                    continue;
                }

                if let Some(err) = check_wrong_kind_in_base(
                    parent_ref,
                    &stripped_parent_ref,
                    raw_id,
                    "Event",
                    base_signals,
                ) {
                    fatal_errors.push(err);
                    continue;
                }

                match deps.lookup_signal(
                    "event",
                    parent_ref.as_ref(),
                    raw_id.as_ref(),
                    Some(&indexed_e.provenance),
                    cache_lookup,
                ) {
                    Ok(dep_signal) => {
                        if let DepSignalPayload::Event {
                            name,
                            requirement_level,
                            attributes,
                        } = dep_signal.payload
                        {
                            let mut resolved_attrs = Vec::new();
                            for (attr, req, origin) in attributes {
                                let mapped_attr =
                                    catalog.register_inherited_dep_attribute(attr, &origin)?;
                                resolved_attrs.push((mapped_attr, req));
                            }
                            EventParent {
                                name,
                                attributes: resolved_attrs,
                                entity_associations: dep_signal.entity_associations,
                                association_origins: dep_signal.association_origins,
                                requirement_level,
                                common: dep_signal.common,
                                excluded: dep_signal.excluded,
                            }
                        } else {
                            fatal_errors.push(Error::UnresolvedRefinementParent {
                                refinement_id: raw_id.to_string(),
                                r#ref: parent_ref.to_string(),
                                provenance: Some(Box::new(indexed_e.provenance.clone())),
                            });
                            continue;
                        }
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
                    r#type: "event".to_owned(),
                    used_in: raw_id.to_string(),
                    provenance: Some(Box::new(indexed_e.provenance.clone())),
                });
                continue;
            }

            let (unrefs, ref_groups, inline_refs) = partition_metric_attributes(&e_ref.attributes);
            let used_in = format!("event.{refinement_id}");

            let (merged_items, included_group_attrs) = match prepare_refinement_attributes(
                &ref_groups,
                &inline_refs,
                expanded_groups,
                &used_in,
                is_ref_excluded,
                Some(&indexed_e.provenance),
                warnings,
            ) {
                Ok(res) => res,
                Err(e) => {
                    fatal_errors.push(e);
                    continue;
                }
            };

            let parent_attr_keys: HashSet<String> = parent
                .attributes
                .iter()
                .map(|(a, _)| a.key.clone())
                .collect();
            let parent_required_keys: HashSet<String> = parent
                .attributes
                .iter()
                .filter(|(_, req)| is_required(req))
                .map(|(a, _)| a.key.clone())
                .collect();
            let parent_identifying_keys = HashSet::new();
            let local_attr_keys: HashSet<String> =
                inline_refs.iter().map(|a| a.r#ref.clone()).collect();

            let unref_errors = rules::validate_attribute_unrefs(
                refinement_id.as_ref(),
                parent_ref.as_ref(),
                &unrefs.iter().map(|u| (*u).clone()).collect::<Vec<_>>(),
                &parent_attr_keys,
                &parent_required_keys,
                &parent_identifying_keys,
                &local_attr_keys,
                &included_group_attrs,
                Some(&indexed_e.provenance),
            );
            if !unref_errors.is_empty() {
                fatal_errors.extend(unref_errors);
                continue;
            }

            let unref_set: HashSet<&str> = unrefs.iter().map(|u| u.unref.as_str()).collect();
            let mut current_attrs: Vec<(Attribute, RequirementLevel)> = parent
                .attributes
                .into_iter()
                .filter(|(a, _)| !unref_set.contains(a.key.as_str()))
                .collect();

            if let Err(e) = apply_attribute_overrides(
                &mut current_attrs,
                merged_items,
                &used_in,
                is_ref_excluded,
                catalog,
                Some(&indexed_e.provenance),
            ) {
                fatal_errors.push(e);
                continue;
            }

            current_attrs.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

            let brief = rules::inherit_brief(e_ref.brief.as_deref(), &parent.common.brief);
            let note = rules::inherit_note(&e_ref.note, &parent.common.note);
            let stability = rules::inherit_stability(&e_ref.stability, &parent.common.stability);
            let deprecated =
                rules::inherit_deprecated(&e_ref.deprecated, &parent.common.deprecated);
            let annotations =
                rules::inherit_annotations(&parent.common.annotations, &e_ref.annotations);
            let (entity_associations, association_origins) = if e_ref.entity_associations.is_empty()
            {
                (parent.entity_associations, parent.association_origins)
            } else {
                (
                    EntityAssociations::Raw(e_ref.entity_associations.clone()),
                    BTreeMap::new(),
                )
            };

            let resolved = ResolvedEvent {
                name: parent.name,
                attributes: current_attrs,
                entity_associations,
                association_origins,
                requirement_level: parent.requirement_level,
                common: CommonFields {
                    brief,
                    note,
                    stability,
                    deprecated,
                    annotations,
                },
                provenance,
            };

            resolved_events.push((refinement_id, resolved));
            progress = true;
        }

        if !progress && !still_unresolved.is_empty() {
            for (raw_id, indexed_e) in still_unresolved {
                fatal_errors.push(Error::UnresolvedRefinementParent {
                    refinement_id: raw_id.to_string(),
                    r#ref: indexed_e.refinement.r#ref.to_string(),
                    provenance: Some(Box::new(indexed_e.provenance.clone())),
                });
            }
            break;
        }

        unresolved = still_unresolved;
    }

    handle_errors(fatal_errors)?;

    resolved_events.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(resolved_events)
}

/// Snapshot of a parent entity resolved for refinement inheritance.
struct EntityParent {
    r#type: SignalId,
    identity: Vec<Attribute>,
    description: Vec<(Attribute, RequirementLevel)>,
    requirement_level: Option<SignalRequirementLevel>,
    common: CommonFields,
    excluded: bool,
}

/// Iteratively resolves entity refinements while enforcing identity preservation rules.
fn resolve_entity_refinements<'a, C: SchemaCacheLookup>(
    index: &Index,
    deps: &V2Dependencies,
    base_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
    warnings: &mut Vec<Error>,
) -> Result<Vec<(SignalId, ResolvedEntity)>, Error> {
    let mut resolved_entities: Vec<(SignalId, ResolvedEntity)> = Vec::new();
    let mut unresolved: Vec<(&SignalId, &IndexedEntityRefinement)> =
        index.entity_refinements().iter().collect();
    let mut fatal_errors = Vec::new();

    while !unresolved.is_empty() {
        let mut progress = false;
        let mut still_unresolved: Vec<(&SignalId, &IndexedEntityRefinement)> = Vec::new();

        for (raw_id, indexed_en) in &unresolved {
            let en_ref = &indexed_en.refinement;
            let refinement_id = rules::normalize_refinement_id("entity", raw_id);
            let provenance = SchemaProvenance {
                source: None,
                path: indexed_en.provenance.path.clone(),
            };
            let is_ref_excluded = is_excluded(&en_ref.annotations);
            let parent_ref = &en_ref.r#ref;
            let stripped_parent_ref = rules::normalize_refinement_id("entity", parent_ref);

            let parent: EntityParent = if let Some(base) = base_signals
                .entities
                .iter()
                .find(|en| en.r#type == *parent_ref || en.r#type == stripped_parent_ref)
            {
                EntityParent {
                    r#type: base.r#type.clone(),
                    identity: base.identity.clone(),
                    description: base.description.clone(),
                    requirement_level: base.requirement_level.clone(),
                    common: base.common.clone(),
                    excluded: is_excluded(&base.common.annotations),
                }
            } else if let Some((_, prev)) = resolved_entities
                .iter()
                .find(|(id, _)| id == parent_ref || *id == stripped_parent_ref)
            {
                EntityParent {
                    r#type: prev.r#type.clone(),
                    identity: prev.identity.clone(),
                    description: prev.description.clone(),
                    requirement_level: prev.requirement_level.clone(),
                    common: prev.common.clone(),
                    excluded: is_excluded(&prev.common.annotations),
                }
            } else {
                let is_parent_unresolved = unresolved.iter().any(|(other_id, _)| {
                    if *other_id == *raw_id {
                        return false;
                    }
                    let nid = rules::normalize_refinement_id("entity", other_id);
                    nid == *parent_ref || nid == stripped_parent_ref || *other_id == parent_ref
                });
                if is_parent_unresolved {
                    still_unresolved.push((*raw_id, *indexed_en));
                    continue;
                }

                if let Some(err) = check_wrong_kind_in_base(
                    parent_ref,
                    &stripped_parent_ref,
                    raw_id,
                    "Entity",
                    base_signals,
                ) {
                    fatal_errors.push(err);
                    continue;
                }

                match deps.lookup_signal(
                    "entity",
                    parent_ref.as_ref(),
                    raw_id.as_ref(),
                    Some(&indexed_en.provenance),
                    cache_lookup,
                ) {
                    Ok(dep_signal) => {
                        if let DepSignalPayload::Entity {
                            r#type,
                            identity,
                            description,
                            requirement_level,
                        } = dep_signal.payload
                        {
                            let mut resolved_ident = Vec::new();
                            for (attr, origin) in identity {
                                let mapped_attr =
                                    catalog.register_inherited_dep_attribute(attr, &origin)?;
                                resolved_ident.push(mapped_attr);
                            }
                            let mut resolved_desc = Vec::new();
                            for (attr, req, origin) in description {
                                let mapped_attr =
                                    catalog.register_inherited_dep_attribute(attr, &origin)?;
                                resolved_desc.push((mapped_attr, req));
                            }
                            EntityParent {
                                r#type,
                                identity: resolved_ident,
                                description: resolved_desc,
                                requirement_level,
                                common: dep_signal.common,
                                excluded: dep_signal.excluded,
                            }
                        } else {
                            fatal_errors.push(Error::UnresolvedRefinementParent {
                                refinement_id: raw_id.to_string(),
                                r#ref: parent_ref.to_string(),
                                provenance: Some(Box::new(indexed_en.provenance.clone())),
                            });
                            continue;
                        }
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
                    r#type: "entity".to_owned(),
                    used_in: raw_id.to_string(),
                    provenance: Some(Box::new(indexed_en.provenance.clone())),
                });
                continue;
            }

            let mut identity_ref_keys = Vec::new();
            let mut identity_arefs = Vec::new();
            for item in &en_ref.identity {
                identity_ref_keys.push(item.r#ref.clone());
                identity_arefs.push(item.clone());
            }

            let mut desc_unrefs: Vec<&AttributeUnref> = Vec::new();
            let desc_ref_groups: Vec<SignalId> = Vec::new();
            let mut desc_inline_refs: Vec<AttributeRef> = Vec::new();
            for item in &en_ref.description {
                match item {
                    EntityAttributeRefinement::Ref(a) => desc_inline_refs.push(a.clone()),
                    EntityAttributeRefinement::Unref(u) => desc_unrefs.push(u),
                }
            }

            let parent_ident_keys: HashSet<String> =
                parent.identity.iter().map(|a| a.key.clone()).collect();
            let desc_keys: Vec<String> = desc_inline_refs.iter().map(|a| a.r#ref.clone()).collect();
            let identity_validation_errors = rules::validate_entity_identity_refinements(
                refinement_id.as_ref(),
                parent_ref.as_ref(),
                &parent_ident_keys,
                &identity_ref_keys,
                &desc_keys,
                Some(&indexed_en.provenance),
            );
            if !identity_validation_errors.is_empty() {
                fatal_errors.extend(identity_validation_errors);
                continue;
            }

            let mut identity_attrs = parent.identity.clone();
            for id_ref in &identity_arefs {
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
            let (merged_desc_items, included_group_attrs) = match prepare_refinement_attributes(
                &desc_ref_groups,
                &desc_inline_refs,
                expanded_groups,
                &used_in,
                is_ref_excluded,
                Some(&indexed_en.provenance),
                warnings,
            ) {
                Ok(res) => res,
                Err(e) => {
                    fatal_errors.push(e);
                    continue;
                }
            };

            let parent_desc_keys: HashSet<String> = parent
                .description
                .iter()
                .map(|(a, _)| a.key.clone())
                .collect();
            let parent_required_keys: HashSet<String> = parent
                .description
                .iter()
                .filter(|(_, req)| is_required(req))
                .map(|(a, _)| a.key.clone())
                .collect();
            let local_attr_keys: HashSet<String> =
                desc_inline_refs.iter().map(|a| a.r#ref.clone()).collect();

            let unref_errors = rules::validate_attribute_unrefs(
                refinement_id.as_ref(),
                parent_ref.as_ref(),
                &desc_unrefs.iter().map(|u| (*u).clone()).collect::<Vec<_>>(),
                &parent_desc_keys,
                &parent_required_keys,
                &parent_ident_keys,
                &local_attr_keys,
                &included_group_attrs,
                Some(&indexed_en.provenance),
            );
            if !unref_errors.is_empty() {
                fatal_errors.extend(unref_errors);
                continue;
            }

            let unref_set: HashSet<&str> = desc_unrefs.iter().map(|u| u.unref.as_str()).collect();
            let mut current_desc: Vec<(Attribute, RequirementLevel)> = parent
                .description
                .into_iter()
                .filter(|(a, _)| !unref_set.contains(a.key.as_str()))
                .collect();

            if let Err(e) = apply_attribute_overrides(
                &mut current_desc,
                merged_desc_items,
                &used_in,
                is_ref_excluded,
                catalog,
                Some(&indexed_en.provenance),
            ) {
                fatal_errors.push(e);
                continue;
            }

            identity_attrs.sort_by(|a, b| a.key.cmp(&b.key));
            current_desc.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

            let brief = rules::inherit_brief(en_ref.brief.as_deref(), &parent.common.brief);
            let note = rules::inherit_note(&en_ref.note, &parent.common.note);
            let stability = rules::inherit_stability(&en_ref.stability, &parent.common.stability);
            let deprecated =
                rules::inherit_deprecated(&en_ref.deprecated, &parent.common.deprecated);
            let annotations =
                rules::inherit_annotations(&parent.common.annotations, &en_ref.annotations);

            let resolved = ResolvedEntity {
                r#type: parent.r#type,
                identity: identity_attrs,
                description: current_desc,
                requirement_level: parent.requirement_level,
                common: CommonFields {
                    brief,
                    note,
                    stability,
                    deprecated,
                    annotations,
                },
                provenance,
            };

            resolved_entities.push((refinement_id, resolved));
            progress = true;
        }

        if !progress && !still_unresolved.is_empty() {
            for (raw_id, indexed_en) in still_unresolved {
                fatal_errors.push(Error::UnresolvedRefinementParent {
                    refinement_id: raw_id.to_string(),
                    r#ref: indexed_en.refinement.r#ref.to_string(),
                    provenance: Some(Box::new(indexed_en.provenance.clone())),
                });
            }
            break;
        }

        unresolved = still_unresolved;
    }

    handle_errors(fatal_errors)?;

    resolved_entities.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(resolved_entities)
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
    let metrics = resolve_metric_refinements(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let spans = resolve_span_refinements(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let events = resolve_event_refinements(
        index,
        deps,
        base_signals,
        expanded_groups,
        catalog,
        cache_lookup,
        warnings,
    )?;
    let entities = resolve_entity_refinements(
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
