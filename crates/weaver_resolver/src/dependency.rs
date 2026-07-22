// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.

use globset::GlobSet;
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::registry::Group;
use weaver_resolved_schema::v2::catalog::AttributeCatalog as V2Catalog;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::{attribute::UnresolvedAttribute, v2::Signal};
use weaver_semconv::attribute::{AttributeRole, RequirementLevel};
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::group::{GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::group::{GroupWildcard, ImportsWithProvenance};
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::stability::Stability;

use crate::{
    attribute::{AttributeCatalog, AttributeSource},
    conflict_strategy::{DependencyVersionConflictStrategy, UseLatestMajorVersion},
    dependency_resolution::is_excluded,
    Error,
};

/// Where a group lookup landed: in the local registry or in a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupSource {
    Local,
    Dependency,
}

/// A summary of a group, used during refinement and extends resolution, along with its unresolved attributes.
#[derive(Debug, Clone)]
pub(crate) struct GroupSummary {
    /// The type of the semantic convention.
    pub r#type: GroupType,
    /// The brief description of the semantic convention.
    pub brief: String,
    /// The note of the semantic convention.
    pub note: String,
    /// Specifies the stability of the semantic convention.
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated.
    pub deprecated: Option<Deprecated>,
    /// The metric name.
    pub metric_name: Option<String>,
    /// The instrument type.
    pub instrument: Option<InstrumentSpec>,
    /// The unit.
    pub unit: Option<String>,
    /// The requirement level of the signal.
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Specifies the kind of the span.
    pub span_kind: Option<SpanKindSpec>,
    /// The v2 span name specification, inherited by refinements that do not
    /// override it.
    pub span_name: Option<weaver_semconv::v2::span::SpanName>,
    /// The attributes from this group before being completely resolved to a catalog.
    pub attributes: Vec<UnresolvedAttribute>,
    /// The annotations of the group.
    pub annotations: Option<std::collections::BTreeMap<String, weaver_semconv::YamlValue>>,
    /// Where this summary was looked up from.
    pub source: GroupSource,
}

impl GroupSummary {
    /// Returns a group summary from this group.
    /// Does not include attributes because resolved Schema uses attribute refs,
    /// and this needs to fully resolve those attributes from the catalog.
    pub(crate) fn from_without_attributes(group: &Group, source: GroupSource) -> Self {
        GroupSummary {
            r#type: group.r#type.clone(),
            brief: group.brief.clone(),
            note: group.note.clone(),
            stability: group.stability.clone(),
            deprecated: group.deprecated.clone(),
            metric_name: group.metric_name.clone(),
            instrument: group.instrument.clone(),
            unit: group.unit.clone(),
            requirement_level: group.requirement_level.clone(),
            span_kind: group.span_kind.clone(),
            span_name: group.span_name.clone(),
            attributes: vec![], // Will be set during the dependency or registry loops.
            annotations: group.annotations.clone(),
            source,
        }
    }
}

/// A Resolved dependency, for which we can look up items.
#[derive(Debug)]
pub(crate) enum ResolvedDependency {
    /// A V1 Dependency
    V1(Box<V1Schema>),
    /// A V2 Dependency
    V2(Box<V2Schema>),
}

impl ResolvedDependency {
    /// Looks up a group summary on this dependency.
    pub(crate) fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_group_summary(id),
            ResolvedDependency::V2(schema) => schema.lookup_group_summary(id),
        }
    }
}

/// A group with its source provenance.
pub struct GroupWithProvenance {
    /// The group definition.
    pub group: Group,
    /// The schema URL of the registry it came from.
    pub schema_url: SchemaUrl,
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

impl ImportableDependency for V1Schema {
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        let explicit_imports: Vec<&ImportsWithProvenance> = imports
            .iter()
            .filter(|i| i.provenance.path != "--include-unreferenced")
            .collect();

