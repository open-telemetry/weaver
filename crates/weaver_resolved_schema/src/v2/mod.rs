//! Version 2 of semantic convention schema.

use std::collections::{HashMap, HashSet};

use weaver_semconv::{
    group::GroupType,
    v2::{signal_id::SignalId, span::SpanName, CommonFields},
};

use crate::v2::{
    metric::Metric,
    span::{Span, SpanRefinement},
};

pub mod attribute;
pub mod catalog;
pub mod metric;
pub mod registry;
pub mod span;

fn fix_group_id(prefix: &'static str, group_id: &str) -> SignalId {
    if group_id.starts_with(prefix) {
        group_id.trim_start_matches(prefix).to_owned().into()
    } else {
        group_id.to_owned().into()
    }
}

fn fix_span_group_id(group_id: &str) -> SignalId {
    fix_group_id("span.", group_id)
}

/// Converts a V1 registry + catalog to V2.
pub fn convert_v1_to_v2(
    c: crate::catalog::Catalog,
    r: crate::registry::Registry,
) -> Result<(catalog::Catalog, registry::Registry), crate::error::Error> {
    // When pulling attributes, as we collapse things, we need to filter
    // to just unique.
    let attributes: HashSet<attribute::Attribute> = c
        .attributes
        .iter()
        .cloned()
        .map(|a| {
            attribute::Attribute {
                key: a.name,
                r#type: a.r#type,
                examples: a.examples,
                common: CommonFields {
                    brief: a.brief,
                    note: a.note,
                    // TODO - Check this assumption.
                    stability: a
                        .stability
                        .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                    deprecated: a.deprecated,
                    annotations: a.annotations.unwrap_or_default(),
                },
            }
        })
        .collect();

    let v2_catalog = catalog::Catalog::from_attributes(attributes.into_iter().collect());

    // Create a lookup so we can check inheritance.
    let mut group_type_lookup = HashMap::new();
    for g in r.groups.iter() {
        println!("Group {} is type: {:?}", &g.id, g.r#type);
        let _ = group_type_lookup.insert(g.id.clone(), g.r#type.clone());
    }
    // Pull signals from the registry and create a new span-focused registry.
    let mut spans = Vec::new();
    let mut span_refinements = Vec::new();
    let mut metrics = Vec::new();
    let mut metric_refinements = Vec::new();

    for g in r.groups.iter() {
        let extend_type = g.extends.as_ref().and_then(|id| group_type_lookup.get(id));
        match g.r#type {
            GroupType::Span => {
                // Check if we extend another span.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Span)
                    .unwrap_or(false);
                // Pull all the attribute references.
                let mut span_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        span_attributes.push(span::SpanAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                            sampling_relevant: attr.sampling_relevant.clone(),
                        });
                    } else {
                        // TODO logic error!
                    }
                }
                if !is_refinement {
                    let span = Span {
                        r#type: fix_span_group_id(&g.id),
                        kind: g
                            .span_kind
                            .clone()
                            .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                        // TODO - Pass advanced name controls through V1 groups.
                        name: SpanName {
                            note: g.name.clone().unwrap_or_default(),
                        },
                        entity_associations: g.entity_associations.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        attributes: span_attributes,
                    };
                    spans.push(span.clone());
                    span_refinements.push(SpanRefinement {
                        id: span.r#type.clone(),
                        span,
                    });
                } else {
                    // unwrap should be safe becasue we verified this is a refinement earlier.
                    let span_type = g
                        .lineage
                        .as_ref()
                        .and_then(|l| l.extends_group.as_ref())
                        .map(|id| fix_span_group_id(id))
                        .unwrap();
                    span_refinements.push(SpanRefinement {
                        id: fix_span_group_id(&g.id),
                        span: Span {
                            r#type: span_type,
                            kind: g
                                .span_kind
                                .clone()
                                .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                            // TODO - Pass advanced name controls through V1 groups.
                            name: SpanName {
                                note: g.name.clone().unwrap_or_default(),
                            },
                            entity_associations: g.entity_associations.clone(),
                            common: CommonFields {
                                brief: g.brief.clone(),
                                note: g.note.clone(),
                                stability: g
                                    .stability
                                    .clone()
                                    .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                                deprecated: g.deprecated.clone(),
                                annotations: g.annotations.clone().unwrap_or_default(),
                            },
                            attributes: span_attributes,
                        },
                    })
                }
            }
            GroupType::Event => {
                // todo!()
            }
            GroupType::Metric => {
                // Check if we extend another metric.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Metric)
                    .unwrap_or(false);
                println!(
                    "Group {} is_refinement: {}, extends: {:?}",
                    &g.id, is_refinement, extend_type
                );
                let mut metric_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        metric_attributes.push(metric::MetricAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                    }
                }
                // TODO - deal with unwrap errors.
                let metric = Metric {
                    name: g.metric_name.clone().unwrap().into(),
                    instrument: g.instrument.clone().unwrap(),
                    unit: g.unit.clone().unwrap(),
                    attributes: metric_attributes,
                    entity_associations: g.entity_associations.clone(),
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
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
                // todo!()
            }
            GroupType::AttributeGroup
            | GroupType::MetricGroup
            | GroupType::Scope
            | GroupType::Undefined => {
                // Ignored for now, we should probably issue warnings.
            }
        }
    }

    let v2_registry = registry::Registry {
        registry_url: r.registry_url,
        spans,
        span_refinements,
        metrics,
        metric_refinements,
    };
    Ok((v2_catalog, v2_registry))
}

#[cfg(test)]
mod tests {

    use weaver_semconv::{provenance::Provenance, stability::Stability, v2};

    use crate::{attribute::Attribute, lineage::GroupLineage, registry::Group};

    use super::*;

    #[test]
    fn test_convert_span_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
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
                note: "".to_string(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
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
                note: "".to_string(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_span_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_span_lineage.extends("span.my-span");
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
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
                    attributes: vec![test_refs[1].clone()],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
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
                    attributes: vec![test_refs[1].clone()],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: Some(refinement_span_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                },
            ],
        };

        let (v2_catalog, v2_registry) = convert_v1_to_v2(v1_catalog, v1_registry).unwrap();
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_catalog.attributes().len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.spans.len(), 1);
        if let Some(span) = v2_registry.spans.iter().next() {
            assert_eq!(span.r#type, "my-span".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_registry.span_refinements.len(), 2);
        let span_ref_ids: Vec<String> = v2_registry
            .span_refinements
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            span_ref_ids,
            vec!["my-span".to_owned(), "custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_metric_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
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
                note: "".to_string(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
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
                note: "".to_string(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_metric_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_metric_lineage.extends("metric.http");
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
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
                    attributes: vec![test_refs[0].clone()],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
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
                    attributes: vec![test_refs[1].clone()],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: Some(refinement_metric_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                },
            ],
        };

        let (v2_catalog, v2_registry) = convert_v1_to_v2(v1_catalog, v1_registry).unwrap();
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_catalog.attributes().len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.metrics.len(), 1);
        if let Some(metric) = v2_registry.metrics.iter().next() {
            assert_eq!(metric.name, "http".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_registry.metric_refinements.len(), 2);
        let metric_ref_ids: Vec<String> = v2_registry
            .metric_refinements
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            metric_ref_ids,
            vec!["http".to_owned(), "http.custom".to_owned()]
        );
    }
}
