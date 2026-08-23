//! Version 2 of semantic convention schema.

use std::collections::{BTreeSet, HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    deprecated::Deprecated,
    group::GroupType,
    schema_url::SchemaUrl,
    v2::{
        attribute_group::AttributeGroupVisibilitySpec, signal_id::SignalId, span::SpanName,
        CommonFields,
    },
};
use weaver_version::v2::{RegistryChanges, SchemaChanges, SchemaItemChange};

use crate::{
    v2::{
        attribute::Attribute,
        attribute_group::AttributeGroup,
        catalog::{AttributeCatalog, Catalog},
        entity::Entity,
        metric::Metric,
        refinements::Refinements,
        registry::Registry,
        span::{Span, SpanRefinement},
        stats::Stats,
    },
    V2_RESOLVED_FILE_FORMAT,
};

pub mod attribute;
pub mod attribute_group;
pub mod catalog;
pub mod entity;
pub mod event;
pub mod metric;
pub mod provenance;
pub mod refinements;
pub mod registry;
pub mod span;
pub mod stats;

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    /// Always `"resolved/2.0"` in this version.
    #[schemars(extend("const" = "resolved/2.0"))]
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: SchemaUrl,
    /// Catalog of attributes. Note: this will include duplicates for the same key.
    pub attribute_catalog: Vec<Attribute>,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Refinements for the registry
    pub refinements: Refinements,
    /// Every registry this schema was built from, direct and transitive.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dependencies: BTreeSet<SchemaUrl>,
}

impl ResolvedTelemetrySchema {
    /// Statistics about this schema.
    pub fn stats(&self) -> Stats {
        Stats {
            registry: self.registry.stats(&self.attribute_catalog),
            refinements: self.refinements.stats(),
        }
    }

    /// Generate a diff between the current schema (must be the most recent one)
    /// and a baseline schema.
    #[must_use]
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> SchemaChanges {
        // TODO - get manifests
        SchemaChanges {
            head_schema_url: self.schema_url.clone(),
            baseline_schema_url: baseline_schema.schema_url.clone(),
            registry: self.registry_diff(baseline_schema),
        }
    }

    #[must_use]
    fn registry_diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> RegistryChanges {
        RegistryChanges {
            attribute_changes: self.registry_attribute_diff(baseline_schema),
            attribute_group_changes: diff_signals(
                &self.registry.attribute_groups,
                &baseline_schema.registry.attribute_groups,
            ),
            entity_changes: diff_signals(
                &self.registry.entities,
                &baseline_schema.registry.entities,
            ),
            event_changes: diff_signals(&self.registry.events, &baseline_schema.registry.events),
            metric_changes: diff_signals(&self.registry.metrics, &baseline_schema.registry.metrics),
            span_changes: diff_signals(&self.registry.spans, &baseline_schema.registry.spans),
        }
    }

    #[must_use]
    fn registry_attribute_diff(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
    ) -> Vec<SchemaItemChange> {
        let latest_attributes = self.registry_attribute_map();
        let baseline_attributes = baseline_schema.registry_attribute_map();
        diff_signals_by_hash(&latest_attributes, &baseline_attributes)
    }

    /// Get the registry attributes of the resolved telemetry schema in a fast lookup map.
    fn registry_attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .attributes
            .iter()
            .filter_map(|r| self.attribute_catalog.attribute(r))
            .map(|a| (a.key.as_str(), a))
            .collect()
    }
}

/// Easy conversion from v1 to v2.
impl TryFrom<crate::ResolvedTelemetrySchema> for ResolvedTelemetrySchema {
    type Error = crate::error::Error;
    fn try_from(value: crate::ResolvedTelemetrySchema) -> Result<Self, Self::Error> {
        let (attribute_catalog, registry, refinements, dependencies) =
            convert_v1_to_v2(value.catalog, value.registry, value.dependencies)?;
        let schema_url_str = value.schema_url.clone();
        let schema_url: SchemaUrl =
            value
                .schema_url
                .try_into()
                .map_err(|e| crate::error::Error::InvalidSchemaUrl {
                    url: schema_url_str,
                    error: e,
                })?;

        Ok(ResolvedTelemetrySchema {
            file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url,
            attribute_catalog,
            registry,
            refinements,
            dependencies,
        })
    }
}

/// Turns the name an association entry uses into an entity reference.
struct EntityRefResolver<'a> {
    /// Every entity and refinement of this registry, by the name an association
    /// uses, and the registry each was declared in.
    local: &'a HashMap<SignalId, Option<provenance::DependencyRef>>,
    /// The dependency list, indexed by url.
    dep_index: &'a HashMap<&'a SchemaUrl, provenance::DependencyRef>,
    /// Where the associations of each group resolved.
    origins: &'a crate::registry::EntityAssociationOrigins,
}

impl EntityRefResolver<'_> {
    /// The reference one entry of one group becomes. Returns `None` when
    /// nothing defines the name.
    fn resolve(&self, group_id: &str, name: &str) -> Option<entity::EntityRef> {
        let Some(origin) = self.origins.get(group_id).and_then(|g| g.get(name)) else {
            // No origin recorded, so the entity is one this registry holds.
            let name = SignalId::from(name.to_owned());
            return self
                .local
                .contains_key(&name)
                .then(|| entity::EntityRef::local(name));
        };
        let source = self.dep_index.get(origin).copied();
        if source.is_none() {
            // The resolver found the entity in a dependency, so its url belongs
            // to the dependency closure that `dep_index` holds. An empty
            // provenance would claim the entity is local.
            log::warn!(
                "Logic failure - entity `{name}` resolved to `{origin}`, which is not a dependency"
            );
        }
        let name = SignalId::from(name.to_owned());
        // This registry may hold an entity of the same name. It is the same
        // definition only when the same registry declared it, and then the
        // reference stays local. Otherwise the two merely share a name, and the
        // reference must keep naming the one the association resolved to.
        if self.local.get(&name) == Some(&source) {
            return Some(entity::EntityRef::local(name));
        }
        Some(entity::EntityRef {
            r#type: name,
            provenance: provenance::Provenance {
                source,
                ..Default::default()
            },
        })
    }
}