        let explicit_metrics_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.metrics.as_deref().unwrap_or_default()),
        )?;
        let all_metrics_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.metrics.as_deref().unwrap_or_default()),
        )?;

        let explicit_events_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.events.as_deref().unwrap_or_default()),
        )?;
        let all_events_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.events.as_deref().unwrap_or_default()),
        )?;

        let explicit_entities_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.entities.as_deref().unwrap_or_default()),
        )?;
        let all_entities_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.entities.as_deref().unwrap_or_default()),
        )?;

        let explicit_spans_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.spans.as_deref().unwrap_or_default()),
        )?;
        let all_spans_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.spans.as_deref().unwrap_or_default()),
        )?;

        let explicit_attribute_groups_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.attribute_groups.as_deref().unwrap_or_default()),
        )?;
        let all_attribute_groups_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.attribute_groups.as_deref().unwrap_or_default()),
        )?;

        let matches_explicitly = move |g: &Group| {
            if g.is_v2 {
                match g.r#type {
                    GroupType::AttributeGroup => {
                        explicit_attribute_groups_matcher.is_match(&g.id)
                            || g.id
                                .strip_prefix("registry.")
                                .is_some_and(|s| explicit_attribute_groups_matcher.is_match(s))
                            || g.id
                                .strip_prefix("attribute_group.")
                                .is_some_and(|s| explicit_attribute_groups_matcher.is_match(s))
                    }
                    GroupType::Span => {
                        explicit_spans_matcher.is_match(&g.id)
                            || g.name
                                .as_ref()
                                .is_some_and(|name| explicit_spans_matcher.is_match(name.as_str()))
                            || g.id
                                .strip_prefix("span.")
                                .is_some_and(|s| explicit_spans_matcher.is_match(s))
                    }
                    GroupType::Event => {
                        explicit_events_matcher.is_match(&g.id)
                            || g.name
                                .as_ref()
                                .is_some_and(|name| explicit_events_matcher.is_match(name.as_str()))
                            || g.id
                                .strip_prefix("event.")
                                .is_some_and(|s| explicit_events_matcher.is_match(s))
                    }
                    GroupType::Metric | GroupType::MetricGroup => {
                        explicit_metrics_matcher.is_match(&g.id)
                            || g.metric_name.as_ref().is_some_and(|metric_name| {
                                explicit_metrics_matcher.is_match(metric_name.as_str())
                            })
                            || g.id
                                .strip_prefix("metric.")
                                .is_some_and(|s| explicit_metrics_matcher.is_match(s))
                    }
                    GroupType::Entity => {
                        explicit_entities_matcher.is_match(&g.id)
                            || g.name.as_ref().is_some_and(|name| {
                                explicit_entities_matcher.is_match(name.as_str())
                            })
                            || g.id
                                .strip_prefix("entity.")
                                .is_some_and(|s| explicit_entities_matcher.is_match(s))
                    }
                    GroupType::Scope => false,
                    GroupType::Undefined => false,
                }
            } else {
                match g.r#type {
                    GroupType::AttributeGroup => explicit_attribute_groups_matcher.is_match(&g.id),
                    GroupType::Span => explicit_spans_matcher.is_match(&g.id),
                    GroupType::Event => g
                        .name
                        .as_ref()
                        .is_some_and(|name| explicit_events_matcher.is_match(name.as_str())),
                    GroupType::Metric => g.metric_name.as_ref().is_some_and(|metric_name| {
                        explicit_metrics_matcher.is_match(metric_name.as_str())
                    }),
                    GroupType::MetricGroup => false,
                    GroupType::Entity => g
                        .name
                        .as_ref()
                        .is_some_and(|name| explicit_entities_matcher.is_match(name.as_str())),
                    GroupType::Scope => false,
                    GroupType::Undefined => false,
                }
            }
        };

        let matches_by_any = move |g: &Group| {
            if g.is_v2 {
                match g.r#type {
                    GroupType::AttributeGroup => {
                        all_attribute_groups_matcher.is_match(&g.id)
                            || g.id
                                .strip_prefix("registry.")
                                .is_some_and(|s| all_attribute_groups_matcher.is_match(s))
                            || g.id
                                .strip_prefix("attribute_group.")
                                .is_some_and(|s| all_attribute_groups_matcher.is_match(s))
                    }
                    GroupType::Span => {
                        all_spans_matcher.is_match(&g.id)
                            || g.name
                                .as_ref()
                                .is_some_and(|name| all_spans_matcher.is_match(name.as_str()))
                            || g.id
                                .strip_prefix("span.")
                                .is_some_and(|s| all_spans_matcher.is_match(s))
                    }
                    GroupType::Event => {
                        all_events_matcher.is_match(&g.id)
                            || g.name
                                .as_ref()
                                .is_some_and(|name| all_events_matcher.is_match(name.as_str()))
                            || g.id
                                .strip_prefix("event.")
                                .is_some_and(|s| all_events_matcher.is_match(s))
                    }
                    GroupType::Metric | GroupType::MetricGroup => {
                        all_metrics_matcher.is_match(&g.id)
                            || g.metric_name.as_ref().is_some_and(|metric_name| {
                                all_metrics_matcher.is_match(metric_name.as_str())
                            })
                            || g.id
                                .strip_prefix("metric.")
                                .is_some_and(|s| all_metrics_matcher.is_match(s))
                    }
                    GroupType::Entity => {
                        all_entities_matcher.is_match(&g.id)
                            || g.name
                                .as_ref()
                                .is_some_and(|name| all_entities_matcher.is_match(name.as_str()))
                            || g.id
                                .strip_prefix("entity.")
                                .is_some_and(|s| all_entities_matcher.is_match(s))
                    }
                    GroupType::Scope => false,
                    GroupType::Undefined => false,
                }
            } else {
                match g.r#type {
                    GroupType::AttributeGroup => all_attribute_groups_matcher.is_match(&g.id),
                    GroupType::Span => all_spans_matcher.is_match(&g.id),
                    GroupType::Event => g
                        .name
                        .as_ref()
                        .is_some_and(|name| all_events_matcher.is_match(name.as_str())),
                    GroupType::Metric => g.metric_name.as_ref().is_some_and(|metric_name| {
                        all_metrics_matcher.is_match(metric_name.as_str())
                    }),
                    GroupType::MetricGroup => false,
                    GroupType::Entity => g
                        .name
                        .as_ref()
                        .is_some_and(|name| all_entities_matcher.is_match(name.as_str())),
                    GroupType::Scope => false,
                    GroupType::Undefined => false,
                }
            }
        };

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
            result.push(GroupWithProvenance {
                group: g,
                schema_url: g_url,
            });
        }
        if !exclusion_errors.is_empty() {
            return Err(Error::CompoundError(exclusion_errors));
        }
        Ok(result)
    }
}

