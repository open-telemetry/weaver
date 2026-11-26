// SPDX-License-Identifier: Apache-2.0

//! Resolves `imports` glob patterns against V1 and V2 dependency schemas.
//! Matches visible signals, converts V2 signals to V1 groups, upgrades superseded versions, and deduplicates diamond imports.

use globset::GlobSet;
use std::collections::{BTreeMap, HashMap};
use weaver_resolved_schema::v1::attribute::{Attribute as V1Attribute, AttributeRef};
use weaver_resolved_schema::v1::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::v1::registry::Group;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::attribute::{
    Attribute as V2Attribute, AttributeRef as V2AttributeRef,
};
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2Catalog;
use weaver_resolved_schema::v2::entity::EntityAssociation as V2EntityAssociation;
use weaver_resolved_schema::v2::provenance::Provenance as SchemaProvenance;
use weaver_resolved_schema::v2::span::SpanLink as V2SpanLink;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::v2::Signal;
use weaver_semconv::convert::{
    v2_attribute_type_to_v1, v2_examples_to_v1, v2_instrument_to_v1, v2_requirement_level_to_v1,
    v2_span_kind_to_v1, v2_span_name_to_v1,
};
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::{AttributeRole, BasicRequirementLevelSpec, RequirementLevel};
use weaver_semconv::v1::entity_association::EntityAssociation as V1EntityAssociation;
use weaver_semconv::v1::group::{
    AttributeGroupVisibilitySpec, GroupType, GroupWildcard, ImportsWithProvenance,
    LinkAttributeRef as V1LinkAttributeRef, SpanLink as V1SpanLink,
};
use weaver_semconv::v1::semconv::Imports;
use weaver_semconv::v2::CommonFields;
use weaver_semconv::YamlValue;

use crate::error::Error;
use crate::graph::chosen_versions::{
    chosen_version_if_superseded, lookup_chosen_schema, SchemaCacheLookup,
};
use crate::graph::entity_lookup::{v2_association_origins, v2_source_url, EntityLookup};
use crate::graph::exclusion::is_excluded;
use crate::graph::imports::{build_globset, ImportField};
use crate::v1::attribute::{AttributeCatalog, AttributeSource};
use crate::v1::dependency::v1_dep::find_attribute_source;
use crate::v1::dependency::ResolvedDependency;
use crate::v1::upgrade::upgrade_entity_origin;
use crate::WeaverResolvedSchema;

/// Maps a V1 `GroupType` to its corresponding `imports:` category, or `None` if not importable.
fn import_field_of(group_type: &GroupType) -> Option<ImportField> {
    match group_type {
        GroupType::Metric | GroupType::MetricGroup => Some(ImportField::Metrics),
        GroupType::Event => Some(ImportField::Events),
        GroupType::Span => Some(ImportField::Spans),
        GroupType::Entity => Some(ImportField::Entities),
        GroupType::AttributeGroup => Some(ImportField::AttributeGroups),
        GroupType::Scope | GroupType::Undefined => None,
    }
}

/// Returns the glob patterns configured for `field` in a V1 `Imports` block.
fn import_field_patterns(field: ImportField, imports: &Imports) -> &[GroupWildcard] {
    let list = match field {
        ImportField::Metrics => &imports.metrics,
        ImportField::Events => &imports.events,
        ImportField::Spans => &imports.spans,
        ImportField::Entities => &imports.entities,
        ImportField::AttributeGroups => &imports.attribute_groups,
    };
    list.as_deref().unwrap_or_default()
}

/// Returns `true` if `import` was synthesized by `--include-unreferenced` rather than authored.
fn is_implicit_import(import: &ImportsWithProvenance) -> bool {
    import.provenance.path == "--include-unreferenced"
}

/// Returns `Error::UnmatchedImport` for each explicit import pattern that matched no imported group across all dependencies.
pub(crate) fn unmatched_import_errors(
    imports: &[ImportsWithProvenance],
    groups: &[GroupWithProvenance],
) -> Result<Vec<Error>, Error> {
    let explicit: Vec<&ImportsWithProvenance> =
        imports.iter().filter(|i| !is_implicit_import(i)).collect();
    let mut errors = vec![];
    for field in ImportField::all() {
        let patterns: Vec<&GroupWildcard> = explicit
            .iter()
            .flat_map(|i| import_field_patterns(field, &i.imports))
            .collect();
        if patterns.is_empty() {
            continue;
        }
        // Built from the same list, in the same order, so a match index is an
        // index into `patterns`.
        let matcher = build_globset(patterns.iter().map(|w| &w.0))?;
        let mut matched = vec![false; patterns.len()];
        for group in groups {
            if import_field_of(&group.group.r#type) != Some(field) {
                continue;
            }
            for key in import_match_keys(&group.group) {
                for index in matcher.matches(key) {
                    matched[index] = true;
                }
            }
        }
        errors.extend(
            patterns
                .iter()
                .zip(matched)
                .filter(|(_, matched)| !matched)
                .map(|(pattern, _)| Error::UnmatchedImport {
                    pattern: pattern.0.glob().to_owned(),
                    signal: field.name().to_owned(),
                }),
        );
    }
    Ok(errors)
}

/// Returns candidate identifier keys against which `imports` glob patterns match `g`,
/// including legacy `registry.` and V1 `resource` prefixes.
pub(crate) fn import_match_keys(g: &Group) -> Vec<&str> {
    let name = g.name.as_deref();
    let metric_name = g.metric_name.as_deref();
    if g.is_v2 {
        let (signal_name, prefix) = match g.r#type {
            GroupType::AttributeGroup => (None, "attribute_group."),
            GroupType::Span => (name, "span."),
            GroupType::Event => (name, "event."),
            GroupType::Metric | GroupType::MetricGroup => (metric_name, "metric."),
            GroupType::Entity => (name, "entity."),
            GroupType::Scope | GroupType::Undefined => return vec![],
        };
        let mut keys = vec![g.id.as_str()];
        keys.extend(signal_name);
        // TODO - warn on the `registry.` spelling to move authors off it.
        if g.r#type == GroupType::AttributeGroup {
            keys.extend(g.id.strip_prefix("registry."));
        }
        keys.extend(g.id.strip_prefix(prefix));
        keys
    } else {
        match g.r#type {
            GroupType::AttributeGroup | GroupType::Span => vec![g.id.as_str()],
            GroupType::Event => name.into_iter().collect(),
            // Legacy V1 `resource` groups omit `name` and store the entity type in `id`.
            // TODO - warn on a v1 `resource` entity to move authors to v2.
            GroupType::Entity => {
                let mut keys = vec![g.id.as_str()];
                keys.extend(g.id.strip_prefix("entity."));
                if let Some(name) = name.filter(|n| !keys.contains(n)) {
                    keys.push(name);
                }
                keys
            }
            GroupType::Metric => metric_name.into_iter().collect(),
            GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => vec![],
        }
    }
}

