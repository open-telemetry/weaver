// SPDX-License-Identifier: Apache-2.0

//! Resolving an `imports` block against dependencies.
//!
//! A registry names the signals it wants from its dependencies in `imports`
//! blocks. This module turns those patterns into the groups the registry
//! receives: it compiles the patterns into matchers, decides which candidate
//! definitions are visible, converts v2 signals into v1 groups, substitutes
//! upgraded definitions where version conflict resolution picked a newer
//! registry, and drops the duplicates that a diamond in the dependency graph
//! produces.
//!
//! Looking a definition up in a dependency, as opposed to importing it, lives
//! in [`crate::dependency`].

use globset::GlobSet;
use std::collections::{BTreeMap, HashMap};
use weaver_resolved_schema::attribute::{Attribute, AttributeRef};
use weaver_resolved_schema::lineage::GroupLineage;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::attribute::AttributeRef as V2AttributeRef;
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2Catalog;
use weaver_resolved_schema::v2::entity::{
    to_named_associations, EntityAssociation as V2EntityAssociation,
};
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::v2::Signal;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::attribute::{AttributeRole, RequirementLevel};
use weaver_semconv::group::{GroupType, GroupWildcard, ImportsWithProvenance};
use weaver_semconv::schema_url::SchemaUrl;

use crate::{
    attribute::{AttributeCatalog, AttributeSource},
    conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion},
    dependency::{find_attribute_source, v2_source_url, EntityLookup, ResolvedDependency},
    dependency_resolution::is_excluded,
    Error,
};

/// The field of an `imports` block that lists a signal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ImportField {
    Metrics,
    Events,
    Spans,
    Entities,
    AttributeGroups,
}

impl ImportField {
    /// The field that imports a group of this type. Returns `None` when no
    /// field imports a group of this type.
    fn of(group_type: &GroupType) -> Option<Self> {
        match group_type {
            GroupType::Metric | GroupType::MetricGroup => Some(Self::Metrics),
            GroupType::Event => Some(Self::Events),
            GroupType::Span => Some(Self::Spans),
            GroupType::Entity => Some(Self::Entities),
            GroupType::AttributeGroup => Some(Self::AttributeGroups),
            GroupType::Scope | GroupType::Undefined => None,
        }
    }

    /// Every field, in a stable order.
    fn all() -> [Self; 5] {
        [
            Self::Metrics,
            Self::Events,
            Self::Spans,
            Self::Entities,
            Self::AttributeGroups,
        ]
    }

    /// The name of the field, for a diagnostic.
    fn name(&self) -> &'static str {
        match self {
            Self::Metrics => "metrics",
            Self::Events => "events",
            Self::Spans => "spans",
            Self::Entities => "entities",
            Self::AttributeGroups => "attribute_groups",
        }
    }

    /// The patterns of this field, in one `imports` block.
    fn patterns<'a>(&self, imports: &'a weaver_semconv::semconv::Imports) -> &'a [GroupWildcard] {
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

/// Whether the resolver added this `imports` block itself, rather than the
/// author writing it. `--include-unreferenced` asks for everything, so it is
/// not a name that can be misspelled.
fn is_implicit_import(import: &ImportsWithProvenance) -> bool {
    import.provenance.path == "--include-unreferenced"
}