/// Turns the names in association expressions into entity references. The shape
/// of an expression does not change, only its leaves.
fn convert_entity_associations(
    associations: &[weaver_semconv::entity_association::EntityAssociation],
    entity_refs: &EntityRefResolver<'_>,
    group_id: &str,
) -> Result<Vec<entity::EntityAssociation>, crate::error::Error> {
    use weaver_semconv::entity_association::EntityAssociation as SpecAssociation;
    associations
        .iter()
        .map(|assoc| match assoc {
            SpecAssociation::Ref(name) => entity_refs
                .resolve(group_id, name)
                .map(entity::EntityAssociation::Ref)
                .ok_or_else(|| crate::error::Error::EntityAssociationNotFound {
                    group_id: group_id.to_owned(),
                    entity_type: name.clone(),
                }),
            SpecAssociation::OneOf { one_of } => Ok(entity::EntityAssociation::OneOf {
                one_of: convert_entity_associations(one_of, entity_refs, group_id)?,
            }),
            SpecAssociation::AllOf { all_of } => Ok(entity::EntityAssociation::AllOf {
                all_of: convert_entity_associations(all_of, entity_refs, group_id)?,
            }),
        })
        .collect()
}

/// Strips the group-type prefix that a v2 definition adds to a group id.
///
/// A v2 definition names a signal by type alone, and the v1 group model holds
/// every signal type in one flat id space, so reading a v2 file mints an id
/// like `entity.host`. This undoes that.
///
/// Strips one prefix only. `trim_start_matches` would strip a repeat, so
/// `entity.entity.host` would lose both.
fn fix_group_id(prefix: &'static str, group_id: &str) -> SignalId {
    group_id
        .strip_prefix(prefix)
        .unwrap_or(group_id)
        .to_owned()
        .into()
}

fn fix_span_group_id(group_id: &str) -> SignalId {
    fix_group_id("span.", group_id)
}

/// Whether this group refines another signal of its own type, rather than
/// defining one of its own.
fn is_refinement_of(group: &crate::registry::Group) -> bool {
    group
        .lineage
        .as_ref()
        .is_some_and(|l| l.extends_group_type.as_ref() == Some(&group.r#type))
}

/// The entity type that a v1 entity group declares.
///
/// A v1 entity holds its type in `name`, and its id is free of it: the legacy
/// `resource` groups of semconv are `resource.host` with the name `host`. A v2
/// definition mints the id `entity.<type>` and repeats the type in `name`, so
/// the two agree there. A group with no name at all keeps its type in the id,
/// which a legacy `resource` group may also do.
fn entity_type_of(group: &crate::registry::Group) -> SignalId {
    group
        .name
        .clone()
        .map(SignalId::from)
        .unwrap_or_else(|| fix_group_id("entity.", &group.id))
}

/// The id that a v1 group takes in the v2 namespace of its signal type.
///
/// A definition and a refinement of one share a namespace, because a definition
/// also gets a refinement entry under its own name. Two v1 groups that take one
/// id here therefore collapse onto one entry in the v2 output, and the second
/// silently replaces the first.
///
/// The conversion derives every id from this one rule, and the resolver reports
/// a collision with it, so the check and the conversion cannot drift apart.
///
/// Returns `None` for a group type the v2 conversion drops, and for a v1 event
/// or metric that has no name to take.
#[must_use]
pub fn v2_namespace_id(group: &crate::registry::Group) -> Option<SignalId> {
    let refines = is_refinement_of(group);
    match group.r#type {
        GroupType::Span => Some(fix_span_group_id(&group.id)),
        GroupType::AttributeGroup => Some(fix_group_id("attribute_group.", &group.id)),
        // A refinement is named by its own id. A definition is named by the
        // signal name, which for these three types is a field of its own.
        GroupType::Entity if refines => Some(fix_group_id("entity.", &group.id)),
        GroupType::Entity => Some(entity_type_of(group)),
        GroupType::Event if refines => Some(fix_group_id("event.", &group.id)),
        GroupType::Event => group.name.clone().map(SignalId::from),
        GroupType::Metric if refines => Some(fix_group_id("metric.", &group.id)),
        GroupType::Metric => group.metric_name.clone().map(SignalId::from),
        GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => None,
    }
}

/// Converts one attribute reference of a v1 group into the v1 definition and
/// the v2 catalog reference.
///
/// A lookup that finds nothing is an error. Without the error, the signal loses
/// the attribute in silence. An entity holds its identity in these attributes.
fn convert_attribute_ref<'a>(
    group_id: &str,
    attr_ref: &crate::attribute::AttributeRef,
    c: &'a crate::catalog::Catalog,
    v2_catalog: &Catalog,
) -> Result<(&'a crate::attribute::Attribute, attribute::AttributeRef), crate::error::Error> {
    let not_found = || crate::error::Error::AttributeNotFound {
        group_id: group_id.to_owned(),
        attr_ref: *attr_ref,
    };
    let attr = c.attribute(attr_ref).ok_or_else(not_found)?;
    let v2_ref = v2_catalog.convert_ref(attr).ok_or_else(not_found)?;
    Ok((attr, v2_ref))
}