/// Compiled `GlobSet` matchers for each field of an `imports` block.
struct ImportMatchers {
    metrics: GlobSet,
    events: GlobSet,
    spans: GlobSet,
    entities: GlobSet,
    attribute_groups: GlobSet,
}

impl ImportMatchers {
    /// Compiles all patterns across `imports` into per-field `GlobSet` matchers.
    fn build<'a>(
        imports: impl Iterator<Item = &'a ImportsWithProvenance> + Clone,
    ) -> Result<Self, Error> {
        let for_field = |field: ImportField| {
            build_globset(
                imports
                    .clone()
                    .flat_map(move |i| import_field_patterns(field, &i.imports))
                    .map(|w| &w.0),
            )
        };
        Ok(Self {
            metrics: for_field(ImportField::Metrics)?,
            events: for_field(ImportField::Events)?,
            spans: for_field(ImportField::Spans)?,
            entities: for_field(ImportField::Entities)?,
            attribute_groups: for_field(ImportField::AttributeGroups)?,
        })
    }

    /// Returns the compiled `GlobSet` for `field`.
    fn field(&self, field: ImportField) -> &GlobSet {
        match field {
            ImportField::Metrics => &self.metrics,
            ImportField::Events => &self.events,
            ImportField::Spans => &self.spans,
            ImportField::Entities => &self.entities,
            ImportField::AttributeGroups => &self.attribute_groups,
        }
    }

    /// Returns `true` if any pattern in the matching field matches `g`.
    fn matches(&self, g: &Group) -> bool {
        let Some(field) = import_field_of(&g.r#type) else {
            return false;
        };
        let matcher = self.field(field);
        import_match_keys(g)
            .into_iter()
            .any(|k| matcher.is_match(k))
    }

    /// Returns `true` if any pattern in `field` matches `key`.
    fn matches_key(&self, field: ImportField, key: &str) -> bool {
        self.field(field).is_match(key)
    }
}

/// Imported group paired with its origin schema URL and resolved entity association origins.
pub struct GroupWithProvenance {
    /// Imported group definition.
    pub group: Group,
    /// Schema URL of the defining registry.
    pub schema_url: SchemaUrl,
    /// Defining registry URL for each entity referenced in `group.entity_associations`,
    /// preserved from the declaring registry.
    pub association_origins: BTreeMap<String, SchemaUrl>,
}

/// Imports matching groups from a resolved dependency schema.
pub(crate) trait ImportableDependency {
    /// Returns groups matching `imports` and registers their attributes in `attribute_catalog`.
    fn import_groups<C: SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error>;
}

/// Resolves the defining `SchemaUrl` for each entity associated with `group` in `schema`,
/// falling back to entity lookup when `entity_association_origins` is unpopulated.
pub(crate) fn v1_association_origins(
    schema: &V1Schema,
    group: &Group,
) -> BTreeMap<String, SchemaUrl> {
    let recorded = schema.registry.entity_association_origins.get(&group.id);
    group
        .entity_associations
        .iter()
        .flat_map(|assoc| assoc.referenced_entities())
        .filter_map(|name| {
            let origin = match recorded.and_then(|origins| origins.get(name)) {
                Some(origin) => Some(origin.clone()),
                None => schema
                    .lookup_entity(name)
                    .filter(|location| !location.excluded)
                    .map(|location| location.origin),
            };
            origin.map(|origin| (name.to_owned(), origin))
        })
        .collect()
}

impl ImportableDependency for V1Schema {
    fn import_groups<C: SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        let explicit_imports: Vec<&ImportsWithProvenance> =
            imports.iter().filter(|i| !is_implicit_import(i)).collect();

        let explicit = ImportMatchers::build(explicit_imports.iter().copied())?;
        let any = ImportMatchers::build(imports.iter())?;
        let matches_explicitly = |g: &Group| explicit.matches(g);
        let matches_by_any = |g: &Group| any.matches(g);

        let mut exclusion_errors: Vec<Error> = vec![];
        let mut result: Vec<GroupWithProvenance> = vec![];
        let my_schema_url =
            SchemaUrl::try_from(self.schema_url.as_str()).map_err(|e| Error::InvalidUrl {
                url: self.schema_url.to_string(),
                error: e,
            })?;

        for g in self.registry.groups.iter() {
            let matched_explicitly = matches_explicitly(g);
            let matched_by_any = matches_by_any(g);
            if !matched_by_any {
                continue;
            }
            let decision = g
                .annotations
                .as_ref()
                .map(|a| import_decision(a, matched_explicitly, &g.id, g.r#type.clone()))
                .unwrap_or(ImportDecision::Include);
            match decision {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(e) => {
                    exclusion_errors.push(e);
                    continue;
                }
            }
            let (g, g_url) = if let Some(upgraded) =
                upgrade_imported_group(g, &my_schema_url, attribute_catalog, cache_lookup)?
            {
                let upgraded_url = upgraded
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| {
                        let origin = origin_url(g, &my_schema_url);
                        cache_lookup
                            .chosen_version(origin.name())
                            .cloned()
                            .unwrap_or(origin)
                    });
                (upgraded, upgraded_url)
            } else {
                let mut g = g.clone();
                let mut attributes = vec![];
                for a in g
                    .attributes
                    .iter()
                    .filter_map(|ar| self.catalog().attribute(ar))
                {
                    let source = find_attribute_source(self, &a.name, &my_schema_url);
                    let ar = attribute_catalog.attribute_ref_with_provenance(
                        a.clone(),
                        source,
                        cache_lookup,
                    )?;
                    attributes.push(ar);
                }
                g.attributes = attributes;
                let original_url = g
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| my_schema_url.clone());
                (g, original_url)
            };
            let mut association_origins = v1_association_origins(self, &g);
            for (entity_name, origin) in association_origins.iter_mut() {
                *origin = upgrade_entity_origin(entity_name, origin, cache_lookup)?;
            }
            result.push(GroupWithProvenance {
                group: g,
                schema_url: g_url,
                association_origins,
            });
        }
        if !exclusion_errors.is_empty() {
            return Err(Error::CompoundError(exclusion_errors));
        }
        Ok(result)
    }
}

