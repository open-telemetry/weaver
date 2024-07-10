// SPDX-License-Identifier: Apache-2.0

//! This crate will generate code for markdown files.
//! The entire crate is a rush job to catch feature parity w/ existing python tooling by
//! poorly porting the code into RUST.  We expect to optimise and improve things over time.

use miette::Diagnostic;
use std::{fmt, fs};

use serde::Serialize;
use weaver_cache::Cache;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_diff::diff_output;
use weaver_forge::registry::ResolvedGroup;
use weaver_forge::TemplateEngine;
use weaver_resolved_schema::attribute::{Attribute, AttributeRef};
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::registry::{Group, Registry};
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolver::SchemaResolver;
use weaver_semconv::path::RegistryPath;
use weaver_semconv::registry::SemConvRegistry;

use crate::gen::{AttributeTableView, GenerateMarkdownContext, MetricView};

mod gen;
mod parser;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Thrown when we are unable to find a semconv by id.
    #[error("Could not find: {id}")]
    GroupNotFound {
        /// The id of the semconv lookup
        id: String,
    },

    /// Thrown when forcing a group to be a metricl
    #[error("Expected metric: {id}")]
    GroupMustBeMetric {
        /// The id of the semconv lookup that was not a metric.
        id: String,
    },
    /// Thrown when rendering an attribute group, but no attributes remain after filtering.
    #[error("No attributes retained for '{id}' filtering by '{filter}'")]
    GroupHasNoRenderableAttributesAfterFilter {
        /// The id of the semconv lookup.
        id: String,
        /// The filter for which attributes to display.
        filter: String,
    },
    /// Errors thrown when we are running a dry run and markdown doesn't match.
    #[error("Markdown is not equal:\n{}", diff_output(.original, .updated))]
    MarkdownIsNotEqual {
        /// Original markdown value.
        original: String,
        /// Updated markdown value.
        updated: String,
        // TODO - smart diff.
    },
    /// Thrown when snippet header is invalid.
    #[error("Could not parse snippet header: [{header}]")]
    InvalidSnippetHeader {
        /// Markdown snippet identifier <!-- semconv {header} -->
        header: String,
    },
    /// Errors from using std io library.
    #[error("{0}")]
    StdIoError(String),

    /// Errors from using std fmt library.
    #[error("{error}")]
    StdFmtError {
        /// The error message.
        error: String,
    },

    /// Errors from using weaver_semconv.
    #[error(transparent)]
    SemconvError(#[from] weaver_semconv::Error),

    /// Errors from using weaver_resolver.
    #[error(transparent)]
    ResolverError(#[from] weaver_resolver::Error),

    /// Errors from using weaver_cache.
    #[error(transparent)]
    CacheError(#[from] weaver_cache::Error),

    /// Errors from using weaver_forge.
    #[error(transparent)]
    ForgeError(#[from] weaver_forge::error::Error),

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        match error {
            Error::CompoundError(errors) => DiagnosticMessages::new(
                errors
                    .into_iter()
                    .flat_map(|e| {
                        let diag_msgs: DiagnosticMessages = e.into();
                        diag_msgs.into_inner()
                    })
                    .collect(),
            ),
            Error::SemconvError(e) => e.into(),
            Error::ResolverError(e) => e.into(),
            Error::CacheError(e) => e.into(),
            Error::ForgeError(e) => e.into(),
            _ => DiagnosticMessages::new(vec![DiagnosticMessage::new(error)]),
        }
    }
}

impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error::StdIoError(e.to_string())
    }
}

// TODO - this is based on https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/src/opentelemetry/semconv/templating/markdown/__init__.py#L503
// We can likely model this much better.
/// Parameters users can specify for generating markdown.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownGenParameters {
    /// Filter attributes to those with a given tag.
    Tag(String),
    /// Display all metrics in a group?
    Full,
    /// Generate a metric table
    MetricTable,
    /// Omit the requirement level.
    OmitRequirementLevel,
}

/// Markdown-snippet generation arguments.
pub struct GenerateMarkdownArgs {
    /// The id of the metric, event, span or attribute group to render.
    id: String,
    /// Arguments the user specified that we've parsed.
    args: Vec<MarkdownGenParameters>,
}

