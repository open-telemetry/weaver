//! Version 2 of semantic convention schema.

use std::collections::HashSet;

use weaver_semconv::{
    group::GroupType,
    v2::{span::SpanName, CommonFields},
};

use crate::v2::span::Span;

pub mod attribute;
pub mod catalog;
pub mod registry;
pub mod span;

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

    // TODO - pull spans.
    let mut spans = Vec::new();
    for g in r.groups.iter() {
        if g.r#type == GroupType::Span {
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
            spans.push(Span {
                // TODO - Strip the `span.` from the name if it exists.
                r#type: g.id.clone().into(),
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
            });
        }
    }

    let v2_registry = registry::Registry {
        registry_url: r.registry_url,
        spans,
    };
    Ok((v2_catalog, v2_registry))
}

#[cfg(test)]
mod tests {

    use weaver_semconv::{stability::Stability, v2};

    use crate::{attribute::Attribute, registry::Group};

    use super::*;

    #[test]
    fn test_convert_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(weaver_semconv::attribute::BasicRequirementLevelSpec::Required),
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
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended),
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
            ],
        };

        let (v2_catalog, v2_registry) = convert_v1_to_v2(v1_catalog, v1_registry).unwrap();
        println!("Catalog: {v2_catalog:?}");
        // TODO - assert only ONE attribute due to sharing.
        println!("Registry: {v2_registry:?}");
        // TODO - assert attribute fields not shared show up on ref in span.
    }
}