/// Upgrades an imported group to its chosen dependency version when its origin was superseded,
/// returning `Ok(None)` if no upgrade applies or `Error::GroupNotFoundInUpgradedSchema` if removed.
fn upgrade_imported_group<C: SchemaCacheLookup>(
    group: &Group,
    fallback_url: &SchemaUrl,
    attribute_catalog: &mut AttributeCatalog,
    cache_lookup: &C,
) -> Result<Option<Group>, Error> {
    let origin_url = origin_url(group, fallback_url);
    let Some(chosen_url) = chosen_version_if_superseded(&origin_url, cache_lookup)? else {
        return Ok(None);
    };
    let chosen_schema = lookup_chosen_schema(&chosen_url, cache_lookup)?;
    match chosen_schema.as_ref() {
        WeaverResolvedSchema::V1(chosen_v1) => {
            let Some(upgraded) = chosen_v1
                .registry
                .groups
                .iter()
                .find(|candidate| is_same_imported_group(group, candidate))
            else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let mut upgraded = upgraded.clone();
            let mut attributes = vec![];
            for a in upgraded
                .attributes
                .iter()
                .filter_map(|ar| chosen_v1.catalog().attribute(ar))
            {
                let source = find_attribute_source(chosen_v1, &a.name, &chosen_url);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    a.clone(),
                    source,
                    cache_lookup,
                )?);
            }
            upgraded.attributes = attributes;
            Ok(Some(upgraded))
        }
        WeaverResolvedSchema::V2(chosen_v2) => upgrade_imported_group_v2(
            group,
            &origin_url,
            &chosen_url,
            chosen_v2,
            attribute_catalog,
            cache_lookup,
        )
        .map(Some),
    }
}

