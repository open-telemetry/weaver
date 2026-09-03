// SPDX-License-Identifier: Apache-2.0

//! Helpers to handle reading from dependencies.
//!
//! A dependency holds definitions that the local registry looks up: a
//! refinement or an `extends` clause names a group, and this module answers
//! with a [`GroupSummary`] regardless of whether the dependency is a v1 or a
//! v2 schema.
//!
//! Pulling definitions in through an `imports` block lives in
//! [`crate::imports`], which resolves an attribute's origin registry with the
//! same helpers used here.

use weaver_resolved_schema::v1::attribute::UnresolvedAttribute;
use weaver_resolved_schema::v1::registry::Group;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_resolved_schema::v2::entity::Entity;
use weaver_resolved_schema::v2::provenance::DependencyRef;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
use weaver_resolved_schema::v2::Signal;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::stability::Stability;
use weaver_semconv::v1::attribute::{AttributeRole, RequirementLevel};
use weaver_semconv::v1::group::{GroupType, InstrumentSpec, SpanKindSpec};

use crate::attribute::AttributeSource;
use crate::dependency_resolution::is_excluded;
use crate::imports::import_match_keys;

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
    pub span_name: Option<weaver_semconv::v1::group::SpanName>,
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

    /// Looks up an entity by a name an `entity_associations` entry can use.
    pub(crate) fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        match self {
            ResolvedDependency::V1(schema) => schema.lookup_entity(name),
            ResolvedDependency::V2(schema) => schema.lookup_entity(name),
        }
    }
}

/// Where an entity that an association names was found.
#[derive(Debug, Clone)]
pub(crate) struct EntityLocation {
    /// The registry that declared the entity, which is the dependency itself
    /// unless the dependency re-exports a definition of its own dependency.
    pub origin: SchemaUrl,
    /// True when the declaring registry hides the entity from its dependents.
    pub excluded: bool,
}

/// Looking an entity up by association, as opposed to importing it.
pub(crate) trait EntityLookup {
    /// Looks up an entity by the type or refinement id that an
    /// `entity_associations` entry can name. Returns `None` when this schema
    /// holds no such entity.
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation>;
}

impl EntityLookup for V1Schema {
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        let my_schema_url = SchemaUrl::try_from(self.schema_url.as_str()).ok()?;
        self.registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::Entity)
            .find(|g| import_match_keys(g).contains(&name))
            .map(|g| EntityLocation {
                origin: g
                    .provenance()
                    .map(|prov| prov.schema_url)
                    .unwrap_or(my_schema_url),
                excluded: g.annotations.as_ref().is_some_and(is_excluded),
            })
    }
}

impl EntityLookup for V2Schema {
    fn lookup_entity(&self, name: &str) -> Option<EntityLocation> {
        let deps: Vec<_> = self.dependencies.iter().cloned().collect();
        // A base entity answers to its type, a refinement to its id. Weaver
        // holds the two in one namespace, so an association names either.
        let entity = self
            .registry
            .entities
            .iter()
            .find(|e| &*e.r#type == name)
            .or_else(|| {
                self.refinements
                    .entities
                    .iter()
                    .find(|r| &*r.id == name)
                    .map(|r| &r.entity)
            })?;
        Some(EntityLocation {
            origin: v2_source_url(self, &deps, entity.provenance.source),
            excluded: is_excluded(&entity.common.annotations),
        })
    }
}

/// What the dependencies answer when an association names an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntityResolution {
    /// One registry declares the entity, and it is the one named here.
    Found(SchemaUrl),
    /// Every registry that holds the name keeps the entity private.
    Private,
    /// Two or more registries declare unrelated entities under the name, and
    /// nothing says which one is meant.
    Ambiguous(Vec<SchemaUrl>),
    /// No dependency holds the name.
    Unknown,
}