/// Finds the attribute source for a V1 attribute.
fn find_attribute_source(
    schema: &V1Schema,
    attr_name: &str,
    my_schema_url: &SchemaUrl,
) -> AttributeSource {
    if let Some((_, source_group_id)) = schema.catalog().root_attribute(attr_name) {
        let group = if let Some(schema_name) = source_group_id.strip_prefix("v2_dependency.") {
            schema.registry.groups.iter().find(|g| {
                if let Some(prov) = g.provenance() {
                    prov.schema_url.name() == schema_name
                } else {
                    false
                }
            })
        } else {
            schema
                .registry
                .groups
                .iter()
                .find(|g| g.id == *source_group_id)
        };
        if let Some(group) = group {
            if let Some(prov) = group.provenance() {
                AttributeSource::Dependency {
                    schema_url: prov.schema_url.clone(),
                }
            } else {
                AttributeSource::Dependency {
                    schema_url: my_schema_url.clone(),
                }
            }
        } else {
            AttributeSource::Dependency {
                schema_url: my_schema_url.clone(),
            }
        }
    } else {
        // Fallback: search in all groups to find where this attribute came from
        schema
            .registry
            .groups
            .iter()
            .find(|group| {
                group.attributes.iter().any(|ar| {
                    schema
                        .catalog()
                        .attribute(ar)
                        .is_some_and(|attr| attr.name == attr_name)
                })
            })
            .and_then(|group| {
                group.provenance().map(|prov| AttributeSource::Dependency {
                    schema_url: prov.schema_url.clone(),
                })
            })
            .unwrap_or_else(|| AttributeSource::Dependency {
                schema_url: my_schema_url.clone(),
            })
    }
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
    annotations: &std::collections::BTreeMap<String, weaver_semconv::YamlValue>,
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

/// Converts a V2 attribute (with no requirement level) to a v1 attribute.
fn convert_v2_attribute(
    attr: &weaver_resolved_schema::v2::attribute::Attribute,
    requirement_level: RequirementLevel,
    role: Option<AttributeRole>,
) -> Attribute {
    Attribute {
        name: attr.key.clone(),
        r#type: attr.r#type.clone(),
        brief: attr.common.brief.clone(),
        examples: attr.examples.clone(),
        tag: None,
        requirement_level,
        sampling_relevant: None,
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
impl ImportableDependency for V2Schema {
    fn import_groups<C: crate::SchemaCacheLookup>(
        &self,
        imports: &[ImportsWithProvenance],
        attribute_catalog: &mut AttributeCatalog,
        cache_lookup: &C,
    ) -> Result<Vec<GroupWithProvenance>, Error> {
        let mut result = vec![];
        let mut exclusion_errors: Vec<Error> = vec![];

        // Helper to map V2 provenance to V1 provenance.
        let get_source_provenance = |prov: &weaver_resolved_schema::v2::provenance::Provenance| -> weaver_semconv::provenance::Provenance {
            let url = if let Some(dep_ref) = &prov.source {
                self.dependencies.iter().nth(dep_ref.0 as usize).cloned().unwrap_or_else(|| self.schema_url.clone())
            } else {
                self.schema_url.clone()
            };
            weaver_semconv::provenance::Provenance::new(url, &prov.path)
        };

        // Helper to get attribute source based on provenance.
        let get_attribute_source =
            |attr: &weaver_resolved_schema::v2::attribute::Attribute| -> AttributeSource {
                if let Some(dep_ref) = &attr.provenance.source {
                    AttributeSource::Dependency {
                        schema_url: self
                            .dependencies
                            .iter()
                            .nth(dep_ref.0 as usize)
                            .cloned()
                            .unwrap_or_else(|| self.schema_url.clone()),
                    }
                } else {
                    AttributeSource::Dependency {
                        schema_url: self.schema_url.clone(),
                    }
                }
            };

        let explicit_imports: Vec<&ImportsWithProvenance> = imports
            .iter()
            .filter(|i| i.provenance.path != "--include-unreferenced")
            .collect();

        let explicit_metrics_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.metrics.as_deref().unwrap_or_default()),
        )?;
        let all_metrics_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.metrics.as_deref().unwrap_or_default()),
        )?;

        let explicit_events_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.events.as_deref().unwrap_or_default()),
        )?;
        let all_events_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.events.as_deref().unwrap_or_default()),
        )?;

        let explicit_entities_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.entities.as_deref().unwrap_or_default()),
        )?;
        let all_entities_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.entities.as_deref().unwrap_or_default()),
        )?;

        let explicit_spans_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.spans.as_deref().unwrap_or_default()),
        )?;
        let all_spans_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.spans.as_deref().unwrap_or_default()),
        )?;

        let explicit_attribute_groups_matcher = build_globset(
            explicit_imports
                .iter()
                .flat_map(|i| i.imports.attribute_groups.as_deref().unwrap_or_default()),
        )?;
        let all_attribute_groups_matcher = build_globset(
            imports
                .iter()
                .flat_map(|i| i.imports.attribute_groups.as_deref().unwrap_or_default()),
        )?;

        // First import metrics.  These are *by name* and come from the registry.
        // This is the closest to V1 ref syntax we have.
        for m in self.registry.metrics.iter() {
            let metric_name: &str = &m.name;
            let matched_explicitly = explicit_metrics_matcher.is_match(metric_name);
            let matched_by_any = all_metrics_matcher.is_match(metric_name);
            if !matched_by_any {
                continue;
            }
            match import_decision(
                &m.common.annotations,
                matched_explicitly,
                m.id(),
                GroupType::Metric,
            ) {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(e) => {
                    exclusion_errors.push(e);
                    continue;
                }
            }
            let mut attributes = vec![];
            for ar in m.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(attr, ar.requirement_level.clone(), None),
                    source,
                    cache_lookup,
                )?);
            }
            result.push(Group {
                id: m.id().to_owned(),
                r#type: GroupType::Metric,
                brief: m.common.brief.clone(),
                note: m.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(m.common.stability.clone()),
                deprecated: m.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: Some(m.name.to_string()),
                instrument: Some(m.instrument.clone()),
                unit: Some(m.unit.clone()),
                requirement_level: None,
                name: None,
                lineage: Some(weaver_resolved_schema::lineage::GroupLineage::new(
                    get_source_provenance(&m.provenance),
                )),
                display_name: None,
                body: None,
                annotations: Some(m.common.annotations.clone()),
                entity_associations: m.entity_associations.clone(),
                visibility: None,
                is_v2: true,
                span_name: None,
            });
        }

        // Now event imports.
        for e in self.registry.events.iter() {
            let event_name: &str = &e.name;
            let matched_explicitly = explicit_events_matcher.is_match(event_name);
            let matched_by_any = all_events_matcher.is_match(event_name);
            if !matched_by_any {
                continue;
            }
            match import_decision(
                &e.common.annotations,
                matched_explicitly,
                e.id(),
                GroupType::Event,
            ) {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(err) => {
                    exclusion_errors.push(err);
                    continue;
                }
            }
            let mut attributes = vec![];
            for ar in e.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(attr, ar.requirement_level.clone(), None),
                    source,
                    cache_lookup,
                )?);
            }
            result.push(Group {
                id: e.id().to_owned(),
                r#type: GroupType::Event,
                brief: e.common.brief.clone(),
                note: e.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(e.common.stability.clone()),
                deprecated: e.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some(e.name.to_string()),
                lineage: Some(weaver_resolved_schema::lineage::GroupLineage::new(
                    get_source_provenance(&e.provenance),
                )),
                display_name: None,
                body: None,
                annotations: Some(e.common.annotations.clone()),
                entity_associations: e.entity_associations.clone(),
                visibility: None,
                is_v2: true,
                span_name: None,
            });
        }

        // Now Entity imports.
        for e in self.registry.entities.iter() {
            let entity_type: &str = &e.r#type;
            let matched_explicitly = explicit_entities_matcher.is_match(entity_type);
            let matched_by_any = all_entities_matcher.is_match(entity_type);
            if !matched_by_any {
                continue;
            }
            match import_decision(
                &e.common.annotations,
                matched_explicitly,
                e.id(),
                GroupType::Entity,
            ) {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(err) => {
                    exclusion_errors.push(err);
                    continue;
                }
            }
            let mut attributes = vec![];
            for ar in e.identity.iter() {
                // TODO - this should be non-panic errors.
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(
                        attr,
                        ar.requirement_level.clone(),
                        Some(AttributeRole::Identifying),
                    ),
                    source,
                    cache_lookup,
                )?);
            }
            for ar in e.description.iter() {
                // TODO - this should be non-panic errors.
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(
                        attr,
                        ar.requirement_level.clone(),
                        Some(AttributeRole::Descriptive),
                    ),
                    source,
                    cache_lookup,
                )?);
            }
            result.push(Group {
                id: e.id().to_owned(),
                r#type: GroupType::Entity,
                brief: e.common.brief.clone(),
                note: e.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(e.common.stability.clone()),
                deprecated: e.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some(e.r#type.to_string()),
                lineage: Some(weaver_resolved_schema::lineage::GroupLineage::new(
                    get_source_provenance(&e.provenance),
                )),
                display_name: None,
                body: None,
                annotations: Some(e.common.annotations.clone()),
                entity_associations: vec![],
                visibility: None,
                is_v2: true,
                span_name: None,
            });
        }

        // Now Span imports.
        for s in self.registry.spans.iter() {
            let span_name: &str = &s.r#type;
            let matched_explicitly = explicit_spans_matcher.is_match(span_name);
            let matched_by_any = all_spans_matcher.is_match(span_name);
            if !matched_by_any {
                continue;
            }
            match import_decision(
                &s.common.annotations,
                matched_explicitly,
                s.id(),
                GroupType::Span,
            ) {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(err) => {
                    exclusion_errors.push(err);
                    continue;
                }
            }
            let mut attributes = vec![];
            for ar in s.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(attr, ar.requirement_level.clone(), None),
                    source,
                    cache_lookup,
                )?);
            }
            result.push(Group {
                id: s.id().to_owned(),
                r#type: GroupType::Span,
                brief: s.common.brief.clone(),
                note: s.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(s.common.stability.clone()),
                deprecated: s.common.deprecated.clone(),
                attributes,
                span_kind: Some(s.kind.clone()),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some(s.r#type.to_string()),
                lineage: Some(weaver_resolved_schema::lineage::GroupLineage::new(
                    get_source_provenance(&s.provenance),
                )),
                display_name: None,
                body: None,
                annotations: Some(s.common.annotations.clone()),
                entity_associations: s.entity_associations.clone(),
                visibility: None,
                is_v2: true,
                span_name: Some(s.name.clone()),
            });
        }

        // Now AttributeGroup imports.
        for ag in self.registry.attribute_groups.iter() {
            let ag_id: &str = &ag.id;
            let matched_explicitly = explicit_attribute_groups_matcher.is_match(ag_id);
            let matched_by_any = all_attribute_groups_matcher.is_match(ag_id);
            if !matched_by_any {
                continue;
            }
            match import_decision(
                &ag.common.annotations,
                matched_explicitly,
                ag.id(),
                GroupType::AttributeGroup,
            ) {
                ImportDecision::Include => {}
                ImportDecision::Skip => continue,
                ImportDecision::Error(err) => {
                    exclusion_errors.push(err);
                    continue;
                }
            }
            let mut attributes = vec![];
            for ar in ag.attributes.iter() {
                let attr = self.attribute_catalog.attribute(&ar.base).ok_or(
                    Error::InvalidRegistryAttributeRef {
                        registry_name: self.schema_url.name().to_owned(),
                        attribute_ref: ar.base.0,
                    },
                )?;
                let source = get_attribute_source(attr);
                attributes.push(attribute_catalog.attribute_ref_with_provenance(
                    convert_v2_attribute(attr, ar.requirement_level.clone(), None),
                    source,
                    cache_lookup,
                )?);
            }
            result.push(Group {
                id: ag.id().to_owned(),
                r#type: GroupType::AttributeGroup,
                brief: ag.common.brief.clone(),
                note: ag.common.note.clone(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(ag.common.stability.clone()),
                deprecated: ag.common.deprecated.clone(),
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: None,
                lineage: None,
                display_name: None,
                body: None,
                annotations: Some(ag.common.annotations.clone()),
                entity_associations: vec![],
                visibility: None,
                is_v2: true,
                span_name: None,
            });
        }
        if !exclusion_errors.is_empty() {
            return Err(Error::CompoundError(exclusion_errors));
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
        self.iter()
            .map(|d| d.import_groups(imports, attribute_catalog, cache_lookup))
            .try_fold(vec![], |mut result, next| {
                result.extend(next?);
                Ok(result)
            })
    }
}