/// Converts a V1 registry + catalog to V2.
pub fn convert_v1_to_v2(
    c: crate::catalog::Catalog,
    r: crate::registry::Registry,
    dependencies: BTreeSet<SchemaUrl>,
) -> Result<(Vec<Attribute>, Registry, Refinements, BTreeSet<SchemaUrl>), crate::error::Error> {
    let deps_list: Vec<_> = dependencies.iter().cloned().collect();

    let get_provenance = |g: &crate::registry::Group| -> provenance::Provenance {
        let mut prov = provenance::Provenance::default();
        if let Some(p) = g.provenance() {
            prov.path = p.path.clone();
            if p.schema_url.to_string() != r.registry_url {
                // Note: if idx is not found, it means this came from *ourselves* not from a dependency.
                // In that instance we don't fill out dependency provenance.
                if let Some(idx) = deps_list.iter().position(|u| u == &p.schema_url) {
                    prov.source = Some(provenance::DependencyRef(idx as u32));
                }
            }
        }
        prov
    };

    let attr_provenance = |a: &crate::attribute::Attribute| -> provenance::Provenance {
        // Try to find which group first defined an attribute, using V1 lineage.
        for group in r.groups.iter() {
            if let Some(lineage) = group.lineage.as_ref() {
                if let Some(attr_lineage) = lineage.attribute(&a.name) {
                    if attr_lineage.source_group == group.id {
                        return get_provenance(group);
                    }
                }
            }
        }

        // Fallback: check where it was first defined using the Catalog's root_attribute.
        if let Some((_, source_group_id)) = c.root_attribute(&a.name) {
            // Is it a local group?
            if let Some(group) = r.groups.iter().find(|g| g.id == *source_group_id) {
                return get_provenance(group);
            }
            // Is it a V2 dependency group?
            // See crates/weaver_resolver/src/attribute.rs for more information on this
            // workaround for V2 -> V1 -> V2.
            if let Some(dep_name) = source_group_id.strip_prefix("v2_dependency.") {
                let mut prov = provenance::Provenance::default();
                if let Some(idx) = deps_list.iter().position(|u| u.name() == dep_name) {
                    prov.source = Some(provenance::DependencyRef(idx as u32));
                }
                return prov;
            }
        }

        provenance::Provenance::default()
    };

    // When pulling attributes, as we collapse things, we need to filter
    // to just unique.
    let attributes: HashSet<Attribute> = c
        .attributes()
        .cloned()
        .map(|a| {
            let provenance = attr_provenance(&a);
            Attribute {
                key: a.name,
                r#type: a.r#type,
                examples: a.examples,
                common: CommonFields {
                    brief: a.brief,
                    note: a.note,
                    stability: a.stability.unwrap_or_default(),
                    deprecated: a.deprecated,
                    annotations: a.annotations.unwrap_or_default(),
                },
                provenance,
            }
        })
        .collect();

    let v2_catalog = Catalog::from_attributes(attributes.into_iter().collect());

    // Pull signals from the registry and create a new span-focused registry.
    let mut spans = Vec::new();
    let mut span_refinements = Vec::new();
    let mut metrics = Vec::new();
    let mut metric_refinements = Vec::new();
    let mut events = Vec::new();
    let mut event_refinements = Vec::new();
    let mut entities = Vec::new();
    let mut entity_refinements = Vec::new();
    let mut attribute_groups = Vec::new();

    // Entities come first. A signal names an entity, and the reference records
    // where that entity is defined, so the entities of this registry must be
    // known before any signal is converted.
    for g in r.groups.iter().filter(|g| g.r#type == GroupType::Entity) {
        // Check if we refine another entity.
        let is_refinement = is_refinement_of(g);
        let mut id_attrs = Vec::new();
        let mut desc_attrs = Vec::new();
        for attr_ref in g.attributes.iter() {
            let (attr, base) = convert_attribute_ref(&g.id, attr_ref, &c, &v2_catalog)?;
            let entity_attr = entity::EntityAttributeRef {
                base,
                requirement_level: attr.requirement_level.clone(),
            };
            match attr.role {
                Some(weaver_semconv::attribute::AttributeRole::Identifying) => {
                    id_attrs.push(entity_attr);
                }
                _ => desc_attrs.push(entity_attr),
            }
        }
        let entity_type = if is_refinement {
            let Some(extends_group) = g.lineage.as_ref().and_then(|l| l.extends_group.as_ref())
            else {
                return Err(crate::error::Error::RefinementBaseNotFound {
                    group_id: g.id.clone(),
                });
            };
            // `extends` names the base by group id, and the base declares the
            // type. Read it from the base rather than deriving it a second way.
            r.groups
                .iter()
                .find(|base| &base.id == extends_group)
                .map(entity_type_of)
                .unwrap_or_else(|| fix_group_id("entity.", extends_group))
        } else {
            entity_type_of(g)
        };
        let entity = Entity {
            r#type: entity_type,
            identity: id_attrs,
            description: desc_attrs,
            requirement_level: g.requirement_level.clone(),
            common: CommonFields {
                brief: g.brief.clone(),
                note: g.note.clone(),
                stability: g.stability.clone().unwrap_or_default(),
                deprecated: g.deprecated.clone(),
                annotations: g.annotations.clone().unwrap_or_default(),
            },
            provenance: get_provenance(g),
        };
        if is_refinement {
            entity_refinements.push(entity::EntityRefinement {
                id: fix_group_id("entity.", &g.id),
                entity,
            });
        } else {
            entities.push(entity.clone());
            entity_refinements.push(entity::EntityRefinement {
                id: entity.r#type.clone(),
                entity,
            });
        }
    }

    // An association names an entity type or a refinement id, in the one
    // namespace that `extends` gives them. Read the names back off the
    // refinements: deriving them a second time here would let this map and that
    // list disagree. The value is the registry each entity was declared in,
    // which tells one definition from another that merely shares a name.
    let local_entities: HashMap<SignalId, Option<provenance::DependencyRef>> = entity_refinements
        .iter()
        .map(|refinement| (refinement.id.clone(), refinement.entity.provenance.source))
        .collect();
    let dep_index: HashMap<&SchemaUrl, provenance::DependencyRef> = deps_list
        .iter()
        .enumerate()
        .map(|(index, url)| (url, provenance::DependencyRef(index as u32)))
        .collect();
    let entity_refs = EntityRefResolver {
        local: &local_entities,
        dep_index: &dep_index,
        origins: &r.entity_association_origins,
    };

    for g in r.groups.iter() {
        match g.r#type {
            GroupType::Span => {
                // Check if we extend another span.
                let is_refinement = is_refinement_of(g);
                // Pull all the attribute references.
                let mut span_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        span_attributes.push(span::SpanAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                            sampling_relevant: attr.sampling_relevant,
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                if !is_refinement {
                    let span = Span {
                        r#type: fix_span_group_id(&g.id),
                        kind: g
                            .span_kind
                            .clone()
                            .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                        // Use span_name (carried from v2) if available, fall back to g.name.
                        name: g.span_name.clone().unwrap_or_else(|| SpanName {
                            note: g.name.clone().unwrap_or_default(),
                        }),
                        entity_associations: convert_entity_associations(
                            &g.entity_associations,
                            &entity_refs,
                            &g.id,
                        )?,
                        requirement_level: g.requirement_level.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g.stability.clone().unwrap_or_default(),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        attributes: span_attributes,
                        provenance: get_provenance(g),
                    };
                    spans.push(span.clone());
                    span_refinements.push(SpanRefinement {
                        id: span.r#type.clone(),
                        span,
                    });
                } else {
                    let Some(extends_group) =
                        g.lineage.as_ref().and_then(|l| l.extends_group.as_ref())
                    else {
                        return Err(crate::error::Error::RefinementBaseNotFound {
                            group_id: g.id.clone(),
                        });
                    };
                    let span_type = fix_span_group_id(extends_group);
                    span_refinements.push(SpanRefinement {
                        id: fix_span_group_id(&g.id),
                        span: Span {
                            r#type: span_type,
                            kind: g
                                .span_kind
                                .clone()
                                .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                            // Use span_name (carried from v2) if available, fall back to g.name.
                            name: g.span_name.clone().unwrap_or_else(|| SpanName {
                                note: g.name.clone().unwrap_or_default(),
                            }),
                            entity_associations: convert_entity_associations(
                                &g.entity_associations,
                                &entity_refs,
                                &g.id,
                            )?,
                            requirement_level: g.requirement_level.clone(),
                            common: CommonFields {
                                brief: g.brief.clone(),
                                note: g.note.clone(),
                                stability: g.stability.clone().unwrap_or_default(),
                                deprecated: g.deprecated.clone(),
                                annotations: g.annotations.clone().unwrap_or_default(),
                            },
                            attributes: span_attributes,
                            provenance: get_provenance(g),
                        },
                    });
                }
            }
            GroupType::Event => {
                let is_refinement = is_refinement_of(g);
                let mut event_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        event_attributes.push(event::EventAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                // We cannot convert older repositories before event name was required.
                if let Some(name) = g.name.clone() {
                    let event = event::Event {
                        name: name.into(),
                        attributes: event_attributes,
                        entity_associations: convert_entity_associations(
                            &g.entity_associations,
                            &entity_refs,
                            &g.id,
                        )?,
                        requirement_level: g.requirement_level.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g.stability.clone().unwrap_or_default(),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        provenance: get_provenance(g),
                    };
                    if !is_refinement {
                        events.push(event.clone());
                        event_refinements.push(event::EventRefinement {
                            id: event.name.clone(),
                            event,
                        });
                    } else {
                        event_refinements.push(event::EventRefinement {
                            id: fix_group_id("event.", &g.id),
                            event,
                        });
                    }
                } else {
                    // We have no event name
                    return Err(crate::error::Error::EventNameNotFound {
                        group_id: g.id.clone(),
                    });
                }
            }
            GroupType::Metric => {
                // Check if we extend another metric.
                let is_refinement = is_refinement_of(g);
                let mut metric_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        metric_attributes.push(metric::MetricAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                // TODO - deal with unwrap errors.
                let metric = Metric {
                    name: g
                        .metric_name
                        .clone()
                        .expect("metric_name must exist on metrics prior to translation to v2")
                        .into(),
                    instrument: g
                        .instrument
                        .clone()
                        .expect("instrument must exist on metrics prior to translation to v2"),
                    unit: g
                        .unit
                        .clone()
                        .expect("unit must exist on metrics prior to translation to v2"),
                    attributes: metric_attributes,
                    entity_associations: convert_entity_associations(
                        &g.entity_associations,
                        &entity_refs,
                        &g.id,
                    )?,
                    requirement_level: g.requirement_level.clone(),
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g.stability.clone().unwrap_or_default(),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                    provenance: get_provenance(g),
                };
                if is_refinement {
                    metric_refinements.push(metric::MetricRefinement {
                        id: fix_group_id("metric.", &g.id),
                        metric,
                    });
                } else {
                    metrics.push(metric.clone());
                    metric_refinements.push(metric::MetricRefinement {
                        id: metric.name.clone(),
                        metric,
                    });
                }
            }
            GroupType::Entity => {
                // Converted by the pass above.
            }
            GroupType::AttributeGroup => {
                if g.visibility
                    .as_ref()
                    .is_some_and(|v| AttributeGroupVisibilitySpec::Public == *v)
                {
                    // Now we need to convert the group.
                    let mut attributes = Vec::new();
                    // TODO - we need to check lineage and remove parent groups.
                    for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                        if let Some(a) = v2_catalog.convert_ref(attr) {
                            attributes.push(attribute_group::AttributeGroupAttributeRef {
                                base: a,
                                requirement_level: attr.requirement_level.clone(),
                            });
                        } else {
                            // TODO logic error!
                        }
                    }
                    attribute_groups.push(AttributeGroup {
                        id: fix_group_id("attribute_group.", &g.id),
                        attributes,
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g.stability.clone().unwrap_or_default(),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        provenance: get_provenance(g),
                    });
                }
            }
            GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => {
                // Ignored for now, we should probably issue warnings.
            }
        }
    }

    // Now we need to hunt for attribute definitions
    let mut attributes = Vec::new();
    for g in r.groups.iter() {
        for a in g.attributes.iter() {
            if let Some(attr) = c.attribute(a) {
                // Attribute definitions do not have lineage.
                let is_def = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.attribute(&attr.name))
                    .is_none();
                if is_def {
                    if let Some(v2) = v2_catalog.convert_ref(attr) {
                        attributes.push(v2);
                    } else {
                        // TODO logic error!
                    }
                }
            }
        }
    }
    attributes.sort_by_key(|a| a.0);
    attributes.dedup();

    let v2_registry = Registry {
        attributes,
        spans,
        metrics,
        events,
        entities,
        attribute_groups,
    };
    let v2_refinements = Refinements {
        spans: span_refinements,
        metrics: metric_refinements,
        events: event_refinements,
        entities: entity_refinements,
    };
    Ok((v2_catalog.into(), v2_registry, v2_refinements, dependencies))
}

