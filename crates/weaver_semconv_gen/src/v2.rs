// SPDX-License-Identifier: Apache-2.0

//! Version 2 Schema for Markdown generation.

use serde::Serialize;
use weaver_forge::{
    v2::{
        entity::EntityAttribute, event::EventAttribute, metric::MetricAttribute,
        registry::ForgeResolvedRegistry, span::SpanAttribute,
    },
    TemplateEngine,
};
use weaver_resolved_schema::v2::{
    attribute::Attribute, attribute_group::AttributeGroup, catalog::AttributeCatalog,
    entity::Entity, event::Event, metric::Metric, span::Span, ResolvedTelemetrySchema, Signal,
};

use crate::{
    parser::{parse_id_lookup_v2, IdLookupV2, RegistryLookup},
    Error, MarkdownSnippetGenerator,
};

/// Stat we need to generate markdown snippets from configuration.
pub struct SnippetGenerator {
    lookup: ResolvedTelemetrySchema,
    template_schema: ForgeResolvedRegistry,
    template_engine: TemplateEngine,
}

impl SnippetGenerator {
    /// Constructs a new SnipperGenerator for v2 schema with given template engine.
    #[must_use]
    pub fn new(
        registry: ResolvedTelemetrySchema,
        template_schema: ForgeResolvedRegistry,
        template_engine: TemplateEngine,
    ) -> Self {
        Self {
            lookup: registry,
            template_schema,
            template_engine,
        }
    }
}

impl MarkdownSnippetGenerator for SnippetGenerator {
    fn generate_markdown_snippet(
        &self,
        args: crate::parser::GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error> {
        // Note: args.id could be ANYTHING in new repo.
        // We will do lookups on *refinements* as that
        // is the equivalent of groups in V1.
        // Additionally, we'll use the prefix, e.g. `metric.*` to
        // guide our search. This *may* break some old lookups.
        let group = lookup_id(&self.lookup, &args.id)?.ok_or(Error::GroupNotFound {
            id: args.id.clone(),
        })?;

        let context = MarkdownSnippetContext {
            group,
            tag_filter: args
                .tag_filters()
                .into_iter()
                .map(|s| s.to_owned())
                .collect(),
            attribute_registry_base_url: attribute_registry_base_url.map(|s| s.to_owned()),
        };
        // We automatically default to specific file for the snippet types.
        let snippet_template_file = "snippet.md.j2";
        let mut result = self.template_engine.generate_snippet(
            &context,
            ".",
            snippet_template_file.to_owned(),
        )?;
        result.push('\n');
        Ok(result)
    }

    fn generate_weaver_snippet(
        &self,
        args: crate::parser::WeaverGenerateMarkdownArgs,
    ) -> Result<String, Error> {
        let snippet_template_file = args.template.unwrap_or("snippet.md.j2".to_owned());
        // TODO - we need to JQ our way into the value we'll send.
        let mut result = self.template_engine.generate_snippet(
            &self.template_schema,
            &args.query,
            snippet_template_file.to_owned(),
        )?;
        result.push('\n');
        Ok(result)
    }
}

/// Looks up a signal from a registry by an id string.
fn lookup_signal_by_id<'a, T: Signal>(signals: &'a [T], id: &str) -> Option<&'a T> {
    signals.iter().find(|s| s.id() == id)
}

/// Creates a renderable context for a resolved metric.
fn resolved_metric<AC: AttributeCatalog>(m: &Metric, catalog: &AC) -> ResolvedId {
    let mut attributes = Vec::new();
    for ar in m.attributes.iter() {
        let attr = catalog.attribute(&ar.base).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.base.0
            )
        });
        attributes.push(MetricAttribute {
            base: weaver_forge::v2::attribute::Attribute {
                key: attr.key.clone(),
                r#type: attr.r#type.clone(),
                examples: attr.examples.clone(),
                common: attr.common.clone(),
            },
            requirement_level: ar.requirement_level.clone(),
        });
    }
    ResolvedId::Metric(ResolvedMetric {
        metric: weaver_forge::v2::metric::Metric {
            name: m.name.clone(),
            instrument: m.instrument.clone(),
            unit: m.unit.clone(),
            attributes,
            entity_associations: m.entity_associations.clone(),
            common: m.common.clone(),
        },
    })
}