/// Helper trait for abstracting over V1 and V2 schema.
pub(crate) trait GroupRefinementLookup {
    /// Looks up a group summary on this repo.
    /// id: The group id to find
    /// return: The summary of the group, or None if the group was not found.
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary>;
}

impl GroupRefinementLookup for V1Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.group(id).map(|g| {
            let attributes: Vec<UnresolvedAttribute> = g
                .attributes
                .iter()
                .filter_map(|ar| self.catalog.attribute(ar))
                .map(|a| UnresolvedAttribute {
                    spec: weaver_semconv::attribute::AttributeSpec::Id {
                        id: a.name.clone(),
                        r#type: a.r#type.clone(),
                        brief: Some(a.brief.clone()),
                        examples: a.examples.clone(),
                        tag: a.tag.clone(),
                        requirement_level: a.requirement_level.clone(),
                        sampling_relevant: a.sampling_relevant,
                        note: a.note.clone(),
                        stability: a.stability.clone(),
                        deprecated: a.deprecated.clone(),
                        annotations: a.annotations.clone(),
                        role: a.role.clone(),
                    },
                })
                .collect();
            let mut summary = GroupSummary::from_without_attributes(g, GroupSource::Dependency);
            summary.attributes = attributes;
            summary
        })
    }
}

/// Converts a v2 catalog attribute into an unresolved attribute spec with
/// the given requirement level and sampling relevance taken from the
/// signal's attribute reference.
///
/// TODO: this drops the attribute role (identifying vs descriptive). For
/// entity refinements over a v2 dependency that loses the identity/description
/// split — all inherited attributes collapse into `description` because the
/// v1→v2 conversion routes to `identity` only when the role is `Identifying`.
/// Thread the role through here and set it in the entity branch when we fix
/// entity refinement inheritance (see the ignored
/// `test_v2_dependency_entity_refinement_inherits_attributes`).
fn attr_spec(
    a: &weaver_resolved_schema::v2::attribute::Attribute,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
) -> UnresolvedAttribute {
    UnresolvedAttribute {
        spec: weaver_semconv::attribute::AttributeSpec::Id {
            id: a.key.clone(),
            r#type: a.r#type.clone(),
            brief: Some(a.common.brief.clone()),
            examples: a.examples.clone(),
            tag: None,
            requirement_level,
            sampling_relevant,
            note: a.common.note.clone(),
            stability: Some(a.common.stability.clone()),
            deprecated: a.common.deprecated.clone(),
            annotations: Some(a.common.annotations.clone()),
            role: None,
        },
    }
}