/// A trait that defines a signal, used for performing "diff"
pub trait Signal {
    /// The id of the signal.
    fn id(&self) -> &str;
    /// The common fields for the signal.
    fn common(&self) -> &CommonFields;
}

/// Diffs signal registries.
#[must_use]
fn diff_signals<T: Signal>(latest: &[T], baseline: &[T]) -> Vec<SchemaItemChange> {
    let baseline_signals: HashMap<&str, &T> = baseline.iter().map(|s| (s.id(), s)).collect();
    let latest_signals: HashMap<&str, &T> = latest.iter().map(|s| (s.id(), s)).collect();
    diff_signals_by_hash(&latest_signals, &baseline_signals)
}

/// Finds the difference between two signal registries using a hash into the signal id.
fn diff_signals_by_hash<T: Signal>(
    latest: &HashMap<&str, &T>,
    baseline: &HashMap<&str, &T>,
) -> Vec<SchemaItemChange> {
    let mut changes: Vec<SchemaItemChange> = Vec::new();
    for (&signal_id, latest_signal) in latest.iter() {
        let baseline_signal = baseline.get(signal_id);
        if let Some(baseline_signal) = baseline_signal {
            if let Some(deprecated) = latest_signal.common().deprecated.as_ref() {
                // is this a change from the baseline?
                if let Some(baseline_deprecated) = baseline_signal.common().deprecated.as_ref() {
                    if deprecated == baseline_deprecated {
                        continue;
                    }
                }

                match deprecated {
                    Deprecated::Renamed {
                        renamed_to: rename_to,
                        ..
                    } => {
                        changes.push(SchemaItemChange::Renamed {
                            old_name: signal_id.to_owned(),
                            new_name: rename_to.clone(),
                            note: deprecated.note(),
                        });
                    }
                    Deprecated::Obsoleted { note } => {
                        changes.push(SchemaItemChange::Obsoleted {
                            name: signal_id.to_owned(),
                            note: note.clone(),
                        });
                    }
                    Deprecated::Unspecified { note } | Deprecated::Uncategorized { note } => {
                        changes.push(SchemaItemChange::Uncategorized {
                            name: signal_id.to_owned(),
                            note: note.clone(),
                        });
                    }
                }
            }
        } else {
            changes.push(SchemaItemChange::Added {
                name: signal_id.to_owned(),
            });
        }
    }
    // Any signal in the baseline schema that is not present in the latest schema
    // is considered removed.
    // Note: This should never occur if the registry evolution process is followed.
    // However, detecting this case is useful for identifying a violation of the process.
    for (signal_name, _) in baseline.iter() {
        if !latest.contains_key(signal_name) {
            changes.push(SchemaItemChange::Removed {
                name: (*signal_name).to_owned(),
            });
        }
    }
    changes
}