/// Rebuilds an imported group from a superseded dependency's upgraded V2 schema.
fn upgrade_imported_group_v2<C: SchemaCacheLookup>(
    group: &Group,
    origin_url: &SchemaUrl,
    chosen_url: &SchemaUrl,
    chosen_v2: &V2Schema,
    attribute_catalog: &mut AttributeCatalog,
    cache_lookup: &C,
) -> Result<Group, Error> {
    let deps: Vec<SchemaUrl> = chosen_v2.dependencies.iter().cloned().collect();
    match group.r#type {
        GroupType::Metric => {
            let Some(m) = chosen_v2.registry.metrics.iter().find(|m| {
                group.metric_name.as_deref() == Some(m.name.as_ref())
                    || strip_group_type_prefix(&group.id) == strip_group_type_prefix(m.id())
            }) else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let attributes = import_v2_attributes(
                chosen_v2,
                &deps,
                m.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut upgraded = imported_v2_group(
                m.id().to_owned(),
                GroupType::Metric,
                &m.common,
                attributes,
                GroupLineage::new(v2_provenance(chosen_v2, &deps, &m.provenance)),
            );
            upgraded.metric_name = Some(m.name.to_string());
            upgraded.instrument = Some(v2_instrument_to_v1(m.instrument));
            upgraded.unit = Some(m.unit.clone());
            upgraded.entity_associations = to_named_associations(&m.entity_associations);
            upgraded.requirement_level = m.requirement_level.clone().map(Into::into);
            Ok(upgraded)
        }
        GroupType::Event => {
            let Some(e) = chosen_v2.registry.events.iter().find(|e| {
                group.name.as_deref() == Some(e.name.as_ref())
                    || strip_group_type_prefix(&group.id) == strip_group_type_prefix(e.id())
            }) else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let attributes = import_v2_attributes(
                chosen_v2,
                &deps,
                e.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut upgraded = imported_v2_group(
                e.id().to_owned(),
                GroupType::Event,
                &e.common,
                attributes,
                GroupLineage::new(v2_provenance(chosen_v2, &deps, &e.provenance)),
            );
            upgraded.name = Some(e.name.to_string());
            upgraded.entity_associations = to_named_associations(&e.entity_associations);
            upgraded.requirement_level = e.requirement_level.clone().map(Into::into);
            Ok(upgraded)
        }
        GroupType::Entity => {
            let Some(e) = chosen_v2.registry.entities.iter().find(|e| {
                group.name.as_deref() == Some(e.r#type.as_ref())
                    || strip_group_type_prefix(&group.id) == strip_group_type_prefix(e.id())
            }) else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let attributes = import_v2_attributes(
                chosen_v2,
                &deps,
                e.identity
                    .iter()
                    .map(|ar| {
                        V2SignalAttribute::new(
                            ar,
                            RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                        )
                        .with_role(AttributeRole::Identifying)
                    })
                    .chain(e.description.iter().map(|ar| {
                        V2SignalAttribute::new(
                            &ar.base,
                            v2_requirement_level_to_v1(ar.requirement_level.clone()),
                        )
                        .with_role(AttributeRole::Descriptive)
                    })),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut upgraded = imported_v2_group(
                e.id().to_owned(),
                GroupType::Entity,
                &e.common,
                attributes,
                GroupLineage::new(v2_provenance(chosen_v2, &deps, &e.provenance)),
            );
            upgraded.name = Some(e.r#type.to_string());
            upgraded.requirement_level = e.requirement_level.clone().map(Into::into);
            Ok(upgraded)
        }
        GroupType::Span => {
            let Some(s) = chosen_v2.registry.spans.iter().find(|s| {
                group.name.as_deref() == Some(s.r#type.as_ref())
                    || strip_group_type_prefix(&group.id) == strip_group_type_prefix(s.id())
            }) else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let attributes = import_v2_attributes(
                chosen_v2,
                &deps,
                s.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                    .with_sampling_relevant(ar.sampling_relevant)
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut upgraded = imported_v2_group(
                s.id().to_owned(),
                GroupType::Span,
                &s.common,
                attributes,
                GroupLineage::new(v2_provenance(chosen_v2, &deps, &s.provenance)),
            );
            upgraded.span_kind = Some(v2_span_kind_to_v1(s.kind));
            upgraded.span_name = Some(v2_span_name_to_v1(s.name.clone()));
            upgraded.span_links = v2_span_links_to_v1(chosen_v2, &s.links)?;
            upgraded.name = Some(s.r#type.to_string());
            upgraded.entity_associations = to_named_associations(&s.entity_associations);
            upgraded.requirement_level = s.requirement_level.clone().map(Into::into);
            Ok(upgraded)
        }
        GroupType::AttributeGroup => {
            let Some(ag) =
                chosen_v2.registry.attribute_groups.iter().find(|ag| {
                    strip_group_type_prefix(&group.id) == strip_group_type_prefix(ag.id())
                })
            else {
                return Err(Error::GroupNotFoundInUpgradedSchema {
                    group_id: group.id.clone(),
                    original_url: origin_url.to_string(),
                    upgraded_url: chosen_url.to_string(),
                });
            };
            let attributes = import_v2_attributes(
                chosen_v2,
                &deps,
                ag.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            Ok(imported_v2_group(
                ag.id().to_owned(),
                GroupType::AttributeGroup,
                &ag.common,
                attributes,
                GroupLineage::new(v2_provenance(chosen_v2, &deps, &ag.provenance)),
            ))
        }
        _ => Err(Error::GroupNotFoundInUpgradedSchema {
            group_id: group.id.clone(),
            original_url: origin_url.to_string(),
            upgraded_url: chosen_url.to_string(),
        }),
    }
}

/// Returns `true` if `group` and `candidate` represent the same signal across V1 and V2 ID conventions.
fn is_same_imported_group(group: &Group, candidate: &Group) -> bool {
    if group.r#type != candidate.r#type {
        return false;
    }
    if group.metric_name.is_some() || candidate.metric_name.is_some() {
        return group.metric_name == candidate.metric_name;
    }
    if group.name.is_some() || candidate.name.is_some() {
        return group.name == candidate.name;
    }
    strip_group_type_prefix(&group.id) == strip_group_type_prefix(&candidate.id)
}

/// Strips a leading signal or `registry.` prefix from `id`, if present.
fn strip_group_type_prefix(id: &str) -> &str {
    for prefix in [
        "metric.",
        "event.",
        "entity.",
        "span.",
        "attribute_group.",
        "registry.",
    ] {
        if let Some(rest) = id.strip_prefix(prefix) {
            return rest;
        }
    }
    id
}

/// Visibility outcome for a candidate dependency item during import matching.
enum ImportDecision {
    /// Visible item; include in imported groups.
    Include,
    /// Excluded item matched only via implicit wildcard; omit without error.
    Skip,
    /// Excluded item matched by an explicit `imports` pattern; report as an error.
    Error(Error),
}

/// Determines whether an item with `annotations` should be imported, skipped, or rejected.
fn import_decision(
    annotations: &BTreeMap<String, YamlValue>,
    matched_explicitly: bool,
    id: &str,
    r#type: GroupType,
) -> ImportDecision {
    if !is_excluded(annotations) {
        return ImportDecision::Include;
    }
    if matched_explicitly {
        ImportDecision::Error(Error::ExcludedFromDependencyResolution {
            id: id.to_owned(),
            r#type: r#type.to_string(),
            used_in: "imports".to_owned(),
            provenance: None,
        })
    } else {
        ImportDecision::Skip
    }
}

/// Attribute reference from a V2 signal paired with per-signal overrides.
struct V2SignalAttribute<'a> {
    base: &'a V2AttributeRef,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
}

impl<'a> V2SignalAttribute<'a> {
    /// Creates a V2 signal attribute reference with `requirement_level` and no sampling or role overrides.
    fn new(base: &'a V2AttributeRef, requirement_level: RequirementLevel) -> Self {
        V2SignalAttribute {
            base,
            requirement_level,
            sampling_relevant: None,
            role: None,
        }
    }

    /// Sets the span `sampling_relevant` override on this reference.
    fn with_sampling_relevant(mut self, sampling_relevant: Option<bool>) -> Self {
        self.sampling_relevant = sampling_relevant;
        self
    }

    /// Sets the entity `AttributeRole` on this reference.
    fn with_role(mut self, role: AttributeRole) -> Self {
        self.role = Some(role);
        self
    }
}

/// Converts a V2 catalog attribute and per-signal reference overrides into a V1 `Attribute`.
fn convert_v2_attribute(
    attr: &V2Attribute,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
) -> V1Attribute {
    V1Attribute {
        name: attr.key.clone(),
        r#type: v2_attribute_type_to_v1(attr.r#type.clone()),
        brief: attr.common.brief.clone(),
        examples: attr.examples.clone().map(v2_examples_to_v1),
        tag: None,
        requirement_level,
        sampling_relevant,
        note: attr.common.note.clone().unwrap_or_default(),
        stability: Some(attr.common.stability.clone().into()),
        deprecated: attr.common.deprecated.clone(),
        prefix: false,
        tags: None,
        annotations: Some(attr.common.annotations.clone()),
        value: None,
        role,
    }
}

/// Converts V2 signal provenance into V1 `SemconvProvenance`.
fn v2_provenance(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    provenance: &SchemaProvenance,
) -> SemconvProvenance {
    SemconvProvenance::new(
        v2_source_url(schema, deps, provenance.source),
        &provenance.path,
    )
}

/// Catalog references and attribute lineage entries produced when importing a V2 signal.
struct ImportedAttributes {
    refs: Vec<AttributeRef>,
    lineage: Vec<(String, AttributeLineage)>,
}

/// Converts V2 attribute references to V1, interns them in `attribute_catalog`, and records their lineage.
fn import_v2_attributes<'a, C: SchemaCacheLookup>(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    refs: impl Iterator<Item = V2SignalAttribute<'a>>,
    attribute_catalog: &mut AttributeCatalog,
    cache_lookup: &C,
) -> Result<ImportedAttributes, Error> {
    let mut attributes = ImportedAttributes {
        refs: vec![],
        lineage: vec![],
    };
    for signal_attr in refs {
        let base = signal_attr.base;
        let attr =
            schema
                .attribute_catalog
                .attribute(base)
                .ok_or(Error::InvalidRegistryAttributeRef {
                    registry_name: schema.schema_url.name().to_owned(),
                    attribute_ref: base.0,
                })?;
        let schema_url = v2_source_url(schema, deps, attr.provenance.source);
        let v1_attr = convert_v2_attribute(
            attr,
            signal_attr.requirement_level,
            signal_attr.sampling_relevant,
            signal_attr.role,
        );
        attributes.lineage.push((
            v1_attr.name.clone(),
            imported_attribute_lineage(&schema_url, &v1_attr),
        ));
        attributes
            .refs
            .push(attribute_catalog.attribute_ref_with_provenance(
                v1_attr,
                AttributeSource::Dependency { schema_url },
                cache_lookup,
            )?);
    }
    Ok(attributes)
}

/// Builds `AttributeLineage` marking `attr` as an imported dependency reference from `schema_url` rather than a local definition.
fn imported_attribute_lineage(schema_url: &SchemaUrl, attr: &V1Attribute) -> AttributeLineage {
    let mut lineage = AttributeLineage::new(&format!("v2_dependency.{}", schema_url.name()));
    _ = lineage.brief(&None, &attr.brief);
    _ = lineage.examples(&None, &attr.examples);
    _ = lineage.note(&None, &attr.note);
    _ = lineage.stability(&None, &attr.stability);
    _ = lineage.deprecated(&None, &attr.deprecated);
    _ = lineage.annotations(&None, &attr.annotations);
    _ = lineage.requirement_level(
        &Some(attr.requirement_level.clone()),
        &attr.requirement_level,
    );
    _ = lineage.sampling_relevant(&attr.sampling_relevant, &None);
    _ = lineage.optional_role(&attr.role, &None);
    lineage
}

/// Constructs a V1 `Group` populated with fields common to all imported V2 signals.
fn imported_v2_group(
    id: String,
    r#type: GroupType,
    common: &CommonFields,
    attributes: ImportedAttributes,
    mut lineage: GroupLineage,
) -> Group {
    for (name, attr_lineage) in attributes.lineage {
        lineage.add_attribute_lineage(name, attr_lineage);
    }
    // Published V2 registries contain only public attribute groups.
    let visibility =
        matches!(r#type, GroupType::AttributeGroup).then_some(AttributeGroupVisibilitySpec::Public);
    Group {
        id,
        r#type,
        brief: common.brief.clone(),
        note: common.note.clone(),
        prefix: "".to_owned(),
        extends: None,
        stability: Some(common.stability.clone().into()),
        deprecated: common.deprecated.clone(),
        attributes: attributes.refs,
        span_kind: None,
        events: vec![],
        metric_name: None,
        instrument: None,
        unit: None,
        requirement_level: None,
        name: None,
        lineage: Some(lineage),
        display_name: None,
        body: None,
        annotations: Some(common.annotations.clone()),
        entity_associations: vec![],
        visibility,
        is_v2: true,
        span_name: None,
        span_links: Vec::new(),
    }
}

/// Converts a resolved v2 span's links into the v1 carrier shape: catalog
/// indices become attribute names via the owning schema's catalog.
fn v2_span_links_to_v1(schema: &V2Schema, links: &[V2SpanLink]) -> Result<Vec<V1SpanLink>, Error> {
    let mut v1_links = Vec::new();
    for link in links {
        let mut link_attributes = Vec::new();
        for la in link.attributes.iter() {
            let attr = schema.attribute_catalog.attribute(&la.base).ok_or(
                Error::InvalidRegistryAttributeRef {
                    registry_name: schema.schema_url.name().to_owned(),
                    attribute_ref: la.base.0,
                },
            )?;
            // The catalog entry may be an overridden variant; carry its
            // metadata as explicit overrides so it survives re-resolution.
            link_attributes.push(V1LinkAttributeRef {
                r#ref: attr.key.clone(),
                brief: Some(attr.common.brief.clone()),
                examples: attr.examples.clone().map(v2_examples_to_v1),
                requirement_level: Some(v2_requirement_level_to_v1(la.requirement_level.clone())),
                note: attr.common.note.clone(),
                annotations: attr.common.annotations.clone(),
            });
        }
        v1_links.push(V1SpanLink {
            r#ref: link.r#ref.to_string(),
            brief: link.brief.clone(),
            note: link.note.clone(),
            attributes: link_attributes,
        });
    }
    Ok(v1_links)
}

/// Converts V2 resolved entity associations into V1 name-based association expressions.
pub(crate) fn to_named_associations(
    associations: &[V2EntityAssociation],
) -> Vec<V1EntityAssociation> {
    associations
        .iter()
        .map(|assoc| match assoc {
            V2EntityAssociation::Ref(entity_ref) => {
                V1EntityAssociation::Ref(entity_ref.r#type.to_string())
            }
            V2EntityAssociation::OneOf { one_of } => V1EntityAssociation::OneOf {
                one_of: to_named_associations(one_of),
            },
            V2EntityAssociation::AllOf { all_of } => V1EntityAssociation::AllOf {
                all_of: to_named_associations(all_of),
            },
        })
        .collect()
}

impl ImportableDependency for V2Schema {
    fn import_groups<C: SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        let mut result = vec![];
        let mut exclusion_errors: Vec<Error> = vec![];
        let mut origins: HashMap<String, BTreeMap<String, SchemaUrl>> = HashMap::new();
        let deps: Vec<SchemaUrl> = self.dependencies.iter().cloned().collect();

        let explicit_imports: Vec<&ImportsWithProvenance> =
            imports.iter().filter(|i| !is_implicit_import(i)).collect();
        let explicit = ImportMatchers::build(explicit_imports.iter().copied())?;
        let any = ImportMatchers::build(imports.iter())?;

        // Match V2 signals by ID and collect explicit-import exclusion errors.
        let mut is_imported =
            |signal: &dyn Signal, field: ImportField, r#type: GroupType| -> bool {
                let key = signal.id();
                if !any.matches_key(field, key) {
                    return false;
                }
                match import_decision(
                    &signal.common().annotations,
                    explicit.matches_key(field, key),
                    key,
                    r#type,
                ) {
                    ImportDecision::Include => true,
                    ImportDecision::Skip => false,
                    ImportDecision::Error(e) => {
                        exclusion_errors.push(e);
                        false
                    }
                }
            };

        for m in self.registry.metrics.iter() {
            if !is_imported(m, ImportField::Metrics, GroupType::Metric) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                m.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                m.id().to_owned(),
                GroupType::Metric,
                &m.common,
                attributes,
                GroupLineage::new(v2_provenance(self, &deps, &m.provenance)),
            );
            group.metric_name = Some(m.name.to_string());
            group.instrument = Some(v2_instrument_to_v1(m.instrument));
            group.unit = Some(m.unit.clone());
            group.entity_associations = to_named_associations(&m.entity_associations);
            group.requirement_level = m.requirement_level.clone().map(Into::into);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &m.entity_associations),
            );
            result.push(group);
        }

        for e in self.registry.events.iter() {
            if !is_imported(e, ImportField::Events, GroupType::Event) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                e.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                e.id().to_owned(),
                GroupType::Event,
                &e.common,
                attributes,
                GroupLineage::new(v2_provenance(self, &deps, &e.provenance)),
            );
            group.name = Some(e.name.to_string());
            group.entity_associations = to_named_associations(&e.entity_associations);
            group.requirement_level = e.requirement_level.clone().map(Into::into);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &e.entity_associations),
            );
            result.push(group);
        }

        for e in self.registry.entities.iter() {
            if !is_imported(e, ImportField::Entities, GroupType::Entity) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                e.identity
                    .iter()
                    .map(|ar| {
                        V2SignalAttribute::new(
                            ar,
                            RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                        )
                        .with_role(AttributeRole::Identifying)
                    })
                    .chain(e.description.iter().map(|ar| {
                        V2SignalAttribute::new(
                            &ar.base,
                            v2_requirement_level_to_v1(ar.requirement_level.clone()),
                        )
                        .with_role(AttributeRole::Descriptive)
                    })),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                e.id().to_owned(),
                GroupType::Entity,
                &e.common,
                attributes,
                GroupLineage::new(v2_provenance(self, &deps, &e.provenance)),
            );
            group.name = Some(e.r#type.to_string());
            group.requirement_level = e.requirement_level.clone().map(Into::into);
            result.push(group);
        }

        for s in self.registry.spans.iter() {
            if !is_imported(s, ImportField::Spans, GroupType::Span) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                s.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                    .with_sampling_relevant(ar.sampling_relevant)
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                s.id().to_owned(),
                GroupType::Span,
                &s.common,
                attributes,
                GroupLineage::new(v2_provenance(self, &deps, &s.provenance)),
            );
            group.span_kind = Some(v2_span_kind_to_v1(s.kind));
            group.span_name = Some(v2_span_name_to_v1(s.name.clone()));
            group.span_links = v2_span_links_to_v1(self, &s.links)?;
            group.name = Some(s.r#type.to_string());
            group.entity_associations = to_named_associations(&s.entity_associations);
            group.requirement_level = s.requirement_level.clone().map(Into::into);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &s.entity_associations),
            );
            result.push(group);
        }

        for ag in self.registry.attribute_groups.iter() {
            if !is_imported(ag, ImportField::AttributeGroups, GroupType::AttributeGroup) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                ag.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(
                        &ar.base,
                        v2_requirement_level_to_v1(ar.requirement_level.clone()),
                    )
                }),
                attribute_catalog,
                cache_lookup,
            )?;
            result.push(imported_v2_group(
                ag.id().to_owned(),
                GroupType::AttributeGroup,
                &ag.common,
                attributes,
                GroupLineage::new(v2_provenance(self, &deps, &ag.provenance)),
            ));
        }

        if !exclusion_errors.is_empty() {
            return Err(Error::CompoundError(exclusion_errors));
        }

        let mut upgraded_urls = HashMap::new();
        for group in result.iter_mut() {
            if let Some(upgraded) =
                upgrade_imported_group(group, &self.schema_url, attribute_catalog, cache_lookup)?
            {
                let url = upgraded
                    .provenance()
                    .map(|p| p.schema_url.clone())
                    .unwrap_or_else(|| {
                        let origin = origin_url(group, &self.schema_url);
                        cache_lookup
                            .chosen_version(origin.name())
                            .cloned()
                            .unwrap_or(origin)
                    });
                _ = upgraded_urls.insert(upgraded.id.clone(), url);
                *group = upgraded;
            }
        }

        for group_origins in origins.values_mut() {
            for (entity_name, origin) in group_origins.iter_mut() {
                *origin = upgrade_entity_origin(entity_name, origin, cache_lookup)?;
            }
        }
        Ok(result
            .into_iter()
            .map(|group| {
                let schema_url = upgraded_urls
                    .get(&group.id)
                    .cloned()
                    .or_else(|| group.provenance().map(|p| p.schema_url.clone()))
                    .unwrap_or_else(|| self.schema_url.clone());
                GroupWithProvenance {
                    association_origins: origins.remove(&group.id).unwrap_or_default(),
                    group,
                    schema_url,
                }
            })
            .collect())
    }
}

