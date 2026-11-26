// SPDX-License-Identifier: Apache-2.0

//! Reading V1 schemas as dependencies.

use weaver_resolved_schema::v1::attribute::UnresolvedAttribute;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema as V1Schema;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::AttributeSpec;
use weaver_semconv::v1::group::GroupType;

use crate::graph::entity_lookup::{EntityLocation, EntityLookup};
use crate::graph::exclusion::is_excluded;
use crate::v1::attribute::AttributeSource;
use crate::v1::dependency::{GroupRefinementLookup, GroupSource, GroupSummary};
use crate::v1::imports::{import_match_keys, v1_association_origins};

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
                    spec: AttributeSpec::Id {
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
            summary.association_origins = v1_association_origins(self, g);
            if summary.origin.is_none() {
                summary.origin = my_schema_url;
            }
            summary
        })
    }
}

/// Resolves the defining `AttributeSource` for `attr_name` in `schema` via root attributes or group provenance.
pub(crate) fn find_attribute_source(
    schema: &V1Schema,
    attr_name: &str,
    my_schema_url: &SchemaUrl,
) -> AttributeSource {
    if let Some((_, source_group_id)) = schema.catalog().root_attribute(attr_name) {
        let schema_url = if let Some(schema_name) = source_group_id.strip_prefix("v2_dependency.") {
            // Recover the full `SchemaUrl` from `dependencies` first, since an
            // attribute-only reference imports no group.
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

#[cfg(test)]
pub(crate) fn example_v1_schema() -> V1Schema {
    use std::collections::{BTreeSet, HashMap};
    use weaver_resolved_schema::v1::attribute::{Attribute, AttributeRef};
    use weaver_resolved_schema::v1::catalog::Catalog;
    use weaver_resolved_schema::v1::registry::{Group, Registry};
    use weaver_semconv::v1::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};
    use weaver_semconv::v1::group::SpanKindSpec;

    V1Schema {
        file_format: "resolved/1.0.0".to_owned(),
        schema_url: "http://test/schemas/1.0.0".to_owned(),
        registry_id: "test-registry".to_owned(),
        registry: Registry {
            registry_url: "v1-example".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![
                Group {
                    id: "a".to_owned(),
                    r#type: GroupType::AttributeGroup,
                    brief: Default::default(),
                    note: Default::default(),
                    prefix: Default::default(),
                    extends: Default::default(),
                    stability: Default::default(),
                    deprecated: Default::default(),
                    attributes: vec![AttributeRef(0)],
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
                    span_links: Vec::new(),
                },
                Group {
                    id: "span.v1".to_owned(),
                    r#type: GroupType::Span,
                    brief: Default::default(),
                    note: Default::default(),
                    prefix: Default::default(),
                    extends: Default::default(),
                    stability: Default::default(),
                    deprecated: Default::default(),
                    attributes: vec![],
                    span_kind: Some(SpanKindSpec::Client),
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
                    span_links: Vec::new(),
                },
            ],
        },
        catalog: Catalog::new(
            vec![Attribute {
                name: "a.test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
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
        dependencies: BTreeSet::new(),
        versions: None,
        registry_manifest: None,
    }
}