#[cfg(test)]
mod tests {

    use crate::v2::attribute::{Attribute as AttributeV2, AttributeRef};
    use crate::v2::event::Event;
    use crate::V1_RESOLVED_FILE_FORMAT;
    use crate::{attribute::Attribute, lineage::GroupLineage, registry::Group};
    use std::collections::BTreeMap;
    use weaver_semconv::{provenance::Provenance, stability::Stability};

    use crate::lineage::AttributeLineage;

    use super::*;

    #[test]
    fn test_convert_span_v1_to_v2() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let ref1 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let test_refs = [ref0, ref1];
        let v1_catalog = builder.build();
        let mut refinement_span_lineage =
            GroupLineage::new(Provenance::new(SchemaUrl::new_unknown(), "tmp"));
        refinement_span_lineage.extends("span.my-span", GroupType::Span);
        refinement_span_lineage
            .add_attribute_lineage("test.key".to_owned(), AttributeLineage::new("span.my-span"));
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![
                Group {
                    id: "span.my-span".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    requirement_level: None,
                    name: Some("my span name".to_owned()),
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                    is_v2: false,
                    span_name: None,
                },
                Group {
                    id: "span.custom".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    requirement_level: None,
                    name: Some("my span name".to_owned()),
                    lineage: Some(refinement_span_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                    is_v2: false,
                    span_name: None,
                },
            ],
        };
        let dependencies = BTreeSet::new();
        let (catalog, v2_registry, v2_refinements, _) =
            convert_v1_to_v2(v1_catalog, v1_registry, dependencies)
                .expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(catalog.len(), 1);
        // Assert one attribute shows up, due to lineage.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.spans.len(), 1);
        if let Some(span) = v2_registry.spans.first() {
            assert_eq!(span.r#type, "my-span".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.spans.len(), 2);
        let span_ref_ids: Vec<String> = v2_refinements
            .spans
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            span_ref_ids,
            vec!["my-span".to_owned(), "custom".to_owned()]
        );
    }

