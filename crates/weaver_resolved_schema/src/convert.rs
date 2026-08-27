// SPDX-License-Identifier: Apache-2.0

//! Conversions from V1 to V2 Resolved Telemetry Schema.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::group::GroupType;
use weaver_semconv::v2::{signal_id::SignalId, span::SpanName, CommonFields};

use crate::v1::{
    attribute::{Attribute as V1Attribute, AttributeRef as V1AttributeRef},
    catalog::Catalog as V1Catalog,
    registry::{Group as V1Group, Registry as V1Registry},
    ResolvedTelemetrySchema as V1ResolvedSchema,
};
use crate::v2::{
    attribute::{Attribute as V2Attribute, AttributeRef as V2AttributeRef},
    attribute_group::{self, AttributeGroup as V2AttributeGroup},
    entity::{self, Entity as V2Entity},
    event::{self, Event as V2Event},
    metric::{self, Metric as V2Metric},
    provenance::{self, Provenance as V2Provenance},
    refinements::Refinements as V2Refinements,
    registry::Registry as V2Registry,
    span::{self, Span as V2Span, SpanRefinement as V2SpanRefinement},
    ResolvedTelemetrySchema as V2ResolvedSchema,
};
use crate::v2::V2_RESOLVED_FILE_FORMAT;

/// Temporary catalog used to index V2 attributes and map V1 attributes to V2 AttributeRefs.
#[derive(Debug, Clone, Default)]
struct V2CatalogBuilder {
    attributes: Vec<V2Attribute>,
    lookup: BTreeMap<String, Vec<usize>>,
}

impl From<V2CatalogBuilder> for Vec<V2Attribute> {
    fn from(val: V2CatalogBuilder) -> Self {
        val.attributes
    }
}

impl V2CatalogBuilder {
    fn from_attributes(mut attributes: Vec<V2Attribute>) -> Self {
        attributes.sort_by_cached_key(|attr| {
            (attr.key.clone(), {
                let mut s = DefaultHasher::new();
                attr.hash(&mut s);
                s.finish()
            })
        });
        let mut lookup: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, attr) in attributes.iter().enumerate() {
            lookup.entry(attr.key.clone()).or_default().push(idx);
        }
        Self { attributes, lookup }
    }

    fn convert_ref(&self, attribute: &V1Attribute) -> Option<V2AttributeRef> {
        let v2_type = weaver_semconv::convert::v1_attribute_type_to_v2(attribute.r#type.clone());
        let v2_examples = attribute.examples.clone().map(weaver_semconv::convert::v1_examples_to_v2);

        self.lookup.get(&attribute.name)?.iter().find_map(|idx| {
            self.attributes
                .get(*idx)
                .filter(|a| {
                    a.key == attribute.name
                        && a.r#type == v2_type
                        && a.examples == v2_examples
                        && a.common.brief == attribute.brief
                        && a.common.note == attribute.note
                        && a.common.deprecated == attribute.deprecated
                        && a.common.stability
                            == *attribute
                                .stability
                                .as_ref()
                                .unwrap_or(&weaver_semconv::stability::Stability::default())
                        && attribute
                            .annotations
                            .as_ref()
                            .map(|ans| a.common.annotations == *ans)
                            .unwrap_or(a.common.annotations.is_empty())
                })
                .map(|_| V2AttributeRef(*idx as u32))
        })
    }
}