/// Reports every explicit import pattern that named none of the imported
/// groups.
///
/// An unmatched pattern is nearly always a typo or a stale name, and the
/// resolver drops it in silence. This runs across all dependencies at once,
/// because a pattern can name nothing in one dependency and still be satisfied
/// by another.
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
            .flat_map(|i| field.patterns(&i.imports))
            .collect();
        if patterns.is_empty() {
            continue;
        }
        // Built from the same list, in the same order, so a match index is an
        // index into `patterns`.
        let matcher = build_globset(patterns.iter().copied())?;
        let mut matched = vec![false; patterns.len()];
        for group in groups {
            if ImportField::of(&group.group.r#type) != Some(field) {
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

/// The strings that an import pattern can match a group by.
///
/// A pattern can name a v2 group by its id, by its signal name, or by its id
/// without the group-type prefix. A pattern can name a v1 group by its signal
/// name. For a v1 span or attribute group, the pattern uses the id. For a v1
/// entity, the pattern uses the id or the name.
///
/// Two keys exist only to keep older registries resolving: the `registry.`
/// prefix on an attribute group id, an older spelling that a v2 group can
/// still carry, and the id of a v1 `resource` entity.
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
        // A pattern can also name an attribute group by its id without the
        // `registry.` prefix, which is the older spelling.
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
            // A legacy `resource` group has no name. It holds its entity
            // type in the id. A key list from the name alone is empty, so no
            // import can find such a group.
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

/// One compiled matcher per `imports` field.
struct ImportMatchers {
    metrics: GlobSet,
    events: GlobSet,
    spans: GlobSet,
    entities: GlobSet,
    attribute_groups: GlobSet,
}

impl ImportMatchers {
    fn build<'a>(
        imports: impl Iterator<Item = &'a ImportsWithProvenance> + Clone,
    ) -> Result<Self, Error> {
        let for_field = |field: ImportField| {
            build_globset(
                imports
                    .clone()
                    .flat_map(move |i| field.patterns(&i.imports)),
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

    fn field(&self, field: ImportField) -> &GlobSet {
        match field {
            ImportField::Metrics => &self.metrics,
            ImportField::Events => &self.events,
            ImportField::Spans => &self.spans,
            ImportField::Entities => &self.entities,
            ImportField::AttributeGroups => &self.attribute_groups,
        }
    }

    /// True when a pattern names this group.
    fn matches(&self, g: &Group) -> bool {
        let Some(field) = ImportField::of(&g.r#type) else {
            return false;
        };
        let matcher = self.field(field);
        import_match_keys(g)
            .into_iter()
            .any(|k| matcher.is_match(k))
    }

    /// True when a pattern in `field` names this key. A published v2 signal is
    /// named by its id alone, so it needs no key list.
    fn matches_key(&self, field: ImportField, key: &str) -> bool {
        self.field(field).is_match(key)
    }
}

/// A group with its source provenance.
pub struct GroupWithProvenance {
    /// The group definition.
    pub group: Group,
    /// The schema URL of the registry it came from.
    pub schema_url: SchemaUrl,
    /// The registry that defines each entity this group is associated with, by
    /// the name the association uses.
    ///
    /// The association resolved in the registry that declared the group, and
    /// that answer travels with it. The importing registry may define an
    /// unrelated entity of the same name, and reading the name again there
    /// would point the signal at it.
    pub association_origins: BTreeMap<String, SchemaUrl>,
}

/// Allows importing dependencies
pub(crate) trait ImportableDependency {
    /// Imports groups from the given dependency using the flags provided.
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error>;
}

/// Where each entity a v1 group is associated with is defined.
///
/// The dependency resolved its own associations, and the record survives in
/// memory, so read it first: it is the only place that holds the answer for an
/// entity the dependency named but does not itself hold. A published
/// `resolved/1.0` file carries no such record, and then the group's own entities
/// are all there is to go on.
///
/// A name the dependency cannot answer is left out, so the importing registry
/// resolves it as it would any association of its own. So is a private entity:
/// the dependency keeps it out of reach, and importing a signal is no way in.
fn v1_association_origins(schema: &V1Schema, group: &Group) -> BTreeMap<String, SchemaUrl> {
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
    fn import_groups<C: crate::SchemaCacheLookup>(
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
            let g = if let Some(upgraded) =
                upgrade_imported_group(g, &my_schema_url, attribute_catalog, cache_lookup)?
            {
                upgraded
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
                g
            };
            let mut g_url = my_schema_url.clone();
            if let Some(chosen_url) = cache_lookup.chosen_version(g_url.name()) {
                if chosen_url != &g_url {
                    if let Ok(winning_url) =
                        UseLatestMajorVersion.resolve_conflict(&g_url, chosen_url)
                    {
                        g_url = winning_url;
                    }
                }
            }
            let association_origins = v1_association_origins(self, &g);
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

/// If an imported group's origin registry (recorded in its lineage
/// provenance, which may point at a transitive dependency rather than the
/// immediate one) was upgraded to a newer compatible version by graph-wide
/// version conflict resolution, returns the group's definition from the
/// chosen registry so its body, lineage, and attributes stay consistent
/// with the upgraded attribute catalog — mirroring what
/// `upgrade_attribute_with_source` does for attributes.
///
/// Returns `None` when no upgrade applies, or when the chosen registry does
/// not define the group; the caller then keeps the copy it has.
fn upgrade_imported_group<C: crate::SchemaCacheLookup>(
    group: &Group,
    fallback_url: &SchemaUrl,
    attribute_catalog: &mut AttributeCatalog,
    cache_lookup: &C,
) -> Result<Option<Group>, Error> {
    let origin_url = origin_url(group, fallback_url);
    let Some(chosen_url) = cache_lookup.chosen_version(origin_url.name()) else {
        return Ok(None);
    };
    if *chosen_url == origin_url {
        return Ok(None);
    }
    let Ok(winning_url) = UseLatestMajorVersion.resolve_conflict(&origin_url, chosen_url) else {
        return Ok(None);
    };
    if winning_url != *chosen_url {
        return Ok(None);
    }
    let Some(chosen_schema) = cache_lookup.lookup_schema(chosen_url) else {
        return Ok(None);
    };
    // TODO: also look up the group when the chosen registry is a published
    // resolved V2 schema.
    let Some(chosen_v1) = chosen_schema.as_v1() else {
        return Ok(None);
    };
    let Some(upgraded) = chosen_v1
        .registry
        .groups
        .iter()
        .find(|candidate| is_same_imported_group(group, candidate))
    else {
        return Ok(None);
    };
    let mut upgraded = upgraded.clone();
    let mut attributes = vec![];
    for a in upgraded
        .attributes
        .iter()
        .filter_map(|ar| chosen_v1.catalog().attribute(ar))
    {
        let source = find_attribute_source(chosen_v1, &a.name, chosen_url);
        attributes.push(attribute_catalog.attribute_ref_with_provenance(
            a.clone(),
            source,
            cache_lookup,
        )?);
    }
    upgraded.attributes = attributes;
    Ok(Some(upgraded))
}

/// Whether `candidate` (a group in a chosen upgraded registry) defines the
/// same signal as `group` (an imported group). Group ids may differ in their
/// `<type>.` prefix between the definition (V1) and published (V2) import
/// paths, so signals are matched by type and name where available.
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

/// Strips a group-type id prefix, if present.
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

/// Outcome of an import decision for a candidate dep item.
enum ImportDecision {
    /// Item is visible — proceed with the normal import path.
    Include,
    /// Item is excluded and only matched via `include_all`. Silently dropped:
    /// excluded items are invisible to dependents and shouldn't surface as
    /// errors when the consumer never explicitly asked for them.
    Skip,
    /// Item is excluded and was matched by an explicit `imports:` pattern.
    /// Surfaces as a hard error because the consumer asked for it by name.
    Error(Error),
}

fn import_decision(
    annotations: &BTreeMap<String, weaver_semconv::YamlValue>,
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
        })
    } else {
        ImportDecision::Skip
    }
}

/// A v2 signal's reference to a catalog attribute, with the per-signal state
/// the reference carries.
///
/// The v2 signal types each have their own attribute ref struct, holding
/// whichever of these fields apply to that signal: only a span ref marks an
/// attribute as sampling relevant, and only an entity ref carries a role.
struct V2SignalAttribute<'a> {
    base: &'a V2AttributeRef,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
}

impl<'a> V2SignalAttribute<'a> {
    fn new(base: &'a V2AttributeRef, requirement_level: RequirementLevel) -> Self {
        V2SignalAttribute {
            base,
            requirement_level,
            sampling_relevant: None,
            role: None,
        }
    }

    /// Marks the attribute as (ir)relevant for sampling, as a span ref does.
    fn with_sampling_relevant(mut self, sampling_relevant: Option<bool>) -> Self {
        self.sampling_relevant = sampling_relevant;
        self
    }

    /// Tags the attribute with the role it plays on an entity.
    fn with_role(mut self, role: AttributeRole) -> Self {
        self.role = Some(role);
        self
    }
}

/// Converts a V2 attribute (with no requirement level) to a v1 attribute.
fn convert_v2_attribute(
    attr: &weaver_resolved_schema::v2::attribute::Attribute,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
) -> Attribute {
    Attribute {
        name: attr.key.clone(),
        r#type: attr.r#type.clone(),
        brief: attr.common.brief.clone(),
        examples: attr.examples.clone(),
        tag: None,
        requirement_level,
        sampling_relevant,
        note: attr.common.note.clone(),
        stability: Some(attr.common.stability.clone()),
        deprecated: attr.common.deprecated.clone(),
        prefix: false,
        tags: None,
        annotations: Some(attr.common.annotations.clone()),
        value: None,
        role,
    }
}

/// Maps a v2 signal provenance onto the v1 provenance an imported group carries.
fn v2_provenance(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    provenance: &weaver_resolved_schema::v2::provenance::Provenance,
) -> weaver_semconv::provenance::Provenance {
    weaver_semconv::provenance::Provenance::new(
        v2_source_url(schema, deps, provenance.source),
        &provenance.path,
    )
}

/// Resolves a v2 signal's attribute refs into the importing registry's
/// catalog, converting each one to its v1 form.
fn import_v2_attributes<'a, C: crate::SchemaCacheLookup>(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    refs: impl Iterator<Item = V2SignalAttribute<'a>>,
    attribute_catalog: &mut AttributeCatalog,
    cache_lookup: &C,
) -> Result<Vec<AttributeRef>, Error> {
    let mut attributes = vec![];
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
        let source = AttributeSource::Dependency {
            schema_url: v2_source_url(schema, deps, attr.provenance.source),
        };
        attributes.push(attribute_catalog.attribute_ref_with_provenance(
            convert_v2_attribute(
                attr,
                signal_attr.requirement_level,
                signal_attr.sampling_relevant,
                signal_attr.role,
            ),
            source,
            cache_lookup,
        )?);
    }
    Ok(attributes)
}