    /// A refinement that carries `span_name` (set from a v2 `name`
    /// override or inherited from its base) must surface it as the resolved
    /// span's `name.note`, rather than falling back to the refinement id.
    #[test]
    fn test_span_refinement_name_note_propagates() {
        let v1_catalog = crate::catalog::test_utils::CatalogBuilder::default().build();
        // Helper to build a minimal v1 span Group for this test.
        let span_group =
            |id: &str, lineage: Option<GroupLineage>, span_name: Option<SpanName>| Group {
                id: id.to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![],
                span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some(id.to_owned()),
                lineage,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name,
            };
        let mut refinement_lineage =
            GroupLineage::new(Provenance::new(SchemaUrl::new_unknown(), "tmp"));
        refinement_lineage.extends("span.my-span", GroupType::Span);
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![
                // The base span the refinement points at; its presence (as a
                // span) is what makes the refinement be recognized as a
                // refinement rather than a standalone span.
                span_group(
                    "span.my-span",
                    None,
                    Some(SpanName {
                        note: "base note".to_owned(),
                    }),
                ),
                // The refinement, carrying its own `name` override.
                span_group(
                    "span.custom",
                    Some(refinement_lineage),
                    Some(SpanName {
                        note: "{gen_ai.operation.name} {gen_ai.request.model}".to_owned(),
                    }),
                ),
            ],
        };
        let (_, _, v2_refinements, _) = convert_v1_to_v2(v1_catalog, v1_registry, BTreeSet::new())
            .expect("Failed to convert v1 to v2");
        let refinement = v2_refinements
            .spans
            .iter()
            .find(|s| s.id.to_string() == "custom")
            .expect("expected the `custom` span refinement");
        // Confirm this went through the refinement branch (it refines
        // `my-span`), not the standalone-span branch.
        assert_eq!(refinement.span.r#type.to_string(), "my-span");
        assert_eq!(
            refinement.span.name.note,
            "{gen_ai.operation.name} {gen_ai.request.model}"
        );
    }