/// Builds a dependency group summary with the fields every v2 signal shares;
/// callers set the signal-specific fields (metric name, span kind, ...).
fn signal_summary(
    r#type: GroupType,
    common: &weaver_semconv::v2::CommonFields,
    requirement_level: Option<SignalRequirementLevel>,
    attributes: Vec<UnresolvedAttribute>,
) -> GroupSummary {
    GroupSummary {
        r#type,
        brief: common.brief.clone(),
        note: common.note.clone(),
        stability: Some(common.stability.clone()),
        deprecated: common.deprecated.clone(),
        metric_name: None,
        instrument: None,
        unit: None,
        requirement_level,
        span_kind: None,
        span_name: None,
        attributes,
        annotations: Some(common.annotations.clone()),
        source: GroupSource::Dependency,
    }
}

impl GroupRefinementLookup for V2Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        // groups that come from v2 published signals don't have `span|metrics|etc.` prefix
        // groups that come from v1 have it
        // strip it when looking up v1 groups
        fn find<'a, S: Signal>(signals: &'a [S], id: &str, prefix: &str) -> Option<&'a S> {
            let by_id = |n: &str| signals.iter().find(|s| s.id() == n);
            id.strip_prefix(prefix)
                .and_then(by_id)
                .or_else(|| by_id(id))
        }

        if let Some(e) = find(&self.registry.entities, id, "entity.") {
            let attributes = e
                .identity
                .iter()
                .chain(e.description.iter())
                .filter_map(|ar| {
                    self.attribute_catalog
                        .get(ar.base.0 as usize)
                        .map(|a| attr_spec(a, ar.requirement_level.clone(), None))
                })
                .collect();
            return Some(signal_summary(
                GroupType::Entity,
                &e.common,
                e.requirement_level.clone(),
                attributes,
            ));
        }
        if let Some(m) = find(&self.registry.metrics, id, "metric.") {
            let attributes = m
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog
                        .get(ar.base.0 as usize)
                        .map(|a| attr_spec(a, ar.requirement_level.clone(), None))
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Metric,
                &m.common,
                m.requirement_level.clone(),
                attributes,
            );
            summary.metric_name = Some(m.name.to_string());
            summary.instrument = Some(m.instrument.clone());
            summary.unit = Some(m.unit.clone());
            return Some(summary);
        }
        if let Some(e) = find(&self.registry.events, id, "event.") {
            let attributes = e
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog
                        .get(ar.base.0 as usize)
                        .map(|a| attr_spec(a, ar.requirement_level.clone(), None))
                })
                .collect();
            return Some(signal_summary(
                GroupType::Event,
                &e.common,
                e.requirement_level.clone(),
                attributes,
            ));
        }
        if let Some(s) = find(&self.registry.spans, id, "span.") {
            let attributes = s
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog
                        .get(ar.base.0 as usize)
                        .map(|a| attr_spec(a, ar.requirement_level.clone(), ar.sampling_relevant))
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Span,
                &s.common,
                s.requirement_level.clone(),
                attributes,
            );
            summary.span_kind = Some(s.kind.clone());
            summary.span_name = Some(s.name.clone());
            return Some(summary);
        }
        None
    }
}