impl GenerateMarkdownArgs {
    // Returns true if the `full` flag was specified.
    fn is_full(&self) -> bool {
        self.args
            .iter()
            .any(|a| matches!(a, MarkdownGenParameters::Full))
    }
    /// Returns true if the `omit_requirement_level` flag was specified.
    fn is_omit_requirement(&self) -> bool {
        self.args
            .iter()
            .any(|a| matches!(a, MarkdownGenParameters::OmitRequirementLevel))
    }
    /// Returns true if a metric table should be rendered.
    fn is_metric_table(&self) -> bool {
        self.args
            .iter()
            .any(|a| matches!(a, MarkdownGenParameters::MetricTable))
    }

    /// Returns the tag filter specified, if any.  Assumes only one.
    fn tag_filter(&self) -> Option<&str> {
        self.args.iter().find_map(|arg| match arg {
            MarkdownGenParameters::Tag(value) => Some(value.as_str()),
            _ => None,
        })
    }

    /// Returns all tag filters in a list.
    fn tag_filters(&self) -> Vec<&str> {
        self.args
            .iter()
            .find_map(|arg| match arg {
                MarkdownGenParameters::Tag(value) => Some(value.as_str()),
                _ => None,
            })
            .into_iter()
            .collect()
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

// TODO - This entire function could be optimised and reworked.
fn update_markdown_contents(
    contents: &str,
    generator: &SnippetGenerator,
    attribute_registry_base_url: Option<&str>,
) -> Result<String, Error> {
    let mut result = String::new();
    let mut handling_snippet = false;
    for line in contents.lines() {
        if handling_snippet {
            if parser::is_semconv_trailer(line) {
                result.push_str(line);
                // TODO - do we always need this or did we trim oddly?
                result.push('\n');
                handling_snippet = false;
            }
        } else {
            // Always push this line.
            result.push_str(line);
            // TODO - don't do this on last line.
            result.push('\n');
            // Check to see if line matches snippet request.
            // If so, generate the snippet and continue.
            if parser::is_markdown_snippet_directive(line) {
                handling_snippet = true;
                let arg = parser::parse_markdown_snippet_directive(line)?;
                let snippet =
                    generator.generate_markdown_snippet(arg, attribute_registry_base_url)?;
                result.push_str(&snippet);
            }
        }
    }
    Ok(result)
}

/// Updates a single markdown file using the resolved schema.
pub fn update_markdown(
    file: &str,
    generator: &SnippetGenerator,
    dry_run: bool,
    attribute_registry_base_url: Option<&str>,
) -> Result<(), Error> {
    let original_markdown = fs::read_to_string(file)
        .map_err(|e| Error::StdIoError(e.to_string()))?
        .replace("\r\n", "\n");
    let updated_markdown =
        update_markdown_contents(&original_markdown, generator, attribute_registry_base_url)?;
    if !dry_run {
        fs::write(file, updated_markdown).map_err(|e| Error::StdIoError(e.to_string()))?;
        Ok(())
    } else if original_markdown != updated_markdown {
        Err(Error::MarkdownIsNotEqual {
            original: original_markdown,
            updated: updated_markdown,
        })
    } else {
        Ok(())
    }
}

/// State we need to generate markdown snippets from configuration.
pub struct SnippetGenerator {
    lookup: ResolvedSemconvRegistry,
    template_engine: Option<TemplateEngine>,
}

impl SnippetGenerator {
    // TODO - move registry base url into state of the struct...
    fn generate_markdown_snippet(
        &self,
        args: GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error> {
        if let Some(template) = &self.template_engine {
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
            let mut result =
                template.generate_snippet(&context, snippet_template_file.to_owned())?;
            result.push('\n');
            Ok(result)
        } else {
            self.generate_legacy_markdown_snippet(args, attribute_registry_base_url)
        }
    }

    fn generate_legacy_markdown_snippet(
        &self,
        args: GenerateMarkdownArgs,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<String, Error> {
        let mut ctx = GenerateMarkdownContext::default();
        let mut result = String::new();
        if args.is_metric_table() {
            let view = MetricView::try_new(args.id.as_str(), &self.lookup)?;
            view.generate_markdown(&mut result, &mut ctx)?;
        } else {
            let other = AttributeTableView::try_new(args.id.as_str(), &self.lookup)?;
            other.generate_markdown(&mut result, &args, &mut ctx, attribute_registry_base_url)?;
        }
        Ok(result)
    }

    /// Resolve semconv registry (possibly from git), and make it available for rendering.
    pub fn try_from_url(
        registry_path: RegistryPath,
        cache: &Cache,
        template_engine: Option<TemplateEngine>,
    ) -> Result<SnippetGenerator, Error> {
        let registry = ResolvedSemconvRegistry::try_from_url(registry_path, cache)?;
        Ok(SnippetGenerator {
            lookup: registry,
            template_engine,
        })
    }

    // Used in tests
    #[allow(dead_code)]
    fn try_from_path(
        path_pattern: &str,
        template_engine: Option<TemplateEngine>,
    ) -> Result<SnippetGenerator, Error> {
        let cache = Cache::try_new()?;
        Self::try_from_url(
            RegistryPath::Local {
                path_pattern: path_pattern.to_owned(),
            },
            &cache,
            template_engine,
        )
    }
}

/// The resolved Semantic Convention repository that is used to drive snipper generation.
struct ResolvedSemconvRegistry {
    schema: ResolvedTelemetrySchema,
    registry_id: String,
}

impl ResolvedSemconvRegistry {
    /// Resolve semconv registry (possibly from git), and make it available for rendering.
    fn try_from_url(
        registry_path: RegistryPath,
        cache: &Cache,
    ) -> Result<ResolvedSemconvRegistry, Error> {
        let registry_id = "semantic_conventions";
        let semconv_specs = SchemaResolver::load_semconv_specs(&registry_path, cache)?;
        let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
        let schema = SchemaResolver::resolve_semantic_convention_registry(&mut registry)?;
        let lookup = ResolvedSemconvRegistry {
            schema,
            registry_id: registry_id.into(),
        };
        Ok(lookup)
    }

    fn my_registry(&self) -> Option<&Registry> {
        self.schema.registry(self.registry_id.as_str())
    }

    fn catalog(&self) -> &Catalog {
        &self.schema.catalog
    }

    fn find_group(&self, id: &str) -> Option<&Group> {
        self.my_registry()
            .and_then(|r| r.groups.iter().find(|g| g.id == id))
    }

    /// Finds an attribute by reference.
    fn attribute(&self, attr: &AttributeRef) -> Option<&Attribute> {
        self.schema.catalog.attribute(attr)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use weaver_forge::config::Params;

    use weaver_forge::file_loader::FileSystemFileLoader;
    use weaver_forge::TemplateEngine;

    use crate::{update_markdown, Error, SnippetGenerator};

    fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_template_engine() -> Result<(), Error> {
        let loader = FileSystemFileLoader::try_new("templates/registry/markdown".into())?;
        let template = TemplateEngine::try_new(loader, Params::default())?;
        let generator = SnippetGenerator::try_from_path("data", Some(template))?;
        let attribute_registry_url = "/docs/attributes-registry";
        // Now we should check a snippet.
        let test = "data/templates.md";
        println!("--- Running template engine test: {test} ---");
        force_print_error(update_markdown(
            test,
            &generator,
            true,
            Some(attribute_registry_url),
        ));
        Ok(())
    }

    #[test]
    fn test_http_semconv() -> Result<(), Error> {
        let lookup = SnippetGenerator::try_from_path("data", None)?;
        let attribute_registry_url = "/docs/attributes-registry";
        // Check our test files.
        for test in [
            "data/http-span-full-attribute-table.md",
            "data/http-metric-semconv.md",
            "data/user-agent.md",
        ] {
            println!("--- Running test: {test} ---");
            force_print_error(update_markdown(
                test,
                &lookup,
                true,
                Some(attribute_registry_url),
            ));
        }
        Ok(())
    }

    #[test]
    fn run_legacy_tests() {
        // Note: We could update this to run all tests in parallel and join results.
        // For now we're just getting things working.
        let test_dirs = fs::read_dir("legacy_tests").unwrap();
        for dir in test_dirs.flatten() {
            if dir.path().join("test.md").exists() {
                println!();
                println!("--- Running test: {} ---", dir.path().display());
                println!();
                force_print_error(run_legacy_test(dir.path()));
            }
        }
    }

    fn run_legacy_test(path: PathBuf) -> Result<(), Error> {
        let semconv_path = format!("{}", path.display());
        let lookup = SnippetGenerator::try_from_path(&semconv_path, None)?;
        let test_path = path.join("test.md").display().to_string();
        // Attempts to update the test - will fail if there is any difference in the generated markdown.
        update_markdown(&test_path, &lookup, true, None)
    }
}