    #[test]
    fn test_convert_metric_v1_to_v2() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let ref1 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let test_refs = [ref0, ref1];
        let v1_catalog = builder.build();
        let mut refinement_metric_lineage =
            GroupLineage::new(Provenance::new(SchemaUrl::new_unknown(), "tmp"));
        refinement_metric_lineage.extends("metric.http", GroupType::Metric);
        refinement_metric_lineage
            .add_attribute_lineage("test.key".to_owned(), AttributeLineage::new("metric.http"));
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![
                Group {
                    id: "metric.http".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[0]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    requirement_level: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                    is_v2: false,
                    span_name: None,
                },
                Group {
                    id: "metric.http.custom".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    requirement_level: None,
                    name: None,
                    lineage: Some(refinement_metric_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                    is_v2: false,
                    span_name: None,
                },
            ],
        };
        let dependencies = BTreeSet::new();
        let (_, v2_registry, v2_refinements, _) =
            convert_v1_to_v2(v1_catalog, v1_registry, dependencies)
                .expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.metrics.len(), 1);
        if let Some(metric) = v2_registry.metrics.first() {
            assert_eq!(metric.name, "http".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.metrics.len(), 2);
        let metric_ref_ids: Vec<String> = v2_refinements
            .metrics
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            metric_ref_ids,
            vec!["http".to_owned(), "http.custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_event_v1_to_v2() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let test_refs = [ref0];
        let v1_catalog = builder.build();
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![Group {
                id: "event.my-event".to_owned(),
                r#type: GroupType::Event,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some("my-event".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name: None,
            }],
        };
        let dependencies = BTreeSet::new();

        let (_, v2_registry, _, _) = convert_v1_to_v2(v1_catalog, v1_registry, dependencies)
            .expect("Failed to convert v1 to v2");
        assert_eq!(v2_registry.events.len(), 1);
        if let Some(event) = v2_registry.events.first() {
            assert_eq!(event.name, "my-event".to_owned().into());
        }
    }

    #[test]
    fn test_convert_entity_v1_to_v2() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: Some(weaver_semconv::attribute::AttributeRole::Identifying),
            },
            None,
        );
        let test_refs = [ref0];
        let v1_catalog = builder.build();
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![Group {
                id: "entity.my-entity".to_owned(),
                r#type: GroupType::Entity,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some("my-entity".to_owned()),
                lineage: Some(GroupLineage::new(Provenance::new(
                    "https://my.dependency.url/1.0.0".try_into().unwrap(),
                    "/path/to/source.yaml",
                ))),
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name: None,
            }],
        };
        let mut dependencies = BTreeSet::new();
        let _ = dependencies.insert("https://my.dependency.url/1.0.0".try_into().unwrap());
        let (_, v2_registry, _, deps_out) = convert_v1_to_v2(v1_catalog, v1_registry, dependencies)
            .expect("Failed to convert v1 to v2");
        assert_eq!(deps_out.len(), 1);
        assert!(deps_out.contains(&"https://my.dependency.url/1.0.0".try_into().unwrap()));
        assert_eq!(v2_registry.entities.len(), 1);
        if let Some(entity) = v2_registry.entities.first() {
            assert_eq!(entity.r#type, "my-entity".to_owned().into());
            assert_eq!(entity.identity.len(), 1);
            assert_eq!(entity.provenance.source, Some(provenance::DependencyRef(0)));
            assert_eq!(entity.provenance.path, "/path/to/source.yaml");
        }
    }

    /// An association leaf records where the entity is defined.
    ///
    /// A name that an entity group of this registry answers to carries no
    /// provenance. A name that only a dependency defines has no group here, so
    /// the resolver recorded its origin, and the leaf carries the index of that
    /// dependency. A name that neither holds is an error.
    #[test]
    fn test_convert_entity_associations_record_where_they_resolved() {
        use weaver_semconv::entity_association::EntityAssociation as SpecAssociation;

        let dep_url: SchemaUrl = "https://my.dependency.url/1.0.0"
            .try_into()
            .expect("valid schema url");
        let metric_associations = |associations: Vec<SpecAssociation>| Group {
            id: "metric.my-metric".to_owned(),
            r#type: GroupType::Metric,
            brief: "".to_owned(),
            note: "".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            events: vec![],
            metric_name: Some("my.metric".to_owned()),
            instrument: Some(weaver_semconv::group::InstrumentSpec::Counter),
            unit: Some("1".to_owned()),
            requirement_level: None,
            name: None,
            lineage: None,
            display_name: None,
            body: None,
            annotations: None,
            entity_associations: associations,
            visibility: None,
            is_v2: true,
            span_name: None,
        };
        let local_entity = Group {
            id: "entity.service".to_owned(),
            r#type: GroupType::Entity,
            brief: "".to_owned(),
            note: "".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            requirement_level: None,
            name: Some("service".to_owned()),
            lineage: None,
            display_name: None,
            body: None,
            annotations: None,
            entity_associations: vec![],
            visibility: None,
            is_v2: true,
            span_name: None,
        };
        let registry = |groups: Vec<Group>, origins: crate::registry::EntityAssociationOrigins| {
            crate::registry::Registry {
                registry_url: "my.schema.url".to_owned(),
                entity_association_origins: origins,
                groups,
            }
        };
        let mut dependencies = BTreeSet::new();
        let _ = dependencies.insert(dep_url.clone());
        let mut group_origins = BTreeMap::new();
        let _ = group_origins.insert("host".to_owned(), dep_url);
        let mut origins = BTreeMap::new();
        let _ = origins.insert("metric.my-metric".to_owned(), group_origins);

        let associations = vec![SpecAssociation::AllOf {
            all_of: vec![
                SpecAssociation::Ref("service".to_owned()),
                SpecAssociation::Ref("host".to_owned()),
            ],
        }];
        let (_, v2_registry, _, _) = convert_v1_to_v2(
            crate::catalog::Catalog::default(),
            registry(
                vec![local_entity.clone(), metric_associations(associations)],
                origins,
            ),
            dependencies.clone(),
        )
        .expect("Failed to convert v1 to v2");

        let [metric] = v2_registry.metrics.as_slice() else {
            panic!("expected one metric, got {:?}", v2_registry.metrics);
        };
        assert_eq!(
            metric.entity_associations,
            vec![entity::EntityAssociation::AllOf {
                all_of: vec![
                    entity::EntityAssociation::Ref(entity::EntityRef::local(
                        "service".to_owned().into()
                    )),
                    entity::EntityAssociation::Ref(entity::EntityRef {
                        r#type: "host".to_owned().into(),
                        provenance: provenance::Provenance {
                            source: Some(provenance::DependencyRef(0)),
                            path: String::new(),
                        },
                    }),
                ],
            }],
            "the tree keeps its shape, and each leaf says where it resolved"
        );

        // Nothing recorded an origin for `host` this time, and no group answers
        // to the name.
        let error = convert_v1_to_v2(
            crate::catalog::Catalog::default(),
            registry(
                vec![
                    local_entity,
                    metric_associations(vec![SpecAssociation::Ref("host".to_owned())]),
                ],
                BTreeMap::new(),
            ),
            dependencies,
        )
        .expect_err("an association that nothing defines must fail the conversion");
        assert!(
            matches!(
                error,
                crate::error::Error::EntityAssociationNotFound { ref entity_type, .. }
                    if entity_type == "host"
            ),
            "unexpected error: {error:?}"
        );
    }

    /// Every signal keeps an attribute that has no stability, and not only an
    /// entity. The catalog lookup is common to all of them.
    #[test]
    fn test_convert_span_keeps_attribute_without_stability() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: None,
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let v1_catalog = builder.build();
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![Group {
                id: "span.my-span".to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![ref0],
                span_kind: Some(weaver_semconv::group::SpanKindSpec::Internal),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some("my-span".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name: None,
            }],
        };
        let (_, v2_registry, _, _) =
            convert_v1_to_v2(v1_catalog, v1_registry, BTreeSet::new()).expect("conversion failed");
        let span = v2_registry.spans.first().expect("span present");
        assert_eq!(span.attributes.len(), 1, "span lost the attribute");
    }

    /// An entity must keep an attribute that has no stability. A registry with
    /// such an attribute resolves, because a missing stability is only a
    /// warning. An entity that loses its identity attribute has no identity.
    #[test]
    fn test_convert_entity_keeps_attribute_without_stability() {
        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                // The point of this test.
                stability: None,
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: Some(weaver_semconv::attribute::AttributeRole::Identifying),
            },
            None,
        );
        let v1_catalog = builder.build();
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![Group {
                id: "entity.my-entity".to_owned(),
                r#type: GroupType::Entity,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![ref0],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: Some("my-entity".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name: None,
            }],
        };
        let (_, v2_registry, _, _) =
            convert_v1_to_v2(v1_catalog, v1_registry, BTreeSet::new()).expect("conversion failed");
        let entity = v2_registry
            .entities
            .first()
            .expect("the entity is in the v2 registry");
        assert_eq!(
            entity.identity.len(),
            1,
            "an identity attribute with no stability must not be dropped"
        );
    }

    #[test]
    fn test_convert_public_attribute_group_carries_requirement_level() {
        use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};

        let mut builder = crate::catalog::test_utils::CatalogBuilder::default();
        // The catalog attribute carries the requirement level that was
        // resolved from the attribute ref refinement in the public group.
        let ref0 = builder.add(
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            None,
        );
        let v1_catalog = builder.build();
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![Group {
                id: "test.group".to_owned(),
                r#type: GroupType::AttributeGroup,
                brief: "a public group".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![ref0],
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
                annotations: None,
                entity_associations: vec![],
                visibility: Some(AttributeGroupVisibilitySpec::Public),
                is_v2: true,
                span_name: None,
            }],
        };
        let (_, v2_registry, _, _) = convert_v1_to_v2(v1_catalog, v1_registry, BTreeSet::new())
            .expect("Failed to convert v1 to v2");
        // The public attribute group is emitted...
        assert_eq!(v2_registry.attribute_groups.len(), 1);
        let group = &v2_registry.attribute_groups[0];
        assert_eq!(group.id, "test.group".to_owned().into());
        // ...and its attribute ref carries the group-specific requirement level.
        assert_eq!(group.attributes.len(), 1);
        assert_eq!(
            group.attributes[0].requirement_level,
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
        );
    }

    #[test]
    fn test_try_from_v1_to_v2() {
        let mut dependencies = BTreeSet::new();
        let _ = dependencies.insert("http://dependency/url/1.0.0".try_into().unwrap());

        let v1_schema = crate::ResolvedTelemetrySchema {
            file_format: V1_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "my-registry".to_owned(),
            catalog: crate::catalog::Catalog::default(),
            registry: crate::registry::Registry {
                registry_url: "http://another/url/1.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            instrumentation_library: None,
            resource: None,
            dependencies: dependencies.clone(),
            versions: None,
            registry_manifest: None,
        };

        let v2_schema: Result<ResolvedTelemetrySchema, _> = v1_schema.try_into();
        assert!(v2_schema.is_ok());
        let v2_schema = v2_schema.unwrap();
        assert_eq!(v2_schema.file_format, V2_RESOLVED_FILE_FORMAT);
        assert_eq!(
            v2_schema.schema_url,
            "http://test/schemas/1.0.0".try_into().unwrap()
        );
        assert_eq!(v2_schema.dependencies, dependencies);
    }

    #[test]
    fn no_diff() {
        let mut baseline = empty_v2_schema();
        baseline.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        baseline.registry.attributes.push(AttributeRef(0));
        let changes = baseline.diff(&baseline);
        assert!(changes.is_empty());
    }

    #[test]
    fn attribute_diff() {
        let mut baseline = empty_v2_schema();
        baseline.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        baseline.registry.attributes.push(AttributeRef(0));
        let mut latest = empty_v2_schema();
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "test.key.new".to_owned(),
                    note: Some("hated it".to_owned()),
                }),
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key.new".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        latest.registry.attributes.push(AttributeRef(0));
        latest.registry.attributes.push(AttributeRef(1));
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for attr_change in diff.registry.attribute_changes.iter() {
            match attr_change {
                SchemaItemChange::Renamed {
                    old_name,
                    new_name,
                    note,
                } => {
                    assert_eq!(old_name, "test.key");
                    assert_eq!(new_name, "test.key.new");
                    assert_eq!(note, "hated it");
                }
                SchemaItemChange::Added { name } => {
                    assert_eq!(name, "test.key.new");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_metric_removed() {
        // Test a user changing a metric name but not using deprecated field.
        let mut baseline = empty_v2_schema();
        baseline.registry.metrics.push(Metric {
            name: "http".to_owned().into(),
            instrument: weaver_semconv::group::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.metrics.push(Metric {
            name: "http.renamed".to_owned().into(),
            instrument: weaver_semconv::group::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Added { name } => {
                    assert_eq!(name, "http.renamed");
                }
                SchemaItemChange::Removed { name } => {
                    assert_eq!(name, "http");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_entity_uncategorized_deprecation() {
        // Test a user deprecating an entity with unknown change type.
        let mut baseline = empty_v2_schema();
        baseline.registry.entities.push(Entity {
            common: CommonFields::default(),
            r#type: "test.entity".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.entities.push(Entity {
            common: CommonFields {
                deprecated: Some(Deprecated::Uncategorized {
                    note: "note".to_owned(),
                }),
                ..Default::default()
            },
            r#type: "test.entity".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Uncategorized { name, note } => {
                    assert_eq!(name, "test.entity");
                    assert_eq!(note, "note");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_event_obsoleted() {
        // Test a user obsoleting an event.
        let mut baseline = empty_v2_schema();
        baseline.registry.events.push(Event {
            common: CommonFields::default(),
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.events.push(Event {
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                deprecated: Some(Deprecated::Obsoleted {
                    note: "note".to_owned(),
                }),
                ..Default::default()
            },
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Obsoleted { name, note } => {
                    assert_eq!(name, "test.event");
                    assert_eq!(note, "note");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    // create an empty schema for testing.
    fn empty_v2_schema() -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0"
                .try_into()
                .expect("Should be valid schema url"),
            attribute_catalog: vec![],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: BTreeSet::new(),
        }
    }
}
