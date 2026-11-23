// SPDX-License-Identifier: Apache-2.0

//! Resolution of `imports:` blocks against dependencies in native V2.

use globset::GlobSet;
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap, HashSet};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::GroupWildcard;

use super::attributes::{upgrade_attribute_content, Catalog};
use super::dependency::{
    DepImportedAttributeGroup, DepImportedEntity, DepImportedEvent, DepImportedMetric,
    DepImportedSpan, DependencyAdapter, V2Dependencies,
};
use super::groups::ExpandedGroups;
use super::index::{Index, Indexed, IndexedImports};
use super::signals::ResolvedSignals;
use crate::error::Error;
use crate::graph::chosen_versions::{
    chosen_version_if_superseded, lookup_chosen_schema, SchemaCacheLookup,
};
use crate::graph::entity_lookup::upgrade_entity_origin;
use crate::graph::imports::{build_globset, ImportField};

/// All items imported from dependencies.
#[derive(Debug, Default)]
pub(crate) struct ImportedItems {
    pub metrics: Vec<DepImportedMetric>,
    pub spans: Vec<DepImportedSpan>,
    pub events: Vec<DepImportedEvent>,
    pub entities: Vec<DepImportedEntity>,
    pub attribute_groups: Vec<DepImportedAttributeGroup>,
}

/// Upgrades `attr` and registers it (and any V1 definition marker) in `catalog`.
fn register_imported_attr<C: SchemaCacheLookup>(
    deps: &V2Dependencies,
    cache_lookup: &C,
    catalog: &mut Catalog<'_, C>,
    attr: &mut Attribute,
    attr_origin: &mut SchemaUrl,
    is_v1_def: bool,
) -> Result<(), Error> {
    let effective_origin = upgrade_attribute_content(attr, attr_origin, deps, cache_lookup)?;
    *attr_origin = effective_origin.clone();
    attr.provenance = SchemaProvenance {
        source: deps.dependency_ref(&effective_origin),
        path: String::new(),
    };
    catalog.register_attribute(attr.clone());
    if is_v1_def {
        catalog.register_v1_imported_def_attribute(attr.clone());
    }
    Ok(())
}

/// Upgrades entity association origin URLs in place when superseded.
fn upgrade_association_origins<C: SchemaCacheLookup>(
    association_origins: &mut BTreeMap<String, SchemaUrl>,
    cache_lookup: &C,
) -> Result<(), Error> {
    for (name, origin) in association_origins.iter_mut() {
        *origin = upgrade_entity_origin(name, origin, cache_lookup)?;
    }
    Ok(())
}

/// Common interface over dependency-imported signal and attribute group records.
trait ImportableDepItem: Sized + Clone {
    /// `imports:` block field corresponding to this item category.
    const FIELD: ImportField;
    /// Returns the canonical local key (`name`, `type`, or `id`) of this item.
    fn key(&self) -> &SignalId;
    /// Returns the schema URL from which this item originated.
    fn origin(&self) -> &SchemaUrl;
    /// Updates the origin schema URL after a chosen-version upgrade.
    fn set_origin(&mut self, origin: SchemaUrl);
    /// Returns the item's resolved schema provenance.
    fn provenance(&self) -> &SchemaProvenance;
    /// Returns a mutable reference to the item's resolved schema provenance.
    fn provenance_mut(&mut self) -> &mut SchemaProvenance;
    /// Returns `true` if this item is marked excluded from dependency resolution.
    fn excluded(&self) -> bool;
    /// Returns all strings matched against `imports:` glob patterns for this item.
    fn match_keys(&self) -> &[String];
    /// Extracts all importable items of this category from `adapter`.
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self>;

