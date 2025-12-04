// SPDX-License-Identifier: Apache-2.0

//! Version 2 Schema for Markdown generation.

use serde::Serialize;
use weaver_forge::{v2::metric::MetricAttribute, TemplateEngine};
use weaver_resolved_schema::v2::ResolvedTelemetrySchema;
use weaver_semconv::v2::signal_id::SignalId;

use crate::{Error, MarkdownSnippetGenerator};

/// Stat we need to generate markdown snippets from configuration.
pub struct SnippetGenerator {
    lookup: ResolvedTelemetrySchema,
    template_engine: TemplateEngine,
}

impl SnippetGenerator {
    /// Constructs a new SnipperGenerator for v2 schema with given template engine.
    #[must_use]
    pub fn new(registry: ResolvedTelemetrySchema, template_engine: TemplateEngine) -> Self {
        Self {
            lookup: registry,
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
        let group = lookup_id(&self.lookup, &args.id).ok_or(Error::GroupNotFound {
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
        let mut result = self
            .template_engine
            .generate_snippet(&context, snippet_template_file.to_owned())?;
        result.push('\n');
        Ok(result)
    }
}

fn lookup_id(registry: &ResolvedTelemetrySchema, id: &str) -> Option<ResolvedId> {
    // TODO - we should parse ID into a lookup path, e.g.
    // `refinements.spans.*` -> LookupRefinement(Span(*))
    // `spans.*` -> Either(LookupRegistry(Span(*)), LookupRefinement(Span(*)))
    // We probably will deprecate this feature before that's needed.

    // A bit of a hack to use V1 -> V2 namespacing rules.
    if id.starts_with("spans.") {
        let span_type: SignalId = id
            .strip_prefix("spans.")
            .expect("Prefix should already have been tested")
            .to_owned()
            .into();
        registry
            .refinements
            .spans
            .iter()
            .find(|s| s.span.r#type == span_type)
            .map(|s| {
                // TODO
                ResolvedId::Span(ResolvedSpan {})
            })
    } else if id.starts_with("metrics.") {
        let metric_id: SignalId = id
            .strip_prefix("metrics.")
            .expect("Prefix should already have been tested")
            .to_owned()
            .into();
        registry
            .refinements
            .metrics
            .iter()
            .find(|m| m.id == metric_id)
            .map(|m| {
                let mut attributes = Vec::new();
                for ar in m.metric.attributes.iter() {
                    let attr = registry.registry.attribute(&ar.base).expect(&format!(
                        "Invalid schema file: Attribute reference {} does not exist",
                        ar.base.0
                    ));
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
                        name: m.metric.name.clone(),
                        instrument: m.metric.instrument.clone(),
                        unit: m.metric.unit.clone(),
                        attributes,
                        entity_associations: m.metric.entity_associations.clone(),
                        common: m.metric.common.clone(),
                    },
                })
            })
    } else if id.starts_with("events.") {
        let event_name: SignalId = id
            .strip_prefix("events.")
            .expect("Prefix should already have been tested")
            .to_owned()
            .into();
        registry
            .refinements
            .events
            .iter()
            .find(|e| e.event.name == event_name)
            .map(|e| {
                // TODO
                ResolvedId::Event(ResolvedEvent {})
            })
    } else if id.starts_with("entities.") {
        let entity_type: SignalId = id
            .strip_prefix("entities.")
            .expect("Prefix should already have been tested")
            .to_owned()
            .into();
        registry
            .registry
            .entities
            .iter()
            .find(|e| e.r#type == entity_type)
            .map(|e| {
                // TODO
                ResolvedId::Entity(ResolvedEntity {})
            })
    } else {
        // Assume anything not prefixed is a raw attribute group.
        let group_id: SignalId = id.to_owned().into();
        registry
            .registry
            .attribute_groups
            .iter()
            .find(|g| g.id == group_id)
            .map(|g| {
                // TODO
                ResolvedId::AttributeGroup(ResolvedAttributeGroup {})
            })
    }
}

#[derive(Serialize)]
#[serde(tag = "signal_type")]
enum ResolvedId {
    AttributeGroup(ResolvedAttributeGroup),
    Span(ResolvedSpan),
    Metric(ResolvedMetric),
    Event(ResolvedEvent),
    Entity(ResolvedEntity),
}

#[derive(Serialize)]
struct ResolvedAttributeGroup {}

#[derive(Serialize)]
struct ResolvedSpan {}

#[derive(Serialize)]
struct ResolvedMetric {
    #[serde(flatten)]
    metric: weaver_forge::v2::metric::Metric,
}

#[derive(Serialize)]
struct ResolvedEvent {}

#[derive(Serialize)]
struct ResolvedEntity {}

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
    use weaver_common::{diagnostic::DiagnosticMessages, vdir::VirtualDirectoryPath};
    use weaver_forge::{TemplateEngine, config::{Params, WeaverConfig}, file_loader::FileSystemFileLoader};
    use weaver_resolved_schema::v2::{ResolvedTelemetrySchema, attribute_group::AttributeGroup, metric::{Metric, MetricRefinement}, refinements::Refinements, registry::Registry};
    use weaver_semconv::{group::InstrumentSpec, v2::CommonFields};

    use crate::{Error, MarkdownSnippetGenerator, SnipperGeneratorV2};

     fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_template_engine() -> Result<(), Error> {
        // TODO - pull in V2 template.
        let loader = FileSystemFileLoader::try_new("templates/registry".into(), "markdown")?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        let params = {
            let mut p = Params::default();
            let _ = p
                .params
                .insert("test".to_owned(), Value::String("param".to_owned()));
            p
        };
        let template = TemplateEngine::try_new(config, loader, params)?;
        let generator = SnipperGeneratorV2::new(test_registry(), template);
        let attribute_registry_url = "/docs/attributes-registry";
        // Now we should check a snippet.
        let test = "data/templates.md";
        println!("--- Running template engine test: {test} ---");
        force_print_error(generator.update_markdown(test, true, Some(attribute_registry_url)));
        Ok(())
    }

    fn test_registry() -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: "v2/resolved".to_owned(),
            schema_url: "todo/1.0.0".to_owned(),
            registry_id: "main".to_owned(),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![
                    AttributeGroup { 
                        id: "test.common".to_owned().into(),
                        attributes: vec![], 
                        common: CommonFields::default(),
                    },
                ],
                registry_url: "todo".to_owned(),
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![
                    MetricRefinement {
                        id: "test".to_owned().into(),
                        metric: Metric { 
                            name: "test.metric".to_owned().into(),
                            instrument: InstrumentSpec::Counter,
                            unit: "{1}".to_owned(),
                            attributes: vec![],
                            entity_associations: vec![],
                            common: CommonFields::default(),
                        }
                    }
                ],
                events: vec![],
            },
        }
    }
}