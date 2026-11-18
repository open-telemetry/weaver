// SPDX-License-Identifier: Apache-2.0

//! Resolution of `imports:` blocks against dependencies in native V2.

use globset::{GlobSet, GlobSetBuilder};
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap};
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::GroupWildcard;
use weaver_semconv::v2::Imports;

use super::attributes::{upgrade_attribute_content, Catalog};
use super::dependency::{
    DepImportedAttributeGroup, DepImportedEntity, DepImportedEvent, DepImportedMetric,
    DepImportedSpan, DependencyAdapter, V2Dependencies,
};
use super::groups::ExpandedGroups;
use super::index::IndexedImports;
use super::signals::ResolvedSignals;
use crate::error::Error;
use crate::graph::chosen_versions::{
    chosen_version_if_superseded, lookup_chosen_schema, SchemaCacheLookup,
};
use crate::graph::entity_lookup::upgrade_entity_origin;

/// The field of an `imports:` block that lists a signal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImportField {
    Metrics,
    Events,
    Spans,
    Entities,
    AttributeGroups,
}

impl ImportField {
    /// Returns all five importable definition categories in deterministic processing order.
    pub fn all() -> [Self; 5] {
        [
            Self::Metrics,
            Self::Events,
            Self::Spans,
            Self::Entities,
            Self::AttributeGroups,
        ]
    }

    /// Returns the YAML field name for this import category.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Metrics => "metrics",
            Self::Events => "events",
            Self::Spans => "spans",
            Self::Entities => "entities",
            Self::AttributeGroups => "attribute_groups",
        }
    }

    /// Returns the wildcard patterns configured for this category in `imports`.
    pub fn patterns<'a>(&self, imports: &'a Imports) -> &'a [GroupWildcard] {
        let field = match self {
            Self::Metrics => &imports.metrics,
            Self::Events => &imports.events,
            Self::Spans => &imports.spans,
            Self::Entities => &imports.entities,
            Self::AttributeGroups => &imports.attribute_groups,
        };
        field.as_deref().unwrap_or_default()
    }
}

/// All items imported from dependencies.
#[derive(Debug, Default)]
pub(crate) struct ImportedItems {
    pub metrics: Vec<DepImportedMetric>,
    pub spans: Vec<DepImportedSpan>,
    pub events: Vec<DepImportedEvent>,
    pub entities: Vec<DepImportedEntity>,
    pub attribute_groups: Vec<DepImportedAttributeGroup>,
}

/// Compiles an iterator of `GroupWildcard` patterns into a `GlobSet` matcher.
fn build_globset<'a>(wildcards: impl Iterator<Item = &'a GroupWildcard>) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for wildcard in wildcards {
        _ = builder.add(wildcard.0.clone());
    }
    builder.build().map_err(|e| Error::InvalidWildcard {
        error: e.to_string(),
    })
}

/// Upgrades `attr` and updates `attr_origin` in place if its defining schema was superseded.
fn resolve_dep_attr<C: SchemaCacheLookup>(
    deps: &V2Dependencies,
    cache_lookup: &C,
    attr: &mut Attribute,
    attr_origin: &mut SchemaUrl,
) -> Result<SchemaUrl, Error> {
    let effective_origin = upgrade_attribute_content(attr, attr_origin, deps, cache_lookup)?;
    *attr_origin = effective_origin.clone();
    Ok(effective_origin)
}

/// Looks up `key` in `chosen_url`, populating `cache` on first access per schema.
fn lookup_upgraded_item<T: Clone>(
    cache: &mut HashMap<SchemaUrl, HashMap<SignalId, T>>,
    chosen_url: &SchemaUrl,
    key: &SignalId,
    deps: &V2Dependencies,
    cache_lookup: &impl SchemaCacheLookup,
    build_items: impl FnOnce(&DependencyAdapter) -> Vec<T>,
    item_key: impl Fn(&T) -> &SignalId,
) -> Result<Option<T>, Error> {
    if !cache.contains_key(chosen_url) {
        let chosen_schema = lookup_chosen_schema(chosen_url, cache_lookup)?;
        let chosen_adapter = deps.adapter_for(chosen_url, &chosen_schema);
        let items = build_items(&chosen_adapter);
        let mut map = HashMap::with_capacity(items.len());
        for item in items {
            let _ = map.entry(item_key(&item).clone()).or_insert(item);
        }
        let _ = cache.insert(chosen_url.clone(), map);
    }
    Ok(cache.get(chosen_url).and_then(|map| map.get(key).cloned()))
}