    /// Upgrades any inherited entity association origin URLs when the item is upgraded.
    fn upgrade_associations<C: SchemaCacheLookup>(
        &mut self,
        _cache_lookup: &C,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Upgrades and registers all attributes carried by this imported item in `catalog`.
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error>;
}

impl ImportableDepItem for DepImportedMetric {
    const FIELD: ImportField = ImportField::Metrics;
    fn key(&self) -> &SignalId {
        &self.name
    }
    fn origin(&self) -> &SchemaUrl {
        &self.origin
    }
    fn set_origin(&mut self, origin: SchemaUrl) {
        self.origin = origin;
    }
    fn provenance(&self) -> &SchemaProvenance {
        &self.provenance
    }
    fn provenance_mut(&mut self) -> &mut SchemaProvenance {
        &mut self.provenance
    }
    fn excluded(&self) -> bool {
        self.excluded
    }
    fn match_keys(&self) -> &[String] {
        &self.match_keys
    }
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self> {
        adapter.importable_metrics()
    }
    fn upgrade_associations<C: SchemaCacheLookup>(
        &mut self,
        cache_lookup: &C,
    ) -> Result<(), Error> {
        upgrade_association_origins(&mut self.association_origins, cache_lookup)
    }
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error> {
        for (attr, _, attr_origin, is_v1_def) in &mut self.attributes {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        Ok(())
    }
}

impl ImportableDepItem for DepImportedSpan {
    const FIELD: ImportField = ImportField::Spans;
    fn key(&self) -> &SignalId {
        &self.r#type
    }
    fn origin(&self) -> &SchemaUrl {
        &self.origin
    }
    fn set_origin(&mut self, origin: SchemaUrl) {
        self.origin = origin;
    }
    fn provenance(&self) -> &SchemaProvenance {
        &self.provenance
    }
    fn provenance_mut(&mut self) -> &mut SchemaProvenance {
        &mut self.provenance
    }
    fn excluded(&self) -> bool {
        self.excluded
    }
    fn match_keys(&self) -> &[String] {
        &self.match_keys
    }
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self> {
        adapter.importable_spans()
    }
    fn upgrade_associations<C: SchemaCacheLookup>(
        &mut self,
        cache_lookup: &C,
    ) -> Result<(), Error> {
        upgrade_association_origins(&mut self.association_origins, cache_lookup)
    }
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error> {
        for (attr, _, _, attr_origin, is_v1_def) in &mut self.attributes {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        Ok(())
    }
}

impl ImportableDepItem for DepImportedEvent {
    const FIELD: ImportField = ImportField::Events;
    fn key(&self) -> &SignalId {
        &self.name
    }
    fn origin(&self) -> &SchemaUrl {
        &self.origin
    }
    fn set_origin(&mut self, origin: SchemaUrl) {
        self.origin = origin;
    }
    fn provenance(&self) -> &SchemaProvenance {
        &self.provenance
    }
    fn provenance_mut(&mut self) -> &mut SchemaProvenance {
        &mut self.provenance
    }
    fn excluded(&self) -> bool {
        self.excluded
    }
    fn match_keys(&self) -> &[String] {
        &self.match_keys
    }
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self> {
        adapter.importable_events()
    }
    fn upgrade_associations<C: SchemaCacheLookup>(
        &mut self,
        cache_lookup: &C,
    ) -> Result<(), Error> {
        upgrade_association_origins(&mut self.association_origins, cache_lookup)
    }
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error> {
        for (attr, _, attr_origin, is_v1_def) in &mut self.attributes {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        Ok(())
    }
}

impl ImportableDepItem for DepImportedEntity {
    const FIELD: ImportField = ImportField::Entities;
    fn key(&self) -> &SignalId {
        &self.r#type
    }
    fn origin(&self) -> &SchemaUrl {
        &self.origin
    }
    fn set_origin(&mut self, origin: SchemaUrl) {
        self.origin = origin;
    }
    fn provenance(&self) -> &SchemaProvenance {
        &self.provenance
    }
    fn provenance_mut(&mut self) -> &mut SchemaProvenance {
        &mut self.provenance
    }
    fn excluded(&self) -> bool {
        self.excluded
    }
    fn match_keys(&self) -> &[String] {
        &self.match_keys
    }
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self> {
        adapter.importable_entities()
    }
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error> {
        for (attr, attr_origin, is_v1_def) in &mut self.identity {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        for (attr, _, attr_origin, is_v1_def) in &mut self.description {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        Ok(())
    }
}

impl ImportableDepItem for DepImportedAttributeGroup {
    const FIELD: ImportField = ImportField::AttributeGroups;
    fn key(&self) -> &SignalId {
        &self.id
    }
    fn origin(&self) -> &SchemaUrl {
        &self.origin
    }
    fn set_origin(&mut self, origin: SchemaUrl) {
        self.origin = origin;
    }
    fn provenance(&self) -> &SchemaProvenance {
        &self.provenance
    }
    fn provenance_mut(&mut self) -> &mut SchemaProvenance {
        &mut self.provenance
    }
    fn excluded(&self) -> bool {
        self.excluded
    }
    fn match_keys(&self) -> &[String] {
        &self.match_keys
    }
    fn adapter_items(adapter: &DependencyAdapter) -> Vec<Self> {
        adapter.importable_attribute_groups()
    }
    fn register_attributes<C: SchemaCacheLookup>(
        &mut self,
        deps: &V2Dependencies,
        cache_lookup: &C,
        catalog: &mut Catalog<'_, C>,
    ) -> Result<(), Error> {
        for (attr, _, attr_origin, is_v1_def) in &mut self.attributes {
            register_imported_attr(deps, cache_lookup, catalog, attr, attr_origin, *is_v1_def)?;
        }
        Ok(())
    }
}

/// Looks up `key` in `chosen_url`, populating `cache` on first access per schema.
fn lookup_upgraded_item<T: ImportableDepItem>(
    cache: &mut HashMap<SchemaUrl, HashMap<SignalId, T>>,
    chosen_url: &SchemaUrl,
    key: &SignalId,
    deps: &V2Dependencies,
    cache_lookup: &impl SchemaCacheLookup,
) -> Result<Option<T>, Error> {
    if !cache.contains_key(chosen_url) {
        let chosen_schema = lookup_chosen_schema(chosen_url, cache_lookup)?;
        let chosen_adapter = deps.adapter_for(chosen_url, &chosen_schema);
        let items = T::adapter_items(&chosen_adapter);
        let mut map = HashMap::with_capacity(items.len());
        for item in items {
            let _ = map.entry(item.key().clone()).or_insert(item);
        }
        let _ = cache.insert(chosen_url.clone(), map);
    }
    Ok(cache.get(chosen_url).and_then(|map| map.get(key).cloned()))
}

/// Matches, upgrades, deduplicates, and registers imported items of a single category.
fn resolve_category<'a, T: ImportableDepItem, C: SchemaCacheLookup>(
    schema_url: &SchemaUrl,
    local_seeds: impl IntoIterator<Item = (&'a SignalId, &'a str)>,
    any_matchers: &HashMap<ImportField, GlobSet>,
    explicit_matchers: &HashMap<ImportField, GlobSet>,
    matched_patterns: &mut HashMap<ImportField, Vec<bool>>,
    deps: &V2Dependencies,
    catalog: &mut Catalog<'_, C>,
    cache_lookup: &C,
) -> Result<(Vec<T>, BTreeMap<String, Vec<SemconvProvenance>>), Error> {
    let any_matcher = &any_matchers[&T::FIELD];
    let explicit_matcher = &explicit_matchers[&T::FIELD];
    let matched_flags = matched_patterns
        .get_mut(&T::FIELD)
        .expect("pattern flags initialized");

    let mut rivals: BTreeMap<String, Vec<SemconvProvenance>> = BTreeMap::new();
    for (id, path) in local_seeds {
        rivals
            .entry(id.to_string())
            .or_default()
            .push(SemconvProvenance::new(schema_url.clone(), path));
    }

    let mut raw_items: Vec<T> = Vec::new();
    let mut upgraded_cache: HashMap<SchemaUrl, HashMap<SignalId, T>> = HashMap::new();

    for adapter in deps.adapters() {
        for mut item in T::adapter_items(adapter) {
            if item.excluded() {
                continue;
            }
            if !item.match_keys().iter().any(|k| any_matcher.is_match(k)) {
                continue;
            }
            for k in item.match_keys() {
                for idx in explicit_matcher.matches(k) {
                    matched_flags[idx] = true;
                }
            }

            if let Some(chosen_url) = chosen_version_if_superseded(item.origin(), cache_lookup)? {
                let upgraded = lookup_upgraded_item(
                    &mut upgraded_cache,
                    &chosen_url,
                    item.key(),
                    deps,
                    cache_lookup,
                )?;
                let Some(mut upgraded) = upgraded else {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: item.key().to_string(),
                        original_url: item.origin().to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                };
                if upgraded.excluded() {
                    return Err(Error::GroupNotFoundInUpgradedSchema {
                        group_id: item.key().to_string(),
                        original_url: item.origin().to_string(),
                        upgraded_url: chosen_url.to_string(),
                    });
                }
                upgraded.upgrade_associations(cache_lookup)?;
                upgraded.set_origin(chosen_url);
                item = upgraded;
            }

            raw_items.push(item);
        }
    }

    let mut kept_items = Vec::new();
    let mut seen_buckets: HashSet<(String, String)> = HashSet::new();
    for item in raw_items {
        let bucket_key = (item.origin().to_string(), item.key().to_string());
        if !seen_buckets.insert(bucket_key) {
            continue;
        }
        rivals
            .entry(item.key().to_string())
            .or_default()
            .push(SemconvProvenance::new(
                item.origin().clone(),
                &item.provenance().path,
            ));
        kept_items.push(item);
    }

    for item in &mut kept_items {
        item.register_attributes(deps, cache_lookup, catalog)?;
        let source = deps.dependency_ref(item.origin());
        item.provenance_mut().source = source;
    }

    Ok((kept_items, rivals))
}

/// Secondary duplicate-name diagnostic emitted alongside `DuplicateGroupId` on rival imports.
#[derive(Clone, Copy)]
enum RivalNameWarning {
    None,
    MetricName,
    GroupName,
}

/// Appends `DuplicateGroupId` and optional name collision warnings when multiple distinct origins define the same item.
fn collect_rival_warnings(
    rivals: BTreeMap<String, Vec<SemconvProvenance>>,
    id_prefix: Option<&str>,
    name_warning: RivalNameWarning,
    duplicate_group_ids: &mut Vec<Error>,
    duplicate_metric_names: &mut Vec<Error>,
    duplicate_group_names: &mut Vec<Error>,
) {
    for (name, provs) in rivals {
        let unique_provs: Vec<SemconvProvenance> = provs.into_iter().unique().collect();
        if unique_provs.len() > 1 {
            let group_id = match id_prefix {
                Some(prefix) => format!("{prefix}.{name}"),
                None => name.clone(),
            };
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id,
                provenances: unique_provs.clone(),
            });
            match name_warning {
                RivalNameWarning::None => {}
                RivalNameWarning::MetricName => {
                    duplicate_metric_names.push(Error::DuplicateMetricName {
                        metric_name: name,
                        provenances: unique_provs,
                    });
                }
                RivalNameWarning::GroupName => {
                    duplicate_group_names.push(Error::DuplicateGroupName {
                        group_name: name,
                        provenances: unique_provs,
                    });
                }
            }
        }
    }
}

/// Records a `DuplicateGroupId` error when an imported signal collides with a local refinement ID.
fn collect_imported_refinement_collisions<I: ImportableDepItem, R>(
    imported: &[I],
    local_refinements: &BTreeMap<SignalId, Indexed<R>>,
    duplicate_group_ids: &mut Vec<Error>,
) {
    for item in imported {
        let key = item.key();
        if let Some(refinement) = local_refinements.get(key) {
            duplicate_group_ids.push(Error::DuplicateGroupId {
                group_id: key.to_string(),
                provenances: vec![
                    refinement.provenance.clone(),
                    SemconvProvenance::new(item.origin().clone(), ""),
                ],
            });
        }
    }
}

/// Resolves `imports:` blocks against dependencies, applying version upgrades and deduplication.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve<'a, C: SchemaCacheLookup>(
    schema_url: &SchemaUrl,
    local_index: &Index,
    local_signals: &ResolvedSignals,
    expanded_groups: &ExpandedGroups,
    imports: &[IndexedImports],
    deps: &V2Dependencies,
    catalog: &mut Catalog<'a, C>,
    cache_lookup: &C,
) -> Result<(ImportedItems, Vec<Error>), Error> {
    let explicit_imports: Vec<&IndexedImports> =
        imports.iter().filter(|i| i.origin.is_explicit()).collect();

    let mut explicit_matchers = HashMap::new();
    let mut any_matchers = HashMap::new();
    let mut explicit_patterns_map = HashMap::new();
    let mut matched_patterns: HashMap<ImportField, Vec<bool>> = HashMap::new();

    for field in ImportField::all() {
        let exp_pats: Vec<&GroupWildcard> = explicit_imports
            .iter()
            .flat_map(|i| field.patterns(&i.imports))
            .collect();
        let exp_globset = build_globset(exp_pats.iter().map(|w| &w.0))?;
        let count = exp_pats.len();
        _ = explicit_patterns_map.insert(field, exp_pats);
        _ = explicit_matchers.insert(field, exp_globset);
        _ = matched_patterns.insert(field, vec![false; count]);

        let any_pats: Vec<&GroupWildcard> = imports
            .iter()
            .flat_map(|i| field.patterns(&i.imports))
            .collect();
        let any_globset = build_globset(any_pats.iter().map(|w| &w.0))?;
        _ = any_matchers.insert(field, any_globset);
    }

    let (metrics, metric_rivals) = resolve_category::<DepImportedMetric, _>(
        schema_url,
        local_signals
            .metrics
            .iter()
            .map(|m| (&m.name, m.provenance.path.as_str())),
        &any_matchers,
        &explicit_matchers,
        &mut matched_patterns,
        deps,
        catalog,
        cache_lookup,
    )?;

    let (spans, span_rivals) = resolve_category::<DepImportedSpan, _>(
        schema_url,
        local_signals
            .spans
            .iter()
            .map(|s| (&s.r#type, s.provenance.path.as_str())),
        &any_matchers,
        &explicit_matchers,
        &mut matched_patterns,
        deps,
        catalog,
        cache_lookup,
    )?;

    let (events, event_rivals) = resolve_category::<DepImportedEvent, _>(
        schema_url,
        local_signals
            .events
            .iter()
            .map(|e| (&e.name, e.provenance.path.as_str())),
        &any_matchers,
        &explicit_matchers,
        &mut matched_patterns,
        deps,
        catalog,
        cache_lookup,
    )?;

    let (entities, entity_rivals) = resolve_category::<DepImportedEntity, _>(
        schema_url,
        local_signals
            .entities
            .iter()
            .map(|en| (&en.r#type, en.provenance.path.as_str())),
        &any_matchers,
        &explicit_matchers,
        &mut matched_patterns,
        deps,
        catalog,
        cache_lookup,
    )?;

    let (attribute_groups, ag_rivals) = resolve_category::<DepImportedAttributeGroup, _>(
        schema_url,
        expanded_groups
            .public_groups()
            .iter()
            .map(|ag| (&ag.id, ag.provenance.path.as_str())),
        &any_matchers,
        &explicit_matchers,
        &mut matched_patterns,
        deps,
        catalog,
        cache_lookup,
    )?;

    let mut duplicate_group_ids = Vec::new();
    let mut duplicate_metric_names = Vec::new();
    let mut duplicate_group_names = Vec::new();

    collect_rival_warnings(
        metric_rivals,
        Some("metric"),
        RivalNameWarning::MetricName,
        &mut duplicate_group_ids,
        &mut duplicate_metric_names,
        &mut duplicate_group_names,
    );
    collect_rival_warnings(
        span_rivals,
        Some("span"),
        RivalNameWarning::GroupName,
        &mut duplicate_group_ids,
        &mut duplicate_metric_names,
        &mut duplicate_group_names,
    );
    collect_rival_warnings(
        event_rivals,
        Some("event"),
        RivalNameWarning::GroupName,
        &mut duplicate_group_ids,
        &mut duplicate_metric_names,
        &mut duplicate_group_names,
    );
    collect_rival_warnings(
        entity_rivals,
        Some("entity"),
        RivalNameWarning::GroupName,
        &mut duplicate_group_ids,
        &mut duplicate_metric_names,
        &mut duplicate_group_names,
    );
    collect_rival_warnings(
        ag_rivals,
        None,
        RivalNameWarning::None,
        &mut duplicate_group_ids,
        &mut duplicate_metric_names,
        &mut duplicate_group_names,
    );

    collect_imported_refinement_collisions(
        &metrics,
        local_index.metric_refinements(),
        &mut duplicate_group_ids,
    );
    collect_imported_refinement_collisions(
        &spans,
        local_index.span_refinements(),
        &mut duplicate_group_ids,
    );
    collect_imported_refinement_collisions(
        &events,
        local_index.event_refinements(),
        &mut duplicate_group_ids,
    );
    collect_imported_refinement_collisions(
        &entities,
        local_index.entity_refinements(),
        &mut duplicate_group_ids,
    );

    let mut collision_errors = Vec::new();
    collision_errors.extend(duplicate_group_ids);
    collision_errors.extend(duplicate_metric_names);
    collision_errors.extend(duplicate_group_names);
    handle_errors(collision_errors)?;

    let mut warnings = Vec::new();
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

    Ok((
        ImportedItems {
            metrics,
            spans,
            events,
            entities,
            attribute_groups,
        },
        warnings,
    ))
}