impl ImportableDependency for ResolvedDependency {
    fn import_groups<C: SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        match self.as_schema() {
            WeaverResolvedSchema::V1(schema) => {
                schema.import_groups(imports, attribute_catalog, cache_lookup)
            }
            WeaverResolvedSchema::V2(schema) => {
                schema.import_groups(imports, attribute_catalog, cache_lookup)
            }
        }
    }
}

impl ImportableDependency for Vec<ResolvedDependency> {
    fn import_groups<C: SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        // Deduplicate groups imported via multiple diamond dependency paths, bucketing by
        // `(origin_url, group_type)` since `Group` does not implement `Hash`.
        let mut result: Vec<GroupWithProvenance> = vec![];
        let mut buckets: HashMap<(String, GroupType), Vec<usize>> = HashMap::new();
        for dependency in self {
            for candidate in dependency.import_groups(imports, attribute_catalog, cache_lookup)? {
                let key = (
                    origin_url(&candidate.group, &candidate.schema_url).to_string(),
                    candidate.group.r#type.clone(),
                );
                let bucket = buckets.entry(key).or_default();
                let is_duplicate = bucket
                    .iter()
                    .any(|&kept| is_same_imported_group(&result[kept].group, &candidate.group));
                if !is_duplicate {
                    bucket.push(result.len());
                    result.push(candidate);
                }
            }
        }
        Ok(result)
    }
}