/// Looks an association name up across every dependency at once.
///
/// The order the manifest lists dependencies in says nothing about which one an
/// association means, so every dependency is asked and the answers are weighed
/// together rather than the first hit being taken.
///
/// Candidates are counted by the registry that declared the entity, as importing
/// does. One definition reached by two paths through the dependency graph is
/// still one definition. Two definitions that merely share a name are not.
pub(crate) fn resolve_entity(deps: &[ResolvedDependency], name: &str) -> EntityResolution {
    let mut origins: Vec<SchemaUrl> = vec![];
    let mut private = false;
    for location in deps.iter().filter_map(|d| d.lookup_entity(name)) {
        if location.excluded {
            // A private entity is no part of the surface a dependency offers.
            private = true;
        } else if !origins.contains(&location.origin) {
            origins.push(location.origin);
        }
    }
    match origins.len() {
        0 if private => EntityResolution::Private,
        0 => EntityResolution::Unknown,
        1 => EntityResolution::Found(origins.swap_remove(0)),
        _ => EntityResolution::Ambiguous(origins),
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
        let my_schema_url = SchemaUrl::try_from(self.schema_url.as_str()).ok();
        self.group(id).map(|g| {
            let attributes: Vec<UnresolvedAttribute> = g
                .attributes
                .iter()
                .filter_map(|ar| self.catalog.attribute(ar))
                .map(|a| UnresolvedAttribute {
                    origin: my_schema_url.as_ref().map(|url| {
                        match find_attribute_source(self, &a.name, url) {
                            AttributeSource::Dependency { schema_url } => schema_url,
                            AttributeSource::Local { .. } => url.clone(),
                        }
                    }),
                    spec: weaver_semconv::v1::attribute::AttributeSpec::Id {
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

/// Finds the attribute source for a V1 attribute: the registry that declared
/// it, recovered from the catalog's root-attribute table or, failing that, from
/// the provenance of a group that references it.
pub(crate) fn find_attribute_source(
    schema: &V1Schema,
    attr_name: &str,
    my_schema_url: &SchemaUrl,
) -> AttributeSource {
    if let Some((_, source_group_id)) = schema.catalog().root_attribute(attr_name) {
        let schema_url = if let Some(schema_name) = source_group_id.strip_prefix("v2_dependency.") {
            // The attribute originates in one of `schema`'s own dependencies.
            // That registry may not have contributed any whole group to
            // `schema` (e.g. only an attribute was referenced), so recover
            // the full schema URL from the dependency list first and only
            // fall back to the provenance of an imported group.
            schema
                .dependencies
                .iter()
                .find(|url| url.name() == schema_name)
                .cloned()
                .or_else(|| {
                    schema.registry.groups.iter().find_map(|g| {
                        g.provenance()
                            .filter(|prov| prov.schema_url.name() == schema_name)
                            .map(|prov| prov.schema_url)
                    })
                })
        } else {
            schema
                .registry
                .groups
                .iter()
                .find(|g| g.id == *source_group_id)
                .and_then(|g| g.provenance().map(|prov| prov.schema_url))
        };
        AttributeSource::Dependency {
            schema_url: schema_url.unwrap_or_else(|| my_schema_url.clone()),
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

/// The registry a v2 signal or attribute came from: one of `schema`'s own
/// dependencies when its provenance names one, otherwise `schema` itself.
///
/// `deps` is `schema.dependencies` as a slice — the table a [`DependencyRef`]
/// indexes into. Callers materialise it once per schema rather than walking
/// the set on every lookup.
pub(crate) fn v2_source_url(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    source: Option<DependencyRef>,
) -> SchemaUrl {
    source
        .and_then(|dep_ref| deps.get(dep_ref.0 as usize).cloned())
        .unwrap_or_else(|| schema.schema_url.clone())
}

/// Converts a v2 catalog attribute into an unresolved attribute spec with
/// the given requirement level, sampling relevance and role taken from the
/// signal's attribute reference.
fn attr_spec(
    schema: &V2Schema,
    deps: &[SchemaUrl],
    a: &weaver_resolved_schema::v2::attribute::Attribute,
    requirement_level: RequirementLevel,
    sampling_relevant: Option<bool>,
    role: Option<AttributeRole>,
) -> UnresolvedAttribute {
    UnresolvedAttribute {
        origin: Some(v2_source_url(schema, deps, a.provenance.source)),
        spec: weaver_semconv::v1::attribute::AttributeSpec::Id {
            id: a.key.clone(),
            r#type: weaver_semconv::convert::v2_attribute_type_to_v1(a.r#type.clone()),
            brief: Some(a.common.brief.clone()),
            examples: a
                .examples
                .clone()
                .map(weaver_semconv::convert::v2_examples_to_v1),
            tag: None,
            requirement_level,
            sampling_relevant,
            note: a.common.note.clone(),
            stability: Some(a.common.stability.clone()),
            deprecated: a.common.deprecated.clone(),
            annotations: Some(a.common.annotations.clone()),
            role,
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

/// Builds a group summary for an entity, with identity attributes tagged
/// with the identifying role and description attributes with the
/// descriptive role, so refinements inherit them correctly.
fn entity_group_summary(schema: &V2Schema, deps: &[SchemaUrl], e: &Entity) -> GroupSummary {
    let attributes = e
        .identity
        .iter()
        .map(|ar| (ar, AttributeRole::Identifying))
        .chain(
            e.description
                .iter()
                .map(|ar| (ar, AttributeRole::Descriptive)),
        )
        .filter_map(|(ar, role)| {
            schema.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                attr_spec(
                    schema,
                    deps,
                    a,
                    weaver_semconv::convert::v2_requirement_level_to_v1(
                        ar.requirement_level.clone(),
                    ),
                    None,
                    Some(role),
                )
            })
        })
        .collect();
    signal_summary(
        GroupType::Entity,
        &e.common,
        e.requirement_level.clone(),
        attributes,
    )
}

impl GroupRefinementLookup for V2Schema {
    fn lookup_group_summary(&self, id: &str) -> Option<GroupSummary> {
        // An `extends` clause references either the v1 group id
        // (`entity.host`, written by v2 refinements over prefix-stripped
        // published signals) or the raw published signal id
        // (`parent.metric`, written by v1 groups), so try the stripped id
        // first and fall back to the raw id.
        fn find<'a, S: Signal>(signals: &'a [S], id: &str, prefix: &str) -> Option<&'a S> {
            let by_id = |n: &str| signals.iter().find(|s| s.id() == n);
            id.strip_prefix(prefix)
                .and_then(by_id)
                .or_else(|| by_id(id))
        }

        let deps: Vec<_> = self.dependencies.iter().cloned().collect();

        if let Some(e) = find(&self.registry.entities, id, "entity.") {
            return Some(entity_group_summary(self, &deps, e));
        }
        if let Some(m) = find(&self.registry.metrics, id, "metric.") {
            let attributes = m
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            weaver_semconv::convert::v2_requirement_level_to_v1(
                                ar.requirement_level.clone(),
                            ),
                            None,
                            None,
                        )
                    })
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Metric,
                &m.common,
                m.requirement_level.clone(),
                attributes,
            );
            summary.metric_name = Some(m.name.to_string());
            summary.instrument = Some(weaver_semconv::convert::v2_instrument_to_v1(m.instrument));
            summary.unit = Some(m.unit.clone());
            return Some(summary);
        }
        if let Some(e) = find(&self.registry.events, id, "event.") {
            let attributes = e
                .attributes
                .iter()
                .filter_map(|ar| {
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            weaver_semconv::convert::v2_requirement_level_to_v1(
                                ar.requirement_level.clone(),
                            ),
                            None,
                            None,
                        )
                    })
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
                    self.attribute_catalog.get(ar.base.0 as usize).map(|a| {
                        attr_spec(
                            self,
                            &deps,
                            a,
                            weaver_semconv::convert::v2_requirement_level_to_v1(
                                ar.requirement_level.clone(),
                            ),
                            ar.sampling_relevant,
                            None,
                        )
                    })
                })
                .collect();
            let mut summary = signal_summary(
                GroupType::Span,
                &s.common,
                s.requirement_level.clone(),
                attributes,
            );
            summary.span_kind = Some(weaver_semconv::convert::v2_span_kind_to_v1(s.kind));
            summary.span_name = Some(weaver_semconv::convert::v2_span_name_to_v1(s.name.clone()));
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

#[cfg(test)]
pub(crate) mod tests {
    use itertools::Itertools;
    use std::{collections::HashMap, error::Error};
    use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;

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

    pub(crate) fn example_v1_schema() -> V1Schema {
        V1Schema {
            file_format: "resolved/1.0.0".to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "test-registry".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "v1-example".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![
                    weaver_resolved_schema::v1::registry::Group {
                        id: "a".to_owned(),
                        r#type: weaver_semconv::v1::group::GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![weaver_resolved_schema::v1::attribute::AttributeRef(0)],
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
                    weaver_resolved_schema::v1::registry::Group {
                        id: "span.v1".to_owned(),
                        r#type: weaver_semconv::v1::group::GroupType::Span,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: vec![],
                        span_kind: Some(weaver_semconv::v1::group::SpanKindSpec::Client),
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
            catalog: weaver_resolved_schema::v1::catalog::Catalog::new(
                vec![weaver_resolved_schema::v1::attribute::Attribute {
                    name: "a.test".to_owned(),
                    r#type: weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String,
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

    pub(crate) fn example_v2_schema() -> weaver_resolved_schema::v2::ResolvedTelemetrySchema {
        weaver_resolved_schema::v2::ResolvedTelemetrySchema {
            file_format: "resolved/2.0".to_owned(),
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
                                    weaver_semconv::v2::attribute::RequirementLevel::Basic(
                                        weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required,
                                    ),
                            },
                        ],
                        common: Default::default(),
                        provenance: Default::default(),
                    },
                ],
                metrics: vec![weaver_resolved_schema::v2::metric::Metric {
                    name: "metric.a".to_owned().into(),
                    instrument: weaver_semconv::v2::metric::InstrumentSpec::Counter,
                    unit: "1".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![
                        weaver_resolved_schema::v2::entity::EntityAssociation::Ref(
                            weaver_resolved_schema::v2::entity::EntityRef::local(
                                "entity.c".to_owned().into(),
                            ),
                        ),
                    ],
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
                    kind: weaver_semconv::v2::span::SpanKindSpec::Client,
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
                    // An identity and a description attribute, so importing
                    // the entity has to tag each one with its role.
                    identity: vec![weaver_resolved_schema::v2::entity::EntityAttributeRef {
                        base: weaver_resolved_schema::v2::attribute::AttributeRef(1),
                        requirement_level: Default::default(),
                    }],
                    description: vec![weaver_resolved_schema::v2::entity::EntityAttributeRef {
                        base: weaver_resolved_schema::v2::attribute::AttributeRef(2),
                        requirement_level: Default::default(),
                    }],
                    requirement_level: None,
                    common: Default::default(),
                    provenance: Default::default(),
                }],
                attributes: vec![],
            },
            attribute_catalog: vec![
                weaver_resolved_schema::v2::attribute::Attribute {
                    key: "attr.in.group".to_owned(),
                    r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    examples: None,
                    common: Default::default(),
                    provenance: Default::default(),
                },
                weaver_resolved_schema::v2::attribute::Attribute {
                    key: "entity.c.id".to_owned(),
                    r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    examples: None,
                    common: Default::default(),
                    provenance: Default::default(),
                },
                weaver_resolved_schema::v2::attribute::Attribute {
                    key: "entity.c.label".to_owned(),
                    r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                        weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
                    ),
                    examples: None,
                    common: Default::default(),
                    provenance: Default::default(),
                },
            ],
            refinements: weaver_resolved_schema::v2::refinements::Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
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
            weaver_semconv::v1::group::GroupType::Metric
        );

        let result_event = d.lookup_group_summary("event.b");
        assert!(result_event.is_some(), "Should find event.b");
        assert_eq!(
            result_event.unwrap().r#type,
            weaver_semconv::v1::group::GroupType::Event
        );

        let result_entity = d.lookup_group_summary("entity.c");
        assert!(result_entity.is_some(), "Should find entity.c");
        assert_eq!(
            result_entity.unwrap().r#type,
            weaver_semconv::v1::group::GroupType::Entity
        );

        let result_span = d.lookup_group_summary("span.d");
        assert!(result_span.is_some(), "Should find span.d");
        let span_summary = result_span.unwrap();
        assert_eq!(
            span_summary.r#type,
            weaver_semconv::v1::group::GroupType::Span
        );
        assert_eq!(
            span_summary.span_kind,
            Some(weaver_semconv::v1::group::SpanKindSpec::Client)
        );
        // The span name (with its note) is carried over so refinements that do
        // not override it inherit the dependency's definition.
        assert_eq!(
            span_summary.span_name,
            Some(weaver_semconv::v1::group::SpanName {
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
}