/// Builds the v1 group that carries an imported v2 signal, with the fields
/// every signal shares; callers set the signal-specific fields (metric name,
/// span kind, ...).
fn imported_v2_group(
    id: String,
    r#type: GroupType,
    common: &weaver_semconv::v2::CommonFields,
    attributes: Vec<AttributeRef>,
    lineage: Option<GroupLineage>,
) -> Group {
    Group {
        id,
        r#type,
        brief: common.brief.clone(),
        note: common.note.clone(),
        prefix: "".to_owned(),
        extends: None,
        stability: Some(common.stability.clone()),
        deprecated: common.deprecated.clone(),
        attributes,
        span_kind: None,
        events: vec![],
        metric_name: None,
        instrument: None,
        unit: None,
        requirement_level: None,
        name: None,
        lineage,
        display_name: None,
        body: None,
        annotations: Some(common.annotations.clone()),
        entity_associations: vec![],
        visibility: None,
        is_v2: true,
        span_name: None,
        span_links: Vec::new(),
    }
}

/// Where each entity a v2 signal is associated with is defined.
///
/// A resolved leaf that names a dependency already names the registry that
/// declared the entity, because that is what the lookup recorded. A leaf with no
/// provenance names an entity of this schema, which may itself have been
/// imported, so read the origin off the entity.
fn v2_association_origins(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    associations: &[V2EntityAssociation],
) -> BTreeMap<String, SchemaUrl> {
    associations
        .iter()
        .flat_map(|assoc| assoc.refs())
        .filter_map(|entity_ref| {
            let origin = match entity_ref.provenance.source {
                Some(dep_ref) => deps.get(dep_ref.0 as usize).cloned(),
                None => schema
                    .lookup_entity(&entity_ref.r#type)
                    .filter(|location| !location.excluded)
                    .map(|location| location.origin),
            };
            origin.map(|origin| (entity_ref.r#type.to_string(), origin))
        })
        .collect()
}

impl ImportableDependency for V2Schema {
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        let mut result = vec![];
        let mut exclusion_errors: Vec<Error> = vec![];
        // Where the associations of each imported signal resolved, by group id.
        // Collected here because a group holds only the names, and the leaves
        // that hold the answer are read while the signal is converted.
        let mut origins: HashMap<String, BTreeMap<String, SchemaUrl>> = HashMap::new();
        // The table a `DependencyRef` indexes into, materialised once.
        let deps: Vec<SchemaUrl> = self.dependencies.iter().cloned().collect();

        let explicit_imports: Vec<&ImportsWithProvenance> =
            imports.iter().filter(|i| !is_implicit_import(i)).collect();
        let explicit = ImportMatchers::build(explicit_imports.iter().copied())?;
        let any = ImportMatchers::build(imports.iter())?;

        // A published v2 signal is named by its id: the metric or event name,
        // the entity or span type, or the attribute group id. Errors from a
        // signal that an explicit pattern named but that the dependency
        // excludes are collected so every one of them is reported at once.
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

        // First import metrics.  These are *by name* and come from the registry.
        // This is the closest to V1 ref syntax we have.
        for m in self.registry.metrics.iter() {
            if !is_imported(m, ImportField::Metrics, GroupType::Metric) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                m.attributes
                    .iter()
                    .map(|ar| V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                m.id().to_owned(),
                GroupType::Metric,
                &m.common,
                attributes,
                Some(GroupLineage::new(v2_provenance(self, &deps, &m.provenance))),
            );
            group.metric_name = Some(m.name.to_string());
            group.instrument = Some(m.instrument.clone());
            group.unit = Some(m.unit.clone());
            group.entity_associations = to_named_associations(&m.entity_associations);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &m.entity_associations),
            );
            result.push(group);
        }

        // Now event imports.
        for e in self.registry.events.iter() {
            if !is_imported(e, ImportField::Events, GroupType::Event) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                e.attributes
                    .iter()
                    .map(|ar| V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())),
                attribute_catalog,
                cache_lookup,
            )?;
            let mut group = imported_v2_group(
                e.id().to_owned(),
                GroupType::Event,
                &e.common,
                attributes,
                Some(GroupLineage::new(v2_provenance(self, &deps, &e.provenance))),
            );
            group.name = Some(e.name.to_string());
            group.entity_associations = to_named_associations(&e.entity_associations);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &e.entity_associations),
            );
            result.push(group);
        }

        // Now Entity imports. An entity's identity and description attributes
        // keep their roles so refinements inherit them correctly.
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
                        V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())
                            .with_role(AttributeRole::Identifying)
                    })
                    .chain(e.description.iter().map(|ar| {
                        V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())
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
                Some(GroupLineage::new(v2_provenance(self, &deps, &e.provenance))),
            );
            group.name = Some(e.r#type.to_string());
            result.push(group);
        }

        // Now Span imports.
        for s in self.registry.spans.iter() {
            if !is_imported(s, ImportField::Spans, GroupType::Span) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                s.attributes.iter().map(|ar| {
                    V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())
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
                Some(GroupLineage::new(v2_provenance(self, &deps, &s.provenance))),
            );
            group.span_kind = Some(s.kind.clone());
            group.span_name = Some(s.name.clone());
            // Forward the span's resolved links in definition shape: catalog
            // indices become attribute names via this dependency's catalog.
            let mut links = Vec::new();
            for link in s.links.iter() {
                let mut link_attributes = Vec::new();
                for la in link.attributes.iter() {
                    let attr = self.attribute_catalog.attribute(&la.base).ok_or(
                        Error::InvalidRegistryAttributeRef {
                            registry_name: self.schema_url.name().to_owned(),
                            attribute_ref: la.base.0,
                        },
                    )?;
                    link_attributes.push(
                        weaver_semconv::v2::span::SpanAttributeOrGroupRef::Attribute(
                            weaver_semconv::v2::span::SpanAttributeRef {
                                base: weaver_semconv::v2::attribute::AttributeRef {
                                    r#ref: attr.key.clone(),
                                    brief: None,
                                    examples: None,
                                    requirement_level: Some(la.requirement_level.clone()),
                                    note: None,
                                    stability: None,
                                    deprecated: None,
                                    annotations: Default::default(),
                                },
                                sampling_relevant: la.sampling_relevant,
                            },
                        ),
                    );
                }
                links.push(weaver_semconv::v2::span::SpanLink {
                    r#ref: link.r#ref.clone(),
                    requirement_level: Some(link.requirement_level.clone()),
                    brief: link.brief.clone(),
                    note: link.note.clone(),
                    attributes: link_attributes,
                });
            }
            group.span_links = links;
            group.name = Some(s.r#type.to_string());
            group.entity_associations = to_named_associations(&s.entity_associations);
            _ = origins.insert(
                group.id.clone(),
                v2_association_origins(self, &deps, &s.entity_associations),
            );
            result.push(group);
        }

        // Now AttributeGroup imports. An attribute group carries no lineage:
        // it defines no signal for a refinement to extend.
        for ag in self.registry.attribute_groups.iter() {
            if !is_imported(ag, ImportField::AttributeGroups, GroupType::AttributeGroup) {
                continue;
            }
            let attributes = import_v2_attributes(
                self,
                &deps,
                ag.attributes
                    .iter()
                    .map(|ar| V2SignalAttribute::new(&ar.base, ar.requirement_level.clone())),
                attribute_catalog,
                cache_lookup,
            )?;
            result.push(imported_v2_group(
                ag.id().to_owned(),
                GroupType::AttributeGroup,
                &ag.common,
                attributes,
                None,
            ));
        }

        if !exclusion_errors.is_empty() {
            return Err(Error::CompoundError(exclusion_errors));
        }

        // The imported groups may originate in transitive dependencies that
        // graph-wide version conflict resolution upgraded; substitute the
        // chosen version's definition where that applies.
        for group in result.iter_mut() {
            if let Some(upgraded) =
                upgrade_imported_group(group, &self.schema_url, attribute_catalog, cache_lookup)?
            {
                *group = upgraded;
            }
        }

        let mut g_url = self.schema_url.clone();
        if let Some(chosen_url) = cache_lookup.chosen_version(g_url.name()) {
            if chosen_url != &g_url {
                if let Ok(winning_url) = UseLatestMajorVersion.resolve_conflict(&g_url, chosen_url)
                {
                    g_url = winning_url;
                }
            }
        }
        Ok(result
            .into_iter()
            .map(|group| GroupWithProvenance {
                association_origins: origins.remove(&group.id).unwrap_or_default(),
                group,
                schema_url: g_url.clone(),
            })
            .collect())
    }
}