/// Easy conversion from v1 to v2.
impl TryFrom<V1ResolvedSchema> for V2ResolvedSchema {
    type Error = crate::error::Error;
    fn try_from(value: V1ResolvedSchema) -> Result<Self, Self::Error> {
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

        Ok(V2ResolvedSchema {
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
    local: &'a HashMap<SignalId, Option<provenance::DependencyRef>>,
    dep_index: &'a HashMap<&'a SchemaUrl, provenance::DependencyRef>,
    origins: &'a crate::v1::registry::EntityAssociationOrigins,
}

impl EntityRefResolver<'_> {
    fn resolve(&self, group_id: &str, name: &str) -> Option<entity::EntityRef> {
        let Some(origin) = self.origins.get(group_id).and_then(|g| g.get(name)) else {
            let name = SignalId::from(name.to_owned());
            return self
                .local
                .contains_key(&name)
                .then(|| entity::EntityRef::local(name));
        };
        let source = self.dep_index.get(origin).copied();
        if source.is_none() {
            log::warn!(
                "Logic failure - entity `{name}` resolved to `{origin}`, which is not a dependency"
            );
        }
        let name = SignalId::from(name.to_owned());
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

fn is_refinement_of(group: &V1Group) -> bool {
    group
        .lineage
        .as_ref()
        .is_some_and(|l| l.extends_group_type.as_ref() == Some(&group.r#type))
}

fn entity_type_of(group: &V1Group) -> SignalId {
    group
        .name
        .clone()
        .map(SignalId::from)
        .unwrap_or_else(|| fix_group_id("entity.", &group.id))
}

/// The id that a v1 group takes in the v2 namespace of its signal type.
#[must_use]
pub fn v2_namespace_id(group: &V1Group) -> Option<SignalId> {
    let refines = is_refinement_of(group);
    match group.r#type {
        GroupType::Span => Some(fix_span_group_id(&group.id)),
        GroupType::AttributeGroup => Some(fix_group_id("attribute_group.", &group.id)),
        GroupType::Entity if refines => Some(fix_group_id("entity.", &group.id)),
        GroupType::Entity => Some(entity_type_of(group)),
        GroupType::Event if refines => Some(fix_group_id("event.", &group.id)),
        GroupType::Event => group.name.clone().map(SignalId::from),
        GroupType::Metric if refines => Some(fix_group_id("metric.", &group.id)),
        GroupType::Metric => group.metric_name.clone().map(SignalId::from),
        GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => None,
    }
}

fn convert_attribute_ref<'a>(
    group_id: &str,
    attr_ref: &V1AttributeRef,
    c: &'a V1Catalog,
    v2_catalog: &V2CatalogBuilder,
) -> Result<(&'a V1Attribute, V2AttributeRef), crate::error::Error> {
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
    c: V1Catalog,
    r: V1Registry,
    dependencies: BTreeSet<SchemaUrl>,
) -> Result<(Vec<V2Attribute>, V2Registry, V2Refinements, BTreeSet<SchemaUrl>), crate::error::Error> {
    let deps_list: Vec<_> = dependencies.iter().cloned().collect();

    let get_provenance = |g: &V1Group| -> V2Provenance {
        let mut prov = V2Provenance::default();
        if let Some(p) = g.provenance() {
            prov.path = p.path.clone();
            if p.schema_url.to_string() != r.registry_url {
                if let Some(idx) = deps_list.iter().position(|u| u == &p.schema_url) {
                    prov.source = Some(provenance::DependencyRef(idx as u32));
                }
            }
        }
        prov
    };

    let attr_provenance = |a: &V1Attribute| -> V2Provenance {
        for group in r.groups.iter() {
            if let Some(lineage) = group.lineage.as_ref() {
                if let Some(attr_lineage) = lineage.attribute(&a.name) {
                    if attr_lineage.source_group == group.id {
                        return get_provenance(group);
                    }
                }
            }
        }

        if let Some((_, source_group_id)) = c.root_attribute(&a.name) {
            if let Some(group) = r.groups.iter().find(|g| g.id == *source_group_id) {
                return get_provenance(group);
            }
            if let Some(dep_name) = source_group_id.strip_prefix("v2_dependency.") {
                let mut prov = V2Provenance::default();
                if let Some(idx) = deps_list.iter().position(|u| u.name() == dep_name) {
                    prov.source = Some(provenance::DependencyRef(idx as u32));
                }
                return prov;
            }
        }

        V2Provenance::default()
    };

    let attributes: HashSet<V2Attribute> = c
        .attributes()
        .cloned()
        .map(|a| {
            let provenance = attr_provenance(&a);
            let r#type = weaver_semconv::convert::v1_attribute_type_to_v2(a.r#type);
            let examples = a.examples.map(weaver_semconv::convert::v1_examples_to_v2);
            V2Attribute {
                key: a.name,
                r#type,
                examples,
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

    let v2_catalog = V2CatalogBuilder::from_attributes(attributes.into_iter().collect());

    let mut spans = Vec::new();
    let mut span_refinements = Vec::new();
    let mut metrics = Vec::new();
    let mut metric_refinements = Vec::new();
    let mut events = Vec::new();
    let mut event_refinements = Vec::new();
    let mut entities = Vec::new();
    let mut entity_refinements = Vec::new();
    let mut attribute_groups = Vec::new();

    for g in r.groups.iter().filter(|g| g.r#type == GroupType::Entity) {
        let is_refinement = is_refinement_of(g);
        let mut id_attrs = Vec::new();
        let mut desc_attrs = Vec::new();
        for attr_ref in g.attributes.iter() {
            let (attr, base) = convert_attribute_ref(&g.id, attr_ref, &c, &v2_catalog)?;
            let req_level = weaver_semconv::convert::v1_requirement_level_to_v2(attr.requirement_level.clone());
            let entity_attr = entity::EntityAttributeRef {
                base,
                requirement_level: req_level,
            };
            match attr.role {
                Some(weaver_semconv::v1::attribute::AttributeRole::Identifying) => {
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
            r.groups
                .iter()
                .find(|base| &base.id == extends_group)
                .map(entity_type_of)
                .unwrap_or_else(|| fix_group_id("entity.", extends_group))
        } else {
            entity_type_of(g)
        };
        let entity = V2Entity {
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
                let is_refinement = is_refinement_of(g);
                let mut span_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        let req_level = weaver_semconv::convert::v1_requirement_level_to_v2(attr.requirement_level.clone());
                        span_attributes.push(span::SpanAttributeRef {
                            base: a,
                            requirement_level: req_level,
                            sampling_relevant: attr.sampling_relevant,
                        });
                    } else {
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                let span_kind = g
                    .span_kind
                    .clone()
                    .map(weaver_semconv::convert::v1_span_kind_to_v2)
                    .unwrap_or(weaver_semconv::v2::span::SpanKindSpec::Internal);
                let span_name = g
                    .span_name
                    .clone()
                    .map(weaver_semconv::convert::v1_span_name_to_v2)
                    .unwrap_or_else(|| SpanName {
                        note: g.name.clone().unwrap_or_default(),
                    });
                if !is_refinement {
                    let span = V2Span {
                        r#type: fix_span_group_id(&g.id),
                        kind: span_kind,
                        name: span_name,
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
                    span_refinements.push(V2SpanRefinement {
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
                    span_refinements.push(V2SpanRefinement {
                        id: fix_span_group_id(&g.id),
                        span: V2Span {
                            r#type: span_type,
                            kind: span_kind,
                            name: span_name,
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
                        let req_level = weaver_semconv::convert::v1_requirement_level_to_v2(attr.requirement_level.clone());
                        event_attributes.push(event::EventAttributeRef {
                            base: a,
                            requirement_level: req_level,
                        });
                    } else {
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                if let Some(name) = g.name.clone() {
                    let event = V2Event {
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
                    return Err(crate::error::Error::EventNameNotFound {
                        group_id: g.id.clone(),
                    });
                }
            }
            GroupType::Metric => {
                let is_refinement = is_refinement_of(g);
                let mut metric_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        let req_level = weaver_semconv::convert::v1_requirement_level_to_v2(attr.requirement_level.clone());
                        metric_attributes.push(metric::MetricAttributeRef {
                            base: a,
                            requirement_level: req_level,
                        });
                    } else {
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                let instrument = g
                    .instrument
                    .clone()
                    .map(weaver_semconv::convert::v1_instrument_to_v2)
                    .expect("instrument must exist on metrics prior to translation to v2");
                let metric = V2Metric {
                    name: g
                        .metric_name
                        .clone()
                        .expect("metric_name must exist on metrics prior to translation to v2")
                        .into(),
                    instrument,
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
            GroupType::Entity => {}
            GroupType::AttributeGroup => {
                let is_public = g.visibility.as_ref().is_some_and(|v| match v {
                    weaver_semconv::v1::group::AttributeGroupVisibilitySpec::Public => true,
                    weaver_semconv::v1::group::AttributeGroupVisibilitySpec::Internal => false,
                });
                if is_public {
                    let mut attributes = Vec::new();
                    for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                        if let Some(a) = v2_catalog.convert_ref(attr) {
                            let req_level = weaver_semconv::convert::v1_requirement_level_to_v2(attr.requirement_level.clone());
                            attributes.push(attribute_group::AttributeGroupAttributeRef {
                                base: a,
                                requirement_level: req_level,
                            });
                        }
                    }
                    attribute_groups.push(V2AttributeGroup {
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
            GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => {}
        }
    }

    let mut attributes = Vec::new();
    for g in r.groups.iter() {
        for a in g.attributes.iter() {
            if let Some(attr) = c.attribute(a) {
                let is_def = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.attribute(&attr.name))
                    .is_none();
                if is_def {
                    if let Some(v2) = v2_catalog.convert_ref(attr) {
                        attributes.push(v2);
                    }
                }
            }
        }
    }
    attributes.sort_by_key(|a| a.0);
    attributes.dedup();

    let v2_registry = V2Registry {
        attributes,
        spans,
        metrics,
        events,
        entities,
        attribute_groups,
    };
    let v2_refinements = V2Refinements {
        spans: span_refinements,
        metrics: metric_refinements,
        events: event_refinements,
        entities: entity_refinements,
    };
    Ok((v2_catalog.into(), v2_registry, v2_refinements, dependencies))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::attribute::Attribute as V1Attribute;
    use crate::v1::registry::Group as V1Group;
    use crate::v1::V1_RESOLVED_FILE_FORMAT;
    use crate::v2::attribute::AttributeRef;
    use weaver_semconv::stability::Stability;

    #[test]
    fn test_convert_span_v1_to_v2() {
        let mut builder = crate::v1::catalog::test_utils::CatalogBuilder::default();
        let ref0 = builder.add(
            V1Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::v1::attribute::RequirementLevel::Basic(
                    weaver_semconv::v1::attribute::BasicRequirementLevelSpec::Required,
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
            V1Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::v1::attribute::RequirementLevel::Basic(
                    weaver_semconv::v1::attribute::BasicRequirementLevelSpec::Recommended,
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
        let catalog = builder.build();

        let registry = V1Registry {
            registry_url: "http://test/schemas/1.0.0".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![V1Group {
                id: "span.test".to_owned(),
                r#type: GroupType::Span,
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                name: None,
                lineage: None,
                display_name: None,
                attributes: vec![ref0, ref1],
                span_kind: Some(weaver_semconv::v1::group::SpanKindSpec::Client),
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
                is_v2: false,
                span_name: None,
            }],
        };

        let (attribute_catalog, registry, _, _) =
            convert_v1_to_v2(catalog, registry, BTreeSet::new()).unwrap();

        assert_eq!(attribute_catalog.len(), 1);
        assert_eq!(registry.spans.len(), 1);
        assert_eq!(registry.spans[0].attributes.len(), 2);
        assert_eq!(
            registry.spans[0].attributes[0].base,
            AttributeRef(0)
        );
        assert_eq!(
            registry.spans[0].attributes[0].requirement_level,
            weaver_semconv::v2::attribute::RequirementLevel::Basic(
                weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Required
            )
        );
        assert_eq!(
            registry.spans[0].attributes[1].base,
            AttributeRef(0)
        );
        assert_eq!(
            registry.spans[0].attributes[1].requirement_level,
            weaver_semconv::v2::attribute::RequirementLevel::Basic(
                weaver_semconv::v2::attribute::BasicRequirementLevelSpec::Recommended
            )
        );
        assert_eq!(
            registry.spans[0].attributes[1].sampling_relevant,
            Some(true)
        );
    }

    #[test]
    fn test_try_from_v1_to_v2() {
        let mut dependencies = BTreeSet::new();
        let _ = dependencies.insert("http://dependency/url/1.0.0".try_into().unwrap());

        let v1_schema = V1ResolvedSchema {
            file_format: V1_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "my-registry".to_owned(),
            catalog: V1Catalog::default(),
            registry: V1Registry {
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

        let v2_schema: Result<V2ResolvedSchema, _> = v1_schema.try_into();
        assert!(v2_schema.is_ok());
        let v2_schema = v2_schema.unwrap();
        assert_eq!(v2_schema.file_format, V2_RESOLVED_FILE_FORMAT);
        assert_eq!(
            v2_schema.schema_url,
            "http://test/schemas/1.0.0".try_into().unwrap()
        );
        assert_eq!(v2_schema.dependencies, dependencies);
    }
}
