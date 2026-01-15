// SPDX-License-Identifier: Apache-2.0

//! Version 1 Schema for Markdown generation.

use serde::Serialize;
use weaver_common::{
    diagnostic::{DiagnosticMessage, DiagnosticMessages},
    result::WResult,
};
use weaver_forge::{registry::ResolvedGroup, TemplateEngine};
use weaver_resolved_schema::{catalog::Catalog, registry::Group, ResolvedTelemetrySchema};
use weaver_resolver::SchemaResolver;
use weaver_semconv::registry_repo::RegistryRepo;

use crate::{parser::GenerateMarkdownArgs, Error, MarkdownSnippetGenerator};

/// State we need to generate markdown snippets from configuration.
pub struct SnippetGenerator {
    lookup: ResolvedSemconvRegistry,
    template_engine: TemplateEngine,
}

impl SnippetGenerator {
    /// Resolve semconv registry, and make it available for rendering.
    #[deprecated]
    pub fn try_from_registry_repo(
        registry_repo: &RegistryRepo,
        template_engine: TemplateEngine,
        diag_msgs: &mut DiagnosticMessages,
        follow_symlinks: bool,
        include_unreferenced: bool,
    ) -> Result<SnippetGenerator, Error> {
        let registry = ResolvedSemconvRegistry::try_from_registry_repo(
            registry_repo,
            diag_msgs,
            follow_symlinks,
            include_unreferenced,
        )?;
        Ok(SnippetGenerator {
            lookup: registry,
            template_engine,
        })
    }

    /// Constructs a new SnippetGenerator for the v1 schema with the given template engine.
    #[must_use]
    pub fn new(registry: ResolvedTelemetrySchema, template_engine: TemplateEngine) -> Self {
        Self {
            lookup: ResolvedSemconvRegistry { schema: registry },
            template_engine,
        }
    }
}

/// The resolved Semantic Convention repository that is used to drive snippet generation.
struct ResolvedSemconvRegistry {
    schema: ResolvedTelemetrySchema,
}

impl ResolvedSemconvRegistry {
    /// Resolve semconv registry (possibly from git), and make it available for rendering.
    fn try_from_registry_repo(
        registry_repo: &RegistryRepo,
        diag_msgs: &mut DiagnosticMessages,
        follow_symlinks: bool,
        include_unreferenced: bool,
    ) -> Result<ResolvedSemconvRegistry, Error> {
        let loaded = match SchemaResolver::load_semconv_repository(
            registry_repo.clone(),
            follow_symlinks,
        ) {
            WResult::Ok(semconv_specs) => semconv_specs,
            WResult::OkWithNFEs(semconv_specs, errs) => {
                diag_msgs.extend_from_vec(errs.into_iter().map(DiagnosticMessage::new).collect());
                semconv_specs
            }
            WResult::FatalErr(err) => return Err(err.into()),
        };

        let schema = match SchemaResolver::resolve(loaded, include_unreferenced) {
            WResult::Ok(schema) => schema,
            WResult::OkWithNFEs(schema, errs) => {
                diag_msgs.extend_from_vec(errs.into_iter().map(DiagnosticMessage::new).collect());
                schema
            }
            WResult::FatalErr(err) => return Err(err.into()),
        };
        Ok(ResolvedSemconvRegistry { schema })
    }

    fn catalog(&self) -> &Catalog {
        &self.schema.catalog
    }

    fn find_group(&self, id: &str) -> Option<&Group> {
        self.schema.registry.groups.iter().find(|g| g.id == id)
    }
}

impl MarkdownSnippetGenerator for SnippetGenerator {
    // TODO - move registry base url into state of the struct...
    fn generate_markdown_snippet(
        &self,
        args: GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error> {
        // TODO - define context.
        let snippet_type = if args.is_metric_table() {
            SnippetType::MetricTable
        } else {
            SnippetType::AttributeTable
        };
        let group = self
            .lookup
            .find_group(&args.id)
            .ok_or(Error::GroupNotFound {
                id: args.id.clone(),
            })
            .and_then(|g| Ok(ResolvedGroup::try_from_resolved(g, self.lookup.catalog())?))?;
        // Context is the JSON sent to the jinja template engine.
        let context = MarkdownSnippetContext {
            group: group.clone(),
            snippet_type,
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
        _: crate::parser::WeaverGenerateMarkdownArgs,
    ) -> Result<String, Error> {
        Err(Error::WeaverSnippetNotSupported)
    }
}

/// This struct is passed into markdown snippets for generation.
#[derive(Serialize)]
struct MarkdownSnippetContext {
    group: ResolvedGroup,
    snippet_type: SnippetType,
    tag_filter: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attribute_registry_base_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SnippetType {
    AttributeTable,
    MetricTable,
}

#[cfg(test)]
mod tests {
    use crate::v1::SnippetGenerator;
    use crate::{Error, MarkdownSnippetGenerator};
    use serde_yaml::Value;
    use weaver_common::diagnostic::DiagnosticMessages;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_forge::config::{Params, WeaverConfig};
    use weaver_forge::file_loader::FileSystemFileLoader;
    use weaver_forge::TemplateEngine;
    use weaver_semconv::registry_repo::RegistryRepo;

    fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_template_engine() -> Result<(), Error> {
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
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data".to_owned(),
        };
        let mut diag_msgs = DiagnosticMessages::empty();
        let registry_repo = RegistryRepo::try_new("main", &registry_path)?;
        let generator = SnippetGenerator::try_from_registry_repo(
            &registry_repo,
            template,
            &mut diag_msgs,
            false,
            false,
        )?;
        let attribute_registry_url = "/docs/attributes-registry";
        // Now we should check a snippet.
        let test = "data/templates.md";
        println!("--- Running template engine test: {test} ---");
        force_print_error(generator.update_markdown(test, true, Some(attribute_registry_url)));
        Ok(())
    }
}