// Creates renderable span.
fn resolved_span<AC: AttributeCatalog>(s: &Span, catalog: &AC) -> ResolvedId {
    let mut attributes = Vec::new();
    for ar in s.attributes.iter() {
        let attr = catalog.attribute(&ar.base).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.base.0
            )
        });
        attributes.push(SpanAttribute {
            base: weaver_forge::v2::attribute::Attribute {
                key: attr.key.clone(),
                r#type: attr.r#type.clone(),
                examples: attr.examples.clone(),
                common: attr.common.clone(),
            },
            requirement_level: ar.requirement_level.clone(),
            sampling_relevant: ar.sampling_relevant,
        });
    }
    ResolvedId::Span(ResolvedSpan {
        span: weaver_forge::v2::span::Span {
            r#type: s.r#type.clone(),
            name: s.name.clone(),
            attributes,
            kind: s.kind.clone(),
            entity_associations: s.entity_associations.clone(),
            common: s.common.clone(),
        },
    })
}

// Creates renderable event.
fn resolved_event<AC: AttributeCatalog>(s: &Event, catalog: &AC) -> ResolvedId {
    let mut attributes = Vec::new();
    for ar in s.attributes.iter() {
        let attr = catalog.attribute(&ar.base).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.base.0
            )
        });
        attributes.push(EventAttribute {
            base: weaver_forge::v2::attribute::Attribute {
                key: attr.key.clone(),
                r#type: attr.r#type.clone(),
                examples: attr.examples.clone(),
                common: attr.common.clone(),
            },
            requirement_level: ar.requirement_level.clone(),
        });
    }
    ResolvedId::Event(ResolvedEvent {
        event: weaver_forge::v2::event::Event {
            name: s.name.clone(),
            attributes,
            entity_associations: s.entity_associations.clone(),
            common: s.common.clone(),
        },
    })
}

// Creates renderable entity.
fn resolved_entity<AC: AttributeCatalog>(s: &Entity, catalog: &AC) -> ResolvedId {
    let mut identity = Vec::new();
    for ar in s.identity.iter() {
        let attr = catalog.attribute(&ar.base).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.base.0
            )
        });
        identity.push(EntityAttribute {
            base: weaver_forge::v2::attribute::Attribute {
                key: attr.key.clone(),
                r#type: attr.r#type.clone(),
                examples: attr.examples.clone(),
                common: attr.common.clone(),
            },
            requirement_level: ar.requirement_level.clone(),
        });
    }
    let mut description = Vec::new();
    for ar in s.description.iter() {
        let attr = catalog.attribute(&ar.base).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.base.0
            )
        });
        description.push(EntityAttribute {
            base: weaver_forge::v2::attribute::Attribute {
                key: attr.key.clone(),
                r#type: attr.r#type.clone(),
                examples: attr.examples.clone(),
                common: attr.common.clone(),
            },
            requirement_level: ar.requirement_level.clone(),
        });
    }
    ResolvedId::Entity(ResolvedEntity {
        entity: weaver_forge::v2::entity::Entity {
            common: s.common.clone(),
            r#type: s.r#type.clone(),
            identity,
            description,
        },
    })
}

// Creates renderable attribute group.
fn resolved_attribute_group<AC: AttributeCatalog>(s: &AttributeGroup, catalog: &AC) -> ResolvedId {
    let mut attributes = Vec::new();
    for ar in s.attributes.iter() {
        let attr = catalog.attribute(ar).unwrap_or_else(|| {
            panic!(
                "Invalid schema file: Attribute reference {} does not exist",
                ar.0
            )
        });
        attributes.push(weaver_forge::v2::attribute::Attribute {
            key: attr.key.clone(),
            r#type: attr.r#type.clone(),
            examples: attr.examples.clone(),
            common: attr.common.clone(),
        });
    }
    ResolvedId::AttributeGroup(ResolvedAttributeGroup {
        attribute_group: weaver_forge::v2::attribute_group::AttributeGroup {
            common: s.common.clone(),
            id: s.id.clone(),
            attributes,
        },
    })
}

// Creates renderable attribute group.
fn resolved_attribute(attr: &Attribute) -> ResolvedId {
    ResolvedId::Attribute(ResolvedAttribute {
        attribute: weaver_forge::v2::attribute::Attribute {
            key: attr.key.clone(),
            r#type: attr.r#type.clone(),
            examples: attr.examples.clone(),
            common: attr.common.clone(),
        },
    })
}