impl GroupRefinementLookup for Vec<ResolvedDependency> {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        self.iter().find_map(|d| d.lookup_group_summary(id))
    }
}

impl From<V1Schema> for ResolvedDependency {
    fn from(value: V1Schema) -> Self {
        ResolvedDependency::V1(Box::new(value))
    }
}

impl From<V2Schema> for ResolvedDependency {
    fn from(value: V2Schema) -> Self {
        ResolvedDependency::V2(Box::new(value))
    }
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
    use itertools::Itertools;
    use std::{collections::HashMap, error::Error};
    use weaver_resolved_schema::ResolvedTelemetrySchema as V1Schema;

    use crate::dependency::{GroupRefinementLookup, ResolvedDependency};

    #[test]
    fn test_lookup_group_summary() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::V1(Box::new(example_v1_schema()));
        let result = d.lookup_group_summary("a");
        assert!(
            result.is_some(),
            "Should find group summary for `a` on {d:?}"
        );
        if let Some(summary) = result.as_ref() {
            assert!(
                !summary.attributes.is_empty(),
                "Should find attributes for group `a`, found none."
            );
            assert_eq!(summary.attributes[0].spec.id(), "a.test");
        }
        let ds = vec![d];
        let result2 = ds.lookup_group_summary("a");
        // Assert we get the same if we look across a vector vs. raw.
        assert_eq!(
            result.map(|a| a.attributes.iter().map(|a| a.spec.id()).collect_vec()),
            result2.map(|a| a.attributes.iter().map(|a| a.spec.id()).collect_vec())
        );
        Ok(())
    }

    fn example_v1_schema() -> V1Schema {
        V1Schema {
            file_format: "resolved/1.0".parse().unwrap(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "test-registry".to_owned(),
            registry: weaver_resolved_schema::registry::Registry {
                registry_url: "v1-example".to_owned(),
                groups: vec![
                    weaver_resolved_schema::registry::Group {
                        id: "a".to_owned(),
                        r#type: weaver_semconv::group::GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![weaver_resolved_schema::attribute::AttributeRef(0)],
                        span_kind: Default::default(),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        requirement_level: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                        is_v2: Default::default(),
                        span_name: None,
                    },
                    weaver_resolved_schema::registry::Group {
                        id: "span.v1".to_owned(),
                        r#type: weaver_semconv::group::GroupType::Span,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![],
                        span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        requirement_level: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                        is_v2: Default::default(),
                        span_name: None,
                    },
                ],
            },
            catalog: weaver_resolved_schema::catalog::Catalog::new(
                vec![weaver_resolved_schema::attribute::Attribute {
                    name: "a.test".to_owned(),
                    r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    brief: Default::default(),
                    examples: Default::default(),
                    tag: Default::default(),
                    requirement_level: Default::default(),
                    sampling_relevant: Default::default(),
                    note: Default::default(),
                    stability: Default::default(),
                    deprecated: Default::default(),
                    prefix: Default::default(),
                    tags: None,
                    annotations: Default::default(),
                    value: Default::default(),
                    role: Default::default(),
                }],
                HashMap::new(),
            ),
            resource: None,
            instrumentation_library: None,
            dependencies: std::collections::BTreeSet::new(),
            versions: None,
            registry_manifest: None,
        }
    }

    fn example_v2_schema() -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".parse().unwrap(),
            schema_url: "http://test/schemas/2.0.0".try_into().unwrap(),
            registry: weaver_resolved_schema::v2::registry::Registry {
                attribute_groups: vec![
                    weaver_resolved_schema::v2::attribute_group::AttributeGroup {
                        id: "attribute_group.e".to_owned().into(),
                        // A public group whose attribute ref carries a
                        // non-default requirement level; importing the group
                        // must preserve it.
                        attributes: vec![
                            weaver_resolved_schema::v2::attribute_group::AttributeGroupAttributeRef {
                                base: weaver_resolved_schema::v2::attribute::AttributeRef(0),
                                requirement_level:
                                    weaver_semconv::attribute::RequirementLevel::Basic(
                                        weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                                    ),
                            },
                        ],
                        common: Default::default(),
                        provenance: Default::default(),
                    },
                ],
                metrics: vec![weaver_resolved_schema::v2::metric::Metric {
                    name: "metric.a".to_owned().into(),
                    instrument: weaver_semconv::group::InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: Default::default(),
                    provenance: Default::default(),
                }],
                events: vec![weaver_resolved_schema::v2::event::Event {
                    name: "event.b".to_owned().into(),
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: Default::default(),
                    provenance: Default::default(),
                }],
                spans: vec![weaver_resolved_schema::v2::span::Span {
                    r#type: "span.d".to_owned().into(),
                    kind: weaver_semconv::group::SpanKindSpec::Client,
                    name: weaver_semconv::v2::span::SpanName {
                        note: "test".to_owned(),
                    },
                    attributes: vec![],
                    entity_associations: vec![],
                    requirement_level: None,
                    common: Default::default(),
                    provenance: Default::default(),
                }],
                entities: vec![weaver_resolved_schema::v2::entity::Entity {
                    r#type: "entity.c".to_owned().into(),
                    identity: vec![],
                    description: vec![],
                    requirement_level: None,
                    common: Default::default(),
                    provenance: Default::default(),
                }],
                attributes: vec![],
            },
            attribute_catalog: vec![weaver_resolved_schema::v2::attribute::Attribute {
                key: "attr.in.group".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: Default::default(),
                provenance: Default::default(),
            }],
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
            },
            dependencies: std::collections::BTreeSet::new(),
        }
    }

    #[test]
    fn test_lookup_group_summary_v2() -> Result<(), Box<dyn Error>> {
        let d = ResolvedDependency::V2(Box::new(example_v2_schema()));

        let result_metric = d.lookup_group_summary("metric.a");
        assert!(result_metric.is_some(), "Should find metric.a");
        assert_eq!(
            result_metric.unwrap().r#type,
            weaver_semconv::group::GroupType::Metric
        );

        let result_event = d.lookup_group_summary("event.b");
        assert!(result_event.is_some(), "Should find event.b");
        assert_eq!(
            result_event.unwrap().r#type,
            weaver_semconv::group::GroupType::Event
        );

        let result_entity = d.lookup_group_summary("entity.c");
        assert!(result_entity.is_some(), "Should find entity.c");
        assert_eq!(
            result_entity.unwrap().r#type,
            weaver_semconv::group::GroupType::Entity
        );

        let result_span = d.lookup_group_summary("span.d");
        assert!(result_span.is_some(), "Should find span.d");
        let span_summary = result_span.unwrap();
        assert_eq!(span_summary.r#type, weaver_semconv::group::GroupType::Span);
        assert_eq!(
            span_summary.span_kind,
            Some(weaver_semconv::group::SpanKindSpec::Client)
        );
        // The span name (with its note) is carried over so refinements that do
        // not override it inherit the dependency's definition.
        assert_eq!(
            span_summary.span_name,
            Some(weaver_semconv::v2::span::SpanName {
                note: "test".to_owned(),
            })
        );

        // Unknown ids resolve to nothing.
        assert!(
            d.lookup_group_summary("does.not.exist").is_none(),
            "Should not find an unknown group id"
        );

        Ok(())
    }

    #[test]
    fn test_import_groups_v1() -> Result<(), Box<dyn Error>> {
        use crate::dependency::ImportableDependency;
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
        use crate::dependency::ImportableDependency;
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

    #[test]
    fn test_import_groups_vec() -> Result<(), Box<dyn Error>> {
        use crate::dependency::ImportableDependency;
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
        use crate::dependency::ImportableDependency;
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