impl ImportableDependency for ResolvedDependency {
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        match self {
            ResolvedDependency::V1(schema) => {
                schema.import_groups(imports, attribute_catalog, cache_lookup)
            }
            ResolvedDependency::V2(schema) => {
                schema.import_groups(imports, attribute_catalog, cache_lookup)
            }
        }
    }
}

// Allows importing across all dependencies.
impl ImportableDependency for Vec<ResolvedDependency> {
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        // A diamond in the dependency graph reaches one definition by two or
        // more paths. This loop keeps the first copy of each definition, so the
        // registry holds no duplicates for `check_uniqueness` to report.
        //
        // `AttributeCatalog` keys its map on the definition itself. `Group` has
        // no `Hash`, and `is_same_imported_group` compares two groups. So the
        // map divides the candidates into buckets, one for each origin and
        // type. The comparison then runs in one bucket only.
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

/// The registry that declared a group.
///
/// An imported group carries the provenance of its origin. A re-exported
/// definition therefore keeps the url of the registry that declared it. A group
/// with no provenance comes from the dependency in `fallback_url`.
fn origin_url(group: &Group, fallback_url: &SchemaUrl) -> SchemaUrl {
    group
        .provenance()
        .map(|prov| prov.schema_url)
        .unwrap_or_else(|| fallback_url.clone())
}
// Constructs a globset from a set of wildcards.
fn build_globset<'a>(wildcards: impl Iterator<Item = &'a GroupWildcard>) -> Result<GlobSet, Error> {
    let mut builder = GlobSet::builder();
    for wildcard in wildcards {
        _ = builder.add(wildcard.0.clone());
    }
    builder.build().map_err(|e| Error::InvalidWildcard {
        error: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::dependency::tests::{example_v1_schema, example_v2_schema};
    use crate::dependency::ResolvedDependency;
    use crate::imports::ImportableDependency;

    #[test]
    fn test_import_groups_v1() -> Result<(), Box<dyn Error>> {
        let d = example_v1_schema();
        let mut catalog = crate::attribute::AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: None,
                events: None,
                entities: None,
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.v1")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("a")?,
                )]),
            },
        }];

        // By default V1 example schema has an AttributeGroup and a Span.
        let result = d.import_groups(&imports, &mut catalog, &())?;
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
        let mut catalog = crate::attribute::AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("metric.a")?,
                )]),
                events: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("event.b")?,
                )]),
                entities: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("entity.c")?,
                )]),
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.d")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e")?,
                )]),
            },
        }];

        let result = d.import_groups(&imports, &mut catalog, &())?;
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
            weaver_semconv::attribute::RequirementLevel::Basic(
                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
            )
        );

        Ok(())
    }

    /// Every v2 signal type carries fields that only its own kind has. They
    /// are set on top of the shared group body, so this pins each one down.
    #[test]
    fn test_import_groups_v2_signal_specific_fields() -> Result<(), Box<dyn Error>> {
        let d = example_v2_schema();
        let mut catalog = crate::attribute::AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let star = |pattern| {
            Some(vec![weaver_semconv::group::GroupWildcard(
                globset::Glob::new(pattern).expect("valid glob"),
            )])
        };
        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: star("*"),
                events: star("*"),
                entities: star("*"),
                spans: star("*"),
                attribute_groups: star("*"),
            },
        }];

        let result = d.import_groups(&imports, &mut catalog, &())?;
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
            Some(weaver_semconv::group::InstrumentSpec::Counter)
        );
        assert_eq!(metric.unit.as_deref(), Some("1"));
        assert_eq!(
            metric.entity_associations,
            vec![weaver_semconv::entity_association::EntityAssociation::Ref(
                "entity.c".to_owned()
            )]
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
            Some(weaver_semconv::group::SpanKindSpec::Client)
        );
        assert_eq!(
            span.span_name,
            Some(weaver_semconv::v2::span::SpanName {
                note: "test".to_owned(),
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
                    Some(weaver_semconv::attribute::AttributeRole::Identifying)
                ),
                (
                    "entity.c.label".to_owned(),
                    Some(weaver_semconv::attribute::AttributeRole::Descriptive)
                ),
            ]
        );

        // An attribute group defines no signal, so it carries no lineage.
        assert!(group("attribute_group.e").lineage.is_none());

        Ok(())
    }

    #[test]
    fn test_import_groups_vec() -> Result<(), Box<dyn Error>> {
        let deps = vec![
            ResolvedDependency::V1(Box::new(example_v1_schema())),
            ResolvedDependency::V2(Box::new(example_v2_schema())),
        ];
        let mut catalog = crate::attribute::AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");
        let imports = vec![weaver_semconv::group::ImportsWithProvenance {
            provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file"),
            imports: weaver_semconv::semconv::Imports {
                metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("metric.a")?,
                )]),
                events: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("event.b")?,
                )]),
                entities: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("entity.c")?,
                )]),
                spans: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("span.d")?,
                )]),
                attribute_groups: Some(vec![weaver_semconv::group::GroupWildcard(
                    globset::Glob::new("attribute_group.e")?,
                )]),
            },
        }];

        let result = deps.import_groups(&imports, &mut catalog, &())?;
        // V1 schema has AttributeGroup, which returns false unless include_all.
        // V2 schema has metric, event, entity, span, and attribute_group that match.
        assert_eq!(result.len(), 5);

        Ok(())
    }

    #[test]
    fn test_import_groups_combine_blocks() -> Result<(), Box<dyn Error>> {
        let d = example_v2_schema();
        let mut catalog = crate::attribute::AttributeCatalog::default();
        let schema_url =
            weaver_semconv::schema_url::SchemaUrl::try_from_name_version("main", "1.0.0")
                .expect("Failed to create schema_url");

        let imports = vec![
            weaver_semconv::group::ImportsWithProvenance {
                provenance: weaver_semconv::provenance::Provenance::new(
                    schema_url.clone(),
                    "file1",
                ),
                imports: weaver_semconv::semconv::Imports {
                    metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                        globset::Glob::new("metric.a")?,
                    )]),
                    events: None,
                    entities: None,
                    spans: None,
                    attribute_groups: None,
                },
            },
            weaver_semconv::group::ImportsWithProvenance {
                provenance: weaver_semconv::provenance::Provenance::new(schema_url, "file2"),
                imports: weaver_semconv::semconv::Imports {
                    metrics: Some(vec![weaver_semconv::group::GroupWildcard(
                        globset::Glob::new("metric.b")?,
                    )]),
                    events: Some(vec![weaver_semconv::group::GroupWildcard(
                        globset::Glob::new("event.b")?,
                    )]),
                    entities: None,
                    spans: None,
                    attribute_groups: None,
                },
            },
        ];

        let result = d.import_groups(&imports, &mut catalog, &())?;
        assert_eq!(
            result.len(),
            2,
            "Should successfully combine import blocks and import both metric.a and event.b"
        );

        Ok(())
    }
}