fn lookup_id(registry: &ResolvedTelemetrySchema, id: &str) -> Result<Option<ResolvedId>, Error> {
    let lookup = parse_id_lookup_v2(id)?;
    match lookup {
        IdLookupV2::Registry(RegistryLookup::Attribute { id }) => Ok(registry
            .registry
            .attributes
            .iter()
            .filter_map(|ar| registry.attribute_catalog.attribute(ar))
            .find(|a| a.key == id)
            .map(resolved_attribute)),
        IdLookupV2::Registry(RegistryLookup::AttributeGroup { id }) => Ok(lookup_signal_by_id(
            &registry.registry.attribute_groups,
            &id,
        )
        .map(|ag| resolved_attribute_group(ag, &registry.attribute_catalog))),
        IdLookupV2::Registry(RegistryLookup::Span { id }) => {
            Ok(lookup_signal_by_id(&registry.registry.spans, &id)
                .map(|s| resolved_span(s, &registry.attribute_catalog)))
        }
        IdLookupV2::Registry(RegistryLookup::Metric { id }) => {
            Ok(lookup_signal_by_id(&registry.registry.metrics, &id)
                .map(|m| resolved_metric(m, &registry.attribute_catalog)))
        }
        IdLookupV2::Registry(RegistryLookup::Event { id }) => {
            Ok(lookup_signal_by_id(&registry.registry.events, &id)
                .map(|e| resolved_event(e, &registry.attribute_catalog)))
        }
        IdLookupV2::Registry(RegistryLookup::Entity { id }) => {
            Ok(lookup_signal_by_id(&registry.registry.entities, &id)
                .map(|e| resolved_entity(e, &registry.attribute_catalog)))
        }
        IdLookupV2::Refinement(crate::parser::RefinementLookup::Metric { id }) => Ok(registry
            .refinements
            .metrics
            .iter()
            .find(|m| m.id == id)
            .map(|m| resolved_metric(&m.metric, &registry.attribute_catalog))),
        IdLookupV2::Refinement(crate::parser::RefinementLookup::Event { id }) => Ok(registry
            .refinements
            .events
            .iter()
            .find(|s| s.id == id)
            .map(|e| resolved_event(&e.event, &registry.attribute_catalog))),
        IdLookupV2::Refinement(crate::parser::RefinementLookup::Span { id }) => Ok(registry
            .refinements
            .spans
            .iter()
            .find(|s| s.id == id)
            .map(|s| resolved_span(&s.span, &registry.attribute_catalog))),
    }
}

#[derive(Serialize)]
#[serde(tag = "signal_type")]
#[serde(rename_all = "snake_case")]
enum ResolvedId {
    Attribute(ResolvedAttribute),
    AttributeGroup(ResolvedAttributeGroup),
    Span(ResolvedSpan),
    Metric(ResolvedMetric),
    Event(ResolvedEvent),
    Entity(ResolvedEntity),
}

#[derive(Serialize)]
struct ResolvedAttribute {
    #[serde(flatten)]
    attribute: weaver_forge::v2::attribute::Attribute,
}

#[derive(Serialize)]
struct ResolvedAttributeGroup {
    #[serde(flatten)]
    attribute_group: weaver_forge::v2::attribute_group::AttributeGroup,
}

#[derive(Serialize)]
struct ResolvedSpan {
    #[serde(flatten)]
    span: weaver_forge::v2::span::Span,
}

#[derive(Serialize)]
struct ResolvedMetric {
    #[serde(flatten)]
    metric: weaver_forge::v2::metric::Metric,
}

#[derive(Serialize)]
struct ResolvedEvent {
    #[serde(flatten)]
    event: weaver_forge::v2::event::Event,
}

#[derive(Serialize)]
struct ResolvedEntity {
    #[serde(flatten)]
    entity: weaver_forge::v2::entity::Entity,
}