/// Resolves `imports:` blocks against dependencies, applying version upgrades and deduplication.
pub(crate) fn resolve<'a, C: SchemaCacheLookup>(
    schema_url: &SchemaUrl,
    local_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    imports: &[IndexedImports],
    deps: &V2Dependencies,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
) -> Result<(ImportedItems, Vec<Error>), Error> {
    let mut warnings = Vec::new();

    let explicit_imports: Vec<&IndexedImports> =
        imports.iter().filter(|i| i.origin.is_explicit()).collect();

    let mut explicit_matchers = HashMap::new();
    let mut any_matchers = HashMap::new();
    let mut explicit_patterns_map = HashMap::new();

    for field in ImportField::all() {
        let exp_pats: Vec<&GroupWildcard> = explicit_imports
            .iter()
            .flat_map(|i| field.patterns(&i.imports))
            .collect();
        let exp_globset = build_globset(exp_pats.iter().copied())?;
        _ = explicit_patterns_map.insert(field, exp_pats);
        _ = explicit_matchers.insert(field, exp_globset);

        let any_pats: Vec<&GroupWildcard> = imports
            .iter()
            .flat_map(|i| field.patterns(&i.imports))
            .collect();
        let any_globset = build_globset(any_pats.iter().copied())?;
        _ = any_matchers.insert(field, any_globset);
    }

    let mut imported = ImportedItems::default();

    let mut matched_patterns: HashMap<ImportField, Vec<bool>> = HashMap::new();
    for field in ImportField::all() {
        let count = explicit_patterns_map[&field].len();
        _ = matched_patterns.insert(field, vec![false; count]);
    }

    // Seed rival maps with local definitions to detect local-vs-imported clashes.
    let mut metric_rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for m in &local_signals.metrics {
        metric_rivals
            .entry(m.name.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                schema_url.clone(),
                &m.provenance.path,
            ));
    }

    let mut span_rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for s in &local_signals.spans {
        span_rivals
            .entry(s.r#type.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                schema_url.clone(),
                &s.provenance.path,
            ));
    }

    let mut event_rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for e in &local_signals.events {
        event_rivals
            .entry(e.name.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                schema_url.clone(),
                &e.provenance.path,
            ));
    }

    let mut entity_rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for en in &local_signals.entities {
        entity_rivals
            .entry(en.r#type.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                schema_url.clone(),
                &en.provenance.path,
            ));
    }

    let mut ag_rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for ag in expanded_groups.public_groups() {
        ag_rivals
            .entry(ag.id.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                schema_url.clone(),
                &ag.provenance.path,
            ));
    }

    let mut raw_metrics: Vec<DepImportedMetric> = Vec::new();
    let mut upgraded_metrics_cache: HashMap<SchemaUrl, HashMap<SignalId, DepImportedMetric>> =
        HashMap::new();
    for adapter in deps.adapters() {
        for mut m in adapter.importable_metrics() {
            if m.excluded {
                continue;
            }

            let any_matcher = &any_matchers[&ImportField::Metrics];
            let explicit_matcher = &explicit_matchers[&ImportField::Metrics];

            let matches_any = m.match_keys.iter().any(|k| any_matcher.is_match(k));
            if !matches_any {
                continue;
            }

            for k in &m.match_keys {
                for idx in explicit_matcher.matches(k) {
                    matched_patterns
                        .get_mut(&ImportField::Metrics)
                        .expect("pattern flags initialized")[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(&m.origin, cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_metrics_cache,
                    &chosen_url,
                    &m.name,
                    deps,
                    cache_lookup,
                    DependencyAdapter::importable_metrics,
                    |cand| &cand.name,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: m.name.to_string(),
                        original_url: m.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: m.name.to_string(),
                        original_url: m.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                for (name, origin) in upgraded.association_origins.iter_mut() {
                    *origin = upgrade_entity_origin(name, origin, cache_lookup)?;
                }
                upgraded.origin = chosen_url;
                m = upgraded;
            }

            raw_metrics.push(m);
        }
    }

    let mut kept_metrics: Vec<DepImportedMetric> = Vec::new();
    let mut metric_buckets: HashMap<(String, String), usize> = HashMap::new();

    for m in raw_metrics {
        let key = (m.origin.to_string(), m.name.to_string());
        if metric_buckets.contains_key(&key) {
            continue;
        }
        _ = metric_buckets.insert(key, kept_metrics.len());
        metric_rivals
            .entry(m.name.to_string())
            .or_default()
            .push(SemconvProvenance::new(m.origin.clone(), &m.provenance.path));
        kept_metrics.push(m);
    }

    for m in &mut kept_metrics {
        for (attr, _, attr_origin, is_v1_def) in &mut m.attributes {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        m.provenance = SchemaProvenance {
            source: deps.dependency_ref(&m.origin),
            path: m.provenance.path.clone(),
        };
    }
    imported.metrics = kept_metrics;

    let mut raw_spans: Vec<DepImportedSpan> = Vec::new();
    let mut upgraded_spans_cache: HashMap<SchemaUrl, HashMap<SignalId, DepImportedSpan>> =
        HashMap::new();
    for adapter in deps.adapters() {
        for mut s in adapter.importable_spans() {
            if s.excluded {
                continue;
            }

            let any_matcher = &any_matchers[&ImportField::Spans];
            let explicit_matcher = &explicit_matchers[&ImportField::Spans];

            let matches_any = s.match_keys.iter().any(|k| any_matcher.is_match(k));
            if !matches_any {
                continue;
            }

            for k in &s.match_keys {
                for idx in explicit_matcher.matches(k) {
                    matched_patterns
                        .get_mut(&ImportField::Spans)
                        .expect("pattern flags initialized")[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(&s.origin, cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_spans_cache,
                    &chosen_url,
                    &s.r#type,
                    deps,
                    cache_lookup,
                    DependencyAdapter::importable_spans,
                    |cand| &cand.r#type,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: s.r#type.to_string(),
                        original_url: s.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: s.r#type.to_string(),
                        original_url: s.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                for (name, origin) in upgraded.association_origins.iter_mut() {
                    *origin = upgrade_entity_origin(name, origin, cache_lookup)?;
                }
                upgraded.origin = chosen_url;
                s = upgraded;
            }

            raw_spans.push(s);
        }
    }

    let mut kept_spans: Vec<DepImportedSpan> = Vec::new();
    let mut span_buckets: HashMap<(String, String), usize> = HashMap::new();

    for s in raw_spans {
        let key = (s.origin.to_string(), s.r#type.to_string());
        if span_buckets.contains_key(&key) {
            continue;
        }
        _ = span_buckets.insert(key, kept_spans.len());
        span_rivals
            .entry(s.r#type.to_string())
            .or_default()
            .push(SemconvProvenance::new(s.origin.clone(), &s.provenance.path));
        kept_spans.push(s);
    }

    for s in &mut kept_spans {
        for (attr, _, _, attr_origin, is_v1_def) in &mut s.attributes {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        s.provenance = SchemaProvenance {
            source: deps.dependency_ref(&s.origin),
            path: s.provenance.path.clone(),
        };
    }
    imported.spans = kept_spans;

    let mut raw_events: Vec<DepImportedEvent> = Vec::new();
    let mut upgraded_events_cache: HashMap<SchemaUrl, HashMap<SignalId, DepImportedEvent>> =
        HashMap::new();
    for adapter in deps.adapters() {
        for mut e in adapter.importable_events() {
            if e.excluded {
                continue;
            }

            let any_matcher = &any_matchers[&ImportField::Events];
            let explicit_matcher = &explicit_matchers[&ImportField::Events];

            let matches_any = e.match_keys.iter().any(|k| any_matcher.is_match(k));
            if !matches_any {
                continue;
            }

            for k in &e.match_keys {
                for idx in explicit_matcher.matches(k) {
                    matched_patterns
                        .get_mut(&ImportField::Events)
                        .expect("pattern flags initialized")[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(&e.origin, cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_events_cache,
                    &chosen_url,
                    &e.name,
                    deps,
                    cache_lookup,
                    DependencyAdapter::importable_events,
                    |cand| &cand.name,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: e.name.to_string(),
                        original_url: e.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: e.name.to_string(),
                        original_url: e.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                for (name, origin) in upgraded.association_origins.iter_mut() {
                    *origin = upgrade_entity_origin(name, origin, cache_lookup)?;
                }
                upgraded.origin = chosen_url;
                e = upgraded;
            }

            raw_events.push(e);
        }
    }

    let mut kept_events: Vec<DepImportedEvent> = Vec::new();
    let mut event_buckets: HashMap<(String, String), usize> = HashMap::new();

    for e in raw_events {
        let key = (e.origin.to_string(), e.name.to_string());
        if event_buckets.contains_key(&key) {
            continue;
        }
        _ = event_buckets.insert(key, kept_events.len());
        event_rivals
            .entry(e.name.to_string())
            .or_default()
            .push(SemconvProvenance::new(e.origin.clone(), &e.provenance.path));
        kept_events.push(e);
    }

    for e in &mut kept_events {
        for (attr, _, attr_origin, is_v1_def) in &mut e.attributes {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        e.provenance = SchemaProvenance {
            source: deps.dependency_ref(&e.origin),
            path: e.provenance.path.clone(),
        };
    }
    imported.events = kept_events;

    let mut raw_entities: Vec<DepImportedEntity> = Vec::new();
    let mut upgraded_entities_cache: HashMap<SchemaUrl, HashMap<SignalId, DepImportedEntity>> =
        HashMap::new();
    for adapter in deps.adapters() {
        for mut en in adapter.importable_entities() {
            if en.excluded {
                continue;
            }

            let any_matcher = &any_matchers[&ImportField::Entities];
            let explicit_matcher = &explicit_matchers[&ImportField::Entities];

            let matches_any = en.match_keys.iter().any(|k| any_matcher.is_match(k));
            if !matches_any {
                continue;
            }

            for k in &en.match_keys {
                for idx in explicit_matcher.matches(k) {
                    matched_patterns
                        .get_mut(&ImportField::Entities)
                        .expect("pattern flags initialized")[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(&en.origin, cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_entities_cache,
                    &chosen_url,
                    &en.r#type,
                    deps,
                    cache_lookup,
                    DependencyAdapter::importable_entities,
                    |cand| &cand.r#type,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: en.r#type.to_string(),
                        original_url: en.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: en.r#type.to_string(),
                        original_url: en.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                upgraded.origin = chosen_url;
                en = upgraded;
            }

            raw_entities.push(en);
        }
    }

    let mut kept_entities: Vec<DepImportedEntity> = Vec::new();
    let mut entity_buckets: HashMap<(String, String), usize> = HashMap::new();

    for en in raw_entities {
        let key = (en.origin.to_string(), en.r#type.to_string());
        if entity_buckets.contains_key(&key) {
            continue;
        }
        _ = entity_buckets.insert(key, kept_entities.len());
        entity_rivals
            .entry(en.r#type.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                en.origin.clone(),
                &en.provenance.path,
            ));
        kept_entities.push(en);
    }

    for en in &mut kept_entities {
        for (attr, attr_origin, is_v1_def) in &mut en.identity {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        for (attr, _, attr_origin, is_v1_def) in &mut en.description {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        en.provenance = SchemaProvenance {
            source: deps.dependency_ref(&en.origin),
            path: en.provenance.path.clone(),
        };
    }
    imported.entities = kept_entities;

    let mut raw_attribute_groups: Vec<DepImportedAttributeGroup> = Vec::new();
    let mut upgraded_ag_cache: HashMap<SchemaUrl, HashMap<SignalId, DepImportedAttributeGroup>> =
        HashMap::new();
    for adapter in deps.adapters() {
        for mut ag in adapter.importable_attribute_groups() {
            if ag.excluded {
                continue;
            }

            let any_matcher = &any_matchers[&ImportField::AttributeGroups];
            let explicit_matcher = &explicit_matchers[&ImportField::AttributeGroups];

            let matches_any = ag.match_keys.iter().any(|k| any_matcher.is_match(k));
            if !matches_any {
                continue;
            }

            for k in &ag.match_keys {
                for idx in explicit_matcher.matches(k) {
                    matched_patterns
                        .get_mut(&ImportField::AttributeGroups)
                        .expect("pattern flags initialized")[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(&ag.origin, cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_ag_cache,
                    &chosen_url,
                    &ag.id,
                    deps,
                    cache_lookup,
                    DependencyAdapter::importable_attribute_groups,
                    |cand| &cand.id,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: ag.id.to_string(),
                        original_url: ag.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: ag.id.to_string(),
                        original_url: ag.origin.to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                upgraded.origin = chosen_url;
                ag = upgraded;
            }

            raw_attribute_groups.push(ag);
        }
    }

    let mut kept_attribute_groups: Vec<DepImportedAttributeGroup> = Vec::new();
    let mut ag_buckets: HashMap<(String, String), usize> = HashMap::new();

    for ag in raw_attribute_groups {
        let key = (ag.origin.to_string(), ag.id.to_string());
        if ag_buckets.contains_key(&key) {
            continue;
        }
        _ = ag_buckets.insert(key, kept_attribute_groups.len());
        ag_rivals
            .entry(ag.id.to_string())
            .or_default()
            .push(SemconvProvenance::new(
                ag.origin.clone(),
                &ag.provenance.path,
            ));
        kept_attribute_groups.push(ag);
    }

    for ag in &mut kept_attribute_groups {
        for (attr, _, attr_origin, is_v1_def) in &mut ag.attributes {
            let effective_origin = resolve_dep_attr(deps, cache_lookup, attr, attr_origin)?;
            attr.provenance = SchemaProvenance {
                source: deps.dependency_ref(&effective_origin),
                path: String::new(),
            };
            catalog.register_attribute(attr.clone());
            if *is_v1_def {
                catalog.register_v1_imported_def_attribute(attr.clone());
            }
        }
        ag.provenance = SchemaProvenance {
            source: deps.dependency_ref(&ag.origin),
            path: ag.provenance.path.clone(),
        };
    }
    imported.attribute_groups = kept_attribute_groups;

    let mut duplicate_group_ids = Vec::new();
    let mut duplicate_metric_names = Vec::new();
    let mut duplicate_group_names = Vec::new();

    for (name, provs) in metric_rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: format!("metric.{name}"),
                provenances: unique_provs.clone(),
            });
            duplicate_metric_names.push(Error::DuplicateMetricName {
                metric_name: name,
                provenances: unique_provs,
            });
        }
    }

    for (type_id, provs) in span_rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: format!("span.{type_id}"),
                provenances: unique_provs.clone(),
            });
            duplicate_group_names.push(Error::DuplicateGroupName {
                group_name: type_id,
                provenances: unique_provs,
            });
        }
    }

    for (name, provs) in event_rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: format!("event.{name}"),
                provenances: unique_provs.clone(),
            });
            duplicate_group_names.push(Error::DuplicateGroupName {
                group_name: name,
                provenances: unique_provs,
            });
        }
    }

    for (type_id, provs) in entity_rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: format!("entity.{type_id}"),
                provenances: unique_provs.clone(),
            });
            duplicate_group_names.push(Error::DuplicateGroupName {
                group_name: type_id,
                provenances: unique_provs,
            });
        }
    }

    for (id, provs) in ag_rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: id,
                provenances: unique_provs,
            });
        }
    }

    warnings.extend(duplicate_group_ids);
    warnings.extend(duplicate_metric_names);
    warnings.extend(duplicate_group_names);

    for field in ImportField::all() {
        let patterns = &explicit_patterns_map[&field];
        let matched = &matched_patterns[&field];
        for (pattern, is_matched) in patterns.iter().zip(matched) {
            if !is_matched {
                warnings.push(Error::UnmatchedImport {
                    pattern: pattern.0.glob().to_owned(),
                    signal: field.name().to_owned(),
                });
            }
        }
    }

    Ok((imported, warnings))
}