/// Returns `group`'s provenance `SchemaUrl`, or `fallback_url` when unset.
fn origin_url(group: &Group, fallback_url: &SchemaUrl) -> SchemaUrl {
    group
        .provenance()
        .map(|prov| prov.schema_url)
        .unwrap_or_else(|| fallback_url.clone())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::ImportableDependency;
    use crate::v1::attribute::AttributeCatalog;
    use crate::v1::dependency::v1_dep::example_v1_schema;
    use crate::v1::dependency::v2_dep::example_v2_schema;
    use crate::v1::dependency::ResolvedDependency;

    #[test]
    fn test_import_groups_v1() -> Result<(), Box<dyn Error>> {
        let d = example_v1_schema();
        let mut catalog = AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::v1::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::v1::semconv::Imports {
                metrics: None,
                events: None,
                entities: None,
                spans: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("span.v1")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("a")?,
                )]),
            },
        }];

        // By default V1 example schema has an AttributeGroup and a Span.
        let result = d.import_groups(
            &imports,
            &mut catalog,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        )?;
        assert_eq!(
            result.len(),
            2,
            "Attribute group and span should be imported"
        );

        Ok(())
    }

    #[test]
    fn test_import_groups_v2() -> Result<(), Box<dyn Error>> {
        let d = example_v2_schema();
        let mut catalog = AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::v1::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::v1::semconv::Imports {
                metrics: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("metric.a")?,
                )]),
                events: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("event.b")?,
                )]),
                entities: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("entity.c")?,
                )]),
                spans: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("span.d")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e")?,
                )]),
            },
        }];

        let result = d.import_groups(
            &imports,
            &mut catalog,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        )?;
        assert_eq!(
            result.len(),
            5,
            "Should import metric, event, entity, span and attribute_group"
        );

        // The imported public attribute group must preserve the per-attribute
        // requirement level authored on its ref (rather than resetting it to
        // the default).
        let group = result
            .iter()
            .find(|g| g.group.id == "attribute_group.e")
            .expect("attribute_group.e should be imported")
            .group
            .clone();
        assert_eq!(group.attributes.len(), 1);
        let attr = catalog
            .attribute(&group.attributes[0])
            .expect("imported attribute should exist in the catalog");
        assert_eq!(
            attr.requirement_level,
            weaver_semconv::v1::attribute::RequirementLevel::Basic(
                weaver_semconv::v1::attribute::BasicRequirementLevelSpec::Required,
            )
        );

        Ok(())
    }

    /// Every v2 signal type carries fields that only its own kind has. They
    /// are set on top of the shared group body, so this pins each one down.
    #[test]
    fn test_import_groups_v2_signal_specific_fields() -> Result<(), Box<dyn Error>> {
        let d = example_v2_schema();
        let mut catalog = AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let star = |pattern| {
            Some(vec![weaver_semconv::v1::group::GroupWildcard(
                globset::Glob::new(pattern).expect("valid glob"),
            )])
        };
        let imports = vec![weaver_semconv::v1::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::v1::semconv::Imports {
                metrics: star("*"),
                events: star("*"),
                entities: star("*"),
                spans: star("*"),
                attribute_groups: star("*"),
            },
        }];

        let result = d.import_groups(
            &imports,
            &mut catalog,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        )?;
        let group = |id: &str| {
            result
                .iter()
                .find(|g| g.group.id == id)
                .unwrap_or_else(|| panic!("{id} should be imported"))
                .group
                .clone()
        };

        // A metric keeps its name, instrument, unit and entity associations.
        let metric = group("metric.a");
        assert_eq!(metric.metric_name.as_deref(), Some("metric.a"));
        assert_eq!(
            metric.instrument,
            Some(weaver_semconv::v1::group::InstrumentSpec::Counter)
        );
        assert_eq!(metric.unit.as_deref(), Some("1"));
        assert_eq!(
            metric.entity_associations,
            vec![
                weaver_semconv::v1::entity_association::EntityAssociation::Ref(
                    "entity.c".to_owned()
                )
            ]
        );
        assert!(metric.name.is_none(), "A metric has no signal name");
        assert!(metric.lineage.is_some(), "A metric carries its lineage");

        // An event keeps its name.
        assert_eq!(group("event.b").name.as_deref(), Some("event.b"));

        // A span keeps its kind and its name specification.
        let span = group("span.d");
        assert_eq!(span.name.as_deref(), Some("span.d"));
        assert_eq!(
            span.span_kind,
            Some(weaver_semconv::v1::group::SpanKindSpec::Client)
        );
        assert_eq!(
            span.span_name,
            Some(weaver_semconv::v1::group::SpanName {
                templates: Vec::new(),
                note: Some("test".to_owned()),
            })
        );

        // An entity keeps its type as the group name, and tags its identity
        // attributes as identifying and its description attributes as
        // descriptive, in that order.
        let entity = group("entity.c");
        assert_eq!(entity.name.as_deref(), Some("entity.c"));
        let roles: Vec<_> = entity
            .attributes
            .iter()
            .map(|ar| {
                let attr = catalog
                    .attribute(ar)
                    .expect("imported attribute should exist in the catalog");
                (attr.name.clone(), attr.role.clone())
            })
            .collect();
        assert_eq!(
            roles,
            vec![
                (
                    "entity.c.id".to_owned(),
                    Some(weaver_semconv::v1::attribute::AttributeRole::Identifying)
                ),
                (
                    "entity.c.label".to_owned(),
                    Some(weaver_semconv::v1::attribute::AttributeRole::Descriptive)
                ),
            ]
        );

        // Every imported group, an attribute group included, records each of
        // its attributes as a reference to the registry that defines it, so the
        // importing registry does not take them for its own definitions.
        for id in [
            "metric.a",
            "event.b",
            "entity.c",
            "span.d",
            "attribute_group.e",
        ] {
            let imported = group(id);
            let lineage = imported
                .lineage
                .as_ref()
                .unwrap_or_else(|| panic!("{id} should carry its lineage"));
            for ar in &imported.attributes {
                let attr = catalog
                    .attribute(ar)
                    .expect("imported attribute should exist in the catalog");
                let attr_lineage = lineage
                    .attribute(&attr.name)
                    .unwrap_or_else(|| panic!("{id} should record `{}` as a reference", attr.name));
                assert!(
                    attr_lineage.source_group.starts_with("v2_dependency."),
                    "`{}` in {id} should come from a dependency, got `{}`",
                    attr.name,
                    attr_lineage.source_group
                );
            }
        }

        // A published attribute group is public, so the import keeps it public;
        // visibility means nothing on a signal.
        assert_eq!(
            group("attribute_group.e").visibility,
            Some(weaver_semconv::v1::group::AttributeGroupVisibilitySpec::Public)
        );
        assert!(group("metric.a").visibility.is_none());

        Ok(())
    }

    #[test]
    fn test_import_groups_vec() -> Result<(), Box<dyn Error>> {
        let deps = vec![
            ResolvedDependency::from(example_v1_schema()),
            ResolvedDependency::from(example_v2_schema()),
        ];
        let mut catalog = AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::v1::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::v1::semconv::Imports {
                metrics: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("metric.a")?,
                )]),
                events: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("event.b")?,
                )]),
                entities: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("entity.c")?,
                )]),
                spans: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("span.d")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e")?,
                )]),
            },
        }];

        let result = deps.import_groups(
            &imports,
            &mut catalog,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        )?;
        // V1 schema has AttributeGroup, which returns false unless include_all.
        // V2 schema has metric, event, entity, span, and attribute_group that match.
        assert_eq!(result.len(), 5);

        Ok(())
    }

    #[test]
    fn test_import_groups_combine_blocks() -> Result<(), Box<dyn Error>> {
        let d = example_v2_schema();
        let mut catalog = AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");

        let imports = vec![
            weaver_semconv::v1::group::ImportsWithProvenance {
                provenance: weaver_semconv::provenance::Provenance::new(
                    schema_url.clone(),
                    "file1",
                ),
                imports: weaver_semconv::v1::semconv::Imports {
                    metrics: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                        globset::Glob::new("metric.a")?,
                    )]),
                    events: None,
                    entities: None,
                    spans: None,
                    attribute_groups: None,
                },
            },
            weaver_semconv::v1::group::ImportsWithProvenance {
                provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file2"),
                imports: weaver_semconv::v1::semconv::Imports {
                    metrics: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                        globset::Glob::new("metric.b")?,
                    )]),
                    events: Some(vec![weaver_semconv::v1::group::GroupWildcard(
                        globset::Glob::new("event.b")?,
                    )]),
                    entities: None,
                    spans: None,
                    attribute_groups: None,
                },
            },
        ];

        let result = d.import_groups(
            &imports,
            &mut catalog,
            &crate::graph::chosen_versions::NullSchemaCacheLookup,
        )?;
        assert_eq!(
            result.len(),
            2,
            "Should successfully combine import blocks and import both metric.a and event.b"
        );

        Ok(())
    }

    #[test]
    fn test_upgrade_imported_group_v2() -> Result<(), Box<dyn Error>> {
        use super::upgrade_imported_group;
        use crate::graph::chosen_versions::SchemaCacheLookup;
        use crate::v1::attribute::AttributeCatalog;
        use std::sync::Arc;
        use weaver_resolved_schema::v1::lineage::GroupLineage;
        use weaver_resolved_schema::v1::registry::Group;
        use weaver_semconv::provenance::Provenance;
        use weaver_semconv::schema_url::SchemaUrl;

        struct MockLookup {
            schema: Arc<crate::WeaverResolvedSchema>,
            url: SchemaUrl,
        }
        impl SchemaCacheLookup for MockLookup {
            fn lookup_schema(&self, _url: &SchemaUrl) -> Option<Arc<crate::WeaverResolvedSchema>> {
                Some(self.schema.clone())
            }
            fn chosen_version(&self, _registry_name: &str) -> Option<&SchemaUrl> {
                Some(&self.url)
            }
        }

        let mut chosen_schema = example_v2_schema();
        let old_url = SchemaUrl::try_from_name_version("test", "1.0.0")?;
        let chosen_url = SchemaUrl::try_from_name_version("test", "1.1.0")?;
        chosen_schema.schema_url = chosen_url.clone();
        let arc_v2 = Arc::new(crate::WeaverResolvedSchema::V2(chosen_schema));

        let group = Group {
            span_links: Vec::new(),
            id: "metric.a".to_owned(),
            r#type: weaver_semconv::v1::group::GroupType::Metric,
            brief: "Old brief".to_owned(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![],
            lineage: Some(GroupLineage::new(Provenance::new(old_url.clone(), "test"))),
            metric_name: Some("metric.a".to_owned()),
            instrument: Some(weaver_semconv::v1::group::InstrumentSpec::Counter),
            unit: Some("1".to_owned()),
            span_kind: None,
            span_name: None,
            events: vec![],
            annotations: None,
            display_name: None,
            body: None,
            entity_associations: vec![],
            requirement_level: None,
            name: None,
            visibility: None,
            is_v2: true,
        };

        let mut catalog = AttributeCatalog::default();
        let lookup = MockLookup {
            schema: arc_v2,
            url: chosen_url.clone(),
        };

        let upgraded = upgrade_imported_group(&group, &old_url, &mut catalog, &lookup)?
            .expect("group should be upgraded");
        assert_eq!(upgraded.id, "metric.a");
        assert_eq!(upgraded.metric_name.as_deref(), Some("metric.a"));
        assert_eq!(upgraded.attributes.len(), 0);

        let group_ag = Group {
            span_links: Vec::new(),
            id: "attribute_group.e".to_owned(),
            r#type: weaver_semconv::v1::group::GroupType::AttributeGroup,
            brief: "Old brief".to_owned(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![],
            lineage: Some(GroupLineage::new(Provenance::new(old_url.clone(), "test"))),
            metric_name: None,
            instrument: None,
            unit: None,
            span_kind: None,
            span_name: None,
            events: vec![],
            annotations: None,
            display_name: None,
            body: None,
            entity_associations: vec![],
            requirement_level: None,
            name: None,
            visibility: None,
            is_v2: true,
        };

        let upgraded_ag = upgrade_imported_group(&group_ag, &old_url, &mut catalog, &lookup)?
            .expect("attribute_group should be upgraded");
        assert_eq!(upgraded_ag.id, "attribute_group.e");
        assert_eq!(upgraded_ag.attributes.len(), 1);
        assert_eq!(
            upgraded_ag.visibility,
            Some(weaver_semconv::v1::group::AttributeGroupVisibilitySpec::Public)
        );

        // Every other signal type is rebuilt from the chosen version too, and
        // keeps its attributes recorded as references to the registry that
        // defines them, as a fresh import does.
        let mut upgraded_groups = vec![upgraded, upgraded_ag];
        for (id, r#type) in [
            ("event.b", weaver_semconv::v1::group::GroupType::Event),
            ("entity.c", weaver_semconv::v1::group::GroupType::Entity),
            ("span.d", weaver_semconv::v1::group::GroupType::Span),
        ] {
            let old = Group {
                id: id.to_owned(),
                r#type,
                name: Some(id.to_owned()),
                metric_name: None,
                instrument: None,
                unit: None,
                ..group.clone()
            };
            let upgraded = upgrade_imported_group(&old, &old_url, &mut catalog, &lookup)?
                .unwrap_or_else(|| panic!("{id} should be upgraded"));
            assert_eq!(upgraded.id, id);
            upgraded_groups.push(upgraded);
        }
        for upgraded in &upgraded_groups {
            let lineage = upgraded
                .lineage
                .as_ref()
                .unwrap_or_else(|| panic!("{} should carry its lineage", upgraded.id));
            for ar in &upgraded.attributes {
                let attr = catalog
                    .attribute(ar)
                    .expect("upgraded attribute should exist in the catalog");
                assert!(
                    lineage
                        .attribute(&attr.name)
                        .is_some_and(|l| l.source_group.starts_with("v2_dependency.")),
                    "{} should record `{}` as a reference to its dependency",
                    upgraded.id,
                    attr.name
                );
            }
        }

        // The imported copy carries no links; the upgraded definition must
        // get them from the chosen registry, not keep the empty default.
        let group_span = Group {
            span_links: Vec::new(),
            id: "span.d".to_owned(),
            r#type: weaver_semconv::v1::group::GroupType::Span,
            brief: "Old brief".to_owned(),
            note: None,
            prefix: String::new(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![],
            lineage: Some(GroupLineage::new(Provenance::new(old_url.clone(), "test"))),
            metric_name: None,
            instrument: None,
            unit: None,
            span_kind: None,
            span_name: None,
            events: vec![],
            annotations: None,
            display_name: None,
            body: None,
            entity_associations: vec![],
            requirement_level: None,
            name: Some("span.d".to_owned()),
            visibility: None,
            is_v2: true,
        };

        let upgraded_span = upgrade_imported_group(&group_span, &old_url, &mut catalog, &lookup)?
            .expect("span should be upgraded");
        assert_eq!(upgraded_span.id, "span.d");
        assert_eq!(upgraded_span.span_links.len(), 1);
        let link = &upgraded_span.span_links[0];
        assert_eq!(&*link.r#ref, "span.d");
        assert_eq!(link.attributes.len(), 1);
        assert_eq!(link.attributes[0].r#ref, "attr.in.group");
        // A non-default attribute level must survive the resolved-to-carrier hop.
        assert_eq!(
            link.attributes[0].requirement_level,
            Some(weaver_semconv::v1::attribute::RequirementLevel::Basic(
                weaver_semconv::v1::attribute::BasicRequirementLevelSpec::Required,
            ))
        );
        // Overridden metadata must survive the import as explicit overrides.
        assert_eq!(
            link.attributes[0].brief.as_deref(),
            Some("the id, as carried on the link")
        );

        Ok(())
    }

    #[test]
    fn test_to_named_associations_keeps_the_shape() {
        use super::{to_named_associations, V2EntityAssociation};
        use weaver_resolved_schema::v2::entity::EntityRef;
        use weaver_semconv::v1::entity_association::EntityAssociation as SpecAssociation;

        let association_tree = vec![V2EntityAssociation::AllOf {
            all_of: vec![
                V2EntityAssociation::Ref(EntityRef {
                    r#type: "service".into(),
                    provenance: Default::default(),
                }),
                V2EntityAssociation::OneOf {
                    one_of: vec![V2EntityAssociation::Ref(EntityRef {
                        r#type: "host".into(),
                        provenance: weaver_resolved_schema::v2::provenance::Provenance {
                            source: Some(weaver_resolved_schema::v2::provenance::DependencyRef(2)),
                            path: Default::default(),
                        },
                    })],
                },
            ],
        }];

        assert_eq!(
            to_named_associations(&association_tree),
            vec![SpecAssociation::AllOf {
                all_of: vec![
                    SpecAssociation::Ref("service".to_owned()),
                    SpecAssociation::OneOf {
                        one_of: vec![SpecAssociation::Ref("host".to_owned())],
                    },
                ],
            }]
        );
    }
}