/// This struct is passed into markdown snippets for generation.
#[derive(Serialize)]
struct MarkdownSnippetContext {
    // TODO - we need something new here.
    group: ResolvedId,
    tag_filter: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attribute_registry_base_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_yaml::Value;
    use weaver_forge::{
        config::{Params, WeaverConfig},
        file_loader::FileSystemFileLoader,
        v2::registry::ForgeResolvedRegistry,
        TemplateEngine,
    };
    use weaver_resolved_schema::v2::{
        attribute::{Attribute, AttributeRef},
        attribute_group::AttributeGroup,
        entity::{Entity, EntityAttributeRef},
        event::{Event, EventAttributeRef, EventRefinement},
        metric::{Metric, MetricAttributeRef, MetricRefinement},
        refinements::Refinements,
        registry::Registry,
        span::{Span, SpanAttributeRef, SpanRefinement},
        ResolvedTelemetrySchema,
    };
    use weaver_semconv::{
        attribute::{AttributeType, PrimitiveOrArrayTypeSpec},
        group::InstrumentSpec,
        v2::{span::SpanName, CommonFields},
    };

    use crate::{Error, MarkdownSnippetGenerator, SnipperGeneratorV2};

    fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_template_engine() -> Result<(), Error> {
        let loader = FileSystemFileLoader::try_new("templates/registry".into(), "markdown_v2")?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        let params = {
            let mut p = Params::default();
            let _ = p
                .params
                .insert("test".to_owned(), Value::String("param".to_owned()));
            p
        };
        let template = TemplateEngine::try_new(config, loader, params)?;
        let registry = test_registry();
        let template_registry = ForgeResolvedRegistry::try_from_resolved_schema(registry.clone())?;
        let generator = SnipperGeneratorV2::new(registry, template_registry, template);
        let attribute_registry_url = "/docs/attributes-registry";
        // Now we should check a snippet.
        let test = "data_v2/templates.md";
        println!("--- Running template engine test: {test} ---");
        force_print_error(generator.update_markdown(test, true, Some(attribute_registry_url)));
        Ok(())
    }

    fn test_registry() -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: "resolved/2.0.0".to_owned(),
            schema_url: "todo/1.0.0".to_owned(),
            registry_id: "main".to_owned(),
            attribute_catalog: vec![Attribute {
                key: "attr1".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields::default(),
            }],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![AttributeGroup {
                    id: "test.common".to_owned().into(),
                    attributes: vec![AttributeRef(0)],
                    common: CommonFields::default(),
                }],
                registry_url: "todo".to_owned(),
                spans: vec![Span {
                    r#type: "trace.test".to_owned().into(),
                    kind: weaver_semconv::group::SpanKindSpec::Client,
                    name: SpanName {
                        note: "note".to_owned(),
                    },
                    attributes: vec![SpanAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                        sampling_relevant: None,
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                metrics: vec![Metric {
                    name: "test.metric".to_owned().into(),
                    instrument: InstrumentSpec::Counter,
                    unit: "{1}".to_owned(),
                    attributes: vec![MetricAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                events: vec![Event {
                    name: "test.event".to_owned().into(),
                    attributes: vec![EventAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    entity_associations: vec![],
                    common: CommonFields::default(),
                }],
                entities: vec![Entity {
                    r#type: "test.entity".to_owned().into(),
                    identity: vec![EntityAttributeRef {
                        base: AttributeRef(0),
                        requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                            weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                        ),
                    }],
                    description: vec![],
                    common: CommonFields::default(),
                }],
            },
            refinements: Refinements {
                spans: vec![SpanRefinement {
                    id: "test".to_owned().into(),
                    span: Span {
                        r#type: "trace.test".to_owned().into(),
                        kind: weaver_semconv::group::SpanKindSpec::Client,
                        name: SpanName {
                            note: "note".to_owned(),
                        },
                        attributes: vec![SpanAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: None,
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
                    },
                }],
                metrics: vec![MetricRefinement {
                    id: "test".to_owned().into(),
                    metric: Metric {
                        name: "test.metric".to_owned().into(),
                        instrument: InstrumentSpec::Counter,
                        unit: "{1}".to_owned(),
                        attributes: vec![MetricAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
                    },
                }],
                events: vec![EventRefinement {
                    id: "test".to_owned().into(),
                    event: Event {
                        name: "test.event".to_owned().into(),
                        attributes: vec![EventAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        entity_associations: vec![],
                        common: CommonFields::default(),
                    },
                }],
            },
        }
    }
}
