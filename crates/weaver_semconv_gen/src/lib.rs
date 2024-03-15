// SPDX-License-Identifier: Apache-2.0

//! This crate will generate code for markdown files.
//! The entire crate is a rush job to catch feature parity w/ existing python tooling by
//! poorly porting the code into RUST.  We expect to optimise and improve things over time.

#![deny(
    missing_docs,
    clippy::print_stdout,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    unused_extern_crates
)]

use serde_json::Value;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolved_schema::registry::{Group, Registry};
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::metric::{Metric,Instrument};
use weaver_semconv::attribute::{AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel, TemplateTypeSpec, ValueSpec};
use weaver_semconv::group::{GroupType, InstrumentSpec};
use itertools::Itertools;
use std::fs;

mod parser;
mod diff;


/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug)]
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
    #[error("Markdown is not equal:\n{}", diff::diff_output(.original, .updated))]
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
        /// Markdown snippet identifer <!-- semconv {header} -->
        header: String,
    },
    /// Errors from using std io library.
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
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
/// 
pub struct GenerateMarkdownArgs {
    /// The id of the metric, event, span or attribute group to render.
    id: String,
    /// Arguments the user specified that we've parsed.
    args: Vec<MarkdownGenParameters>,
}
impl GenerateMarkdownArgs {
    /// TODO
    fn is_full(&self) -> bool {
        self.args.iter().any(|a| match a {
            MarkdownGenParameters::Full => true,
            _ => false,
        })
    }
    /// Returns true if the omit requirement level flag was specified.
    fn is_omit_requirement(&self) -> bool {
        self.args.iter().any(|a| match a {
            MarkdownGenParameters::OmitRequirementLevel => true,
            _ => false,
        })
    }
    /// TODO
    fn is_metric_table(&self) -> bool {
        self.args.iter().any(|a| match a {
            MarkdownGenParameters::MetricTable => true,
            _ => false,
        })
    }
}

/// Context around the generation of markdown that we use to avoid conflicts
/// between multiple templates within the same markdown file.
#[derive(Default)]
struct GenerateMarkdownContext {
    notes: Vec<String>
}

// The size a string is allowed to be before it is pushed into notes.
const BREAK_COUNT: usize = 50;

impl GenerateMarkdownContext {
    /// Adds a note to the context and returns a link to its index.
    fn add_note(&mut self, note: String) -> String {
        self.notes.push(note);
        let idx = self.notes.len();
        format!("[{idx}]")
    }

    fn rendered_notes(&self) -> String {
        let mut result = String::new();
        for (counter, note) in self.notes.iter().enumerate() {
            result.push_str(&format!("\n**[{}]:** {}\n", counter+1, note.trim()));
        }
        result
    }
}


/// Constructs a markdown snippet (without header/closer)
fn generate_markdown_snippet(schema: &ResolvedTelemetrySchema, args: GenerateMarkdownArgs) -> Result<String, Error> {
    let mut ctx = GenerateMarkdownContext::default();
    if args.is_metric_table() {
        let view = MetricView::try_new(args.id.as_str(), schema)?;
        Ok(view.generate_markdown(&mut ctx))
    } else {
        let other = AttributeTableView::try_new(args.id.as_str(), schema)?;
        Ok(other.generate_markdown(&args, &mut ctx)?)
    }
}


// TODO - This entire function could be optimised and reworked.
fn update_markdown_contents(contents: &str, schema: &ResolvedTelemetrySchema) -> Result<String, Error> {
    let mut result = String::new();
    let mut handling_snippet = false;
    for line in contents.lines() {
        if handling_snippet {
            if parser::is_semconv_trailer(line) {
                result.push_str(line);
                // TODO - do we always need this or did we trim oddly?
                result.push_str("\n");
                handling_snippet = false;
            }
        } else {
            // Always push this line.
            result.push_str(line);
            // TODO - don't do this on last line.
            result.push_str("\n");
            // Check to see if line matches snippet request.
            // If so, generate the snippet and continue.
            if parser::is_markdown_snippet_directive(line) {
                handling_snippet = true;
                let arg = parser::parse_markdown_snippet_directive(line)?;
                let snippet = generate_markdown_snippet(&schema, arg)?;
                result.push_str(&snippet);
            }
        }
    }
    Ok(result)
}

/// Updates a single markdown file using the resolved schema.
pub fn update_markdown(file: &str, 
                       schema: &ResolvedTelemetrySchema,
                       dry_run: bool) -> Result<(), Error> {
    // TODO - throw error.
    let original_markdown = fs::read_to_string(file).expect("Unable to read file");
    let updated_markdown = update_markdown_contents(&original_markdown, schema)?;
    if !dry_run {
        fs::write(file, updated_markdown)?;
        Ok(())
    } else {
        if original_markdown != updated_markdown {
            Err(Error::MarkdownIsNotEqual {
                original: original_markdown,
                updated: updated_markdown,
            })
        } else {
            Ok(())
        }
    }
}


/// Determines an enum's type by the type of its values.
fn enum_type_string(members: &Vec<EnumEntriesSpec>) -> &'static str {
    match members.as_slice() {
        [first, ..] => match first.value {
            ValueSpec::Double(_) => "double",
            ValueSpec::Int(_) => "int",
            ValueSpec::String(_) => "string",
        },
        // TODO - is this a failure scenario?
        _ => "enum",
    }
}

fn examples_string(examples: &Examples) -> String {
    match examples {
        Examples::Bool(value) => format!("`{value}`"),
        Examples::Int(value) => format!("`{value}`"),
        Examples::Double(value) => format!("`{value}`"),
        Examples::String(value) => format!("`{value}`"),
        Examples::Ints(values) => values.iter().map(|v| format!("`{v}`")).join("; "),
        Examples::Doubles(values) => values.iter().map(|v| format!("`{v}`")).join("; "),
        Examples::Bools(values) => values.iter().map(|v| format!("`{v}`")).join("; "),
        Examples::Strings(values) => values.iter().map(|v| format!("`{v}`")).join("; "),
    }
}

fn enum_value_string(value: &ValueSpec) -> String {
    match value {
        ValueSpec::Double(v) => format!("`{v}`"),
        ValueSpec::Int(v) => format!("`{v}`"),
        ValueSpec::String(v) => format!("`{v}`"),
    }
}

fn enum_examples_string(members: &Vec<EnumEntriesSpec>) -> String {
    members.iter().map(|entry| enum_value_string(&entry.value)).join(";")
}


struct AttributeView<'a> {
    attribute: &'a Attribute,
}

impl <'a> AttributeView<'a> {

    fn name(&self) -> String {
        // Templates have `.<key>` after them.
        match &self.attribute.r#type {
            AttributeType::Template(_) => format!("{}.<key>", self.attribute.name),
            _ => self.attribute.name.clone(),
        }
    }

    fn attribute_registry_link(&self) -> String {
        let reg_name = self.attribute.name.split(".").next().unwrap_or("");
        // TODO - the existing build-tools semconv will look at currently
        // generating markdown location to see if it's the same structure
        // as where the attribute originated from.
        //
        // Going forward, link vs. not link should be an option in generation.
        // OR we should move this to a template-render scenario.
        format!("../attributes-registry/{reg_name}.md")
    }

    fn name_with_optional_link(&self) -> String {
        
        let name = self.name();
        let rel_path = self.attribute_registry_link();
        format!("[`{name}`]({rel_path})")
    }

    fn is_enum(&self) -> bool {
        match &self.attribute.r#type {
            AttributeType::Enum{..} => true,
            _ => false,
        }
    }

    fn enum_spec_values(&self) -> Vec<(String,String)> {
        match &self.attribute.r#type {
            AttributeType::Enum{members,..} => 
              members.iter()
              .map(|m| (enum_value_string(&m.value), m.brief.clone().unwrap_or("".to_string())))
              .collect(),
            _ => vec!(),
        }
    }

    fn type_string(&self) -> &'static str {
        match &self.attribute.r#type {
            AttributeType::Enum{members, ..} => enum_type_string(members),
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean) => "boolean",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans) => "boolean[]",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int) => "int",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints) => "int[]",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double) => "double",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles) => "double[]",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String) => "string",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings) => "string[]",
            AttributeType::Template(TemplateTypeSpec::Boolean) => "boolean",
            AttributeType::Template(TemplateTypeSpec::Booleans) => "boolean[]",
            AttributeType::Template(TemplateTypeSpec::Int) => "int",
            AttributeType::Template(TemplateTypeSpec::Ints) => "int[]",
            AttributeType::Template(TemplateTypeSpec::Double) => "double",
            AttributeType::Template(TemplateTypeSpec::Doubles) => "double[]",
            AttributeType::Template(TemplateTypeSpec::String) => "string",
            AttributeType::Template(TemplateTypeSpec::Strings) => "string[]",
          }
    }

    fn description(&self, ctx: &mut GenerateMarkdownContext) -> String {
        if self.attribute.note.is_empty() {
            self.attribute.brief.trim().to_string()
        } else {
            format!("{} {}", self.attribute.brief.trim(), ctx.add_note(self.attribute.note.clone()))
        }
    }

    fn requirement(&self, ctx: &mut GenerateMarkdownContext) -> String {
        match &self.attribute.requirement_level {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => "Required".to_string(),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => "Recommended".to_string(),
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => "Opt-In".to_string(),
            RequirementLevel::ConditionallyRequired { text } => {
                if text.len() > BREAK_COUNT {
                    format!("Conditionally Required: {}", ctx.add_note(text.clone()))
                } else {
                    format!("Conditionally Required: {text}")
                }
            },
            RequirementLevel::Recommended { text } => {
                if text.len() > BREAK_COUNT {
                    format!("Recommended: {}", ctx.add_note(text.clone()))
                } else {
                    format!("Recommended: {text}")
                }
            },
        }
    }

    fn examples(&self) -> String {
        match &self.attribute.examples {
            Some(examples) => examples_string(examples),
            None => 
                // Enums can pull examples from the enum if not otherwise specified.
                match &self.attribute.r#type {
                    AttributeType::Enum{members, ..} => enum_examples_string(members),
                    _ => "".to_string(),
            },
        }
    }
}

struct AttributeTableView<'a> {
    group: &'a Group,
    schema: &'a ResolvedTelemetrySchema,
}

impl <'a> AttributeTableView<'a> {
    pub fn try_new(id: &str, schema: &'a ResolvedTelemetrySchema) -> Result<AttributeTableView<'a>, Error> {
        let opt_group = schema.registries.iter().find_map(|r| {
            r.groups.iter().find(|g| g.id == id)
        });
        match opt_group  {
            Some(group) => Ok(AttributeTableView{group, schema}),
            None => Err(Error::GroupNotFound { id: id.to_string() }),
        }
    }

    fn event_name(&self) -> &str {
        // TODO - exception if group is not an event.
        match &self.group.name {
            Some(value) => value.as_str(),
            None => 
              // TODO - exception if prefix is empty.
              self.group.prefix.as_str(),
        }
    }

    fn attributes(&self) -> impl Iterator<Item=&Attribute>{
        self.group.attributes.iter()
        .map(|a_ref| &self.schema.catalog.attributes[a_ref.0 as usize])
    }

    fn generate_markdown(&self, args: &GenerateMarkdownArgs, ctx: &mut GenerateMarkdownContext) -> Result<String, Error> {        
        let mut result = String::new();
        if self.group.r#type == GroupType::Event {
            result.push_str(&format!("The event name MUST be `{}`\n\n", self.event_name()))
        }

        // TODO - deal with
        // - local / full (do we still support this?)
        // - tag filter

        if args.is_omit_requirement() {
            result.push_str("| Attribute  | Type | Description  | Examples  |\n");
            result.push_str("|---|---|---|---|\n");
        } else {
            // TODO - we should use link version and udpate tests/semconv upstream.
            //result.push_str("| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) |\n");
            result.push_str("| Attribute  | Type | Description  | Examples  | Requirement Level |\n");
            result.push_str("|---|---|---|---|---|\n");
        }

        
        for attr in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .dedup_by(|x,y| x.name == y.name)
                    .map(|attribute| AttributeView { attribute }) {
            if args.is_omit_requirement() {
                result.push_str(&format!("| {} | {} | {} | {} |\n",
                                        attr.name_with_optional_link(),
                                        attr.type_string(),
                                        attr.description(ctx),
                                        attr.examples()));
            } else {
                result.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                                        attr.name_with_optional_link(),
                                        attr.type_string(),
                                        attr.description(ctx),
                                        attr.examples(),
                                        attr.requirement(ctx)));
            }
        }
        // Add "note" footers
        result.push_str(&ctx.rendered_notes());


        // Add sampling relevant callouts.
        let sampling_relevant: Vec<AttributeView> =
          self.attributes()
          .filter(|a| a.sampling_relevant.unwrap_or(false))
          .map(|attribute| AttributeView { attribute })
          .collect();
        if sampling_relevant.len() > 0 {
            result.push_str("\nThe following attributes can be important for making sampling decisions ");
            result.push_str("and SHOULD be provided **at span creation time** (if provided at all):\n\n");
            for a in sampling_relevant {
                // TODO - existing output uses registry-link-name.
                result.push_str(&format!("* {}\n", a.name_with_optional_link()))
            }
        }

        // Add enum footers
        for e in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .dedup_by(|x,y| x.name == y.name)
                    .map(|attribute| AttributeView { attribute })
                    .filter(|a| a.is_enum()) {
           result.push_str("\n`");
           result.push_str(&e.name());
           result.push_str("` has the following list of well-known values. If one of them applies, then the respective value MUST be used, otherwise a custom value MAY be used.\n");
           result.push_str("\n| Value  | Description |\n|---|---|\n");
           // TODO - enum table.
           for (value, description) in e.enum_spec_values() {
            result.push_str(&format!("| {} | {} |\n", value, description.trim()));
           }
        }
        Ok(result)
    }
}


struct MetricView<'a> {
    group: &'a Group,
    // metric: &'a Metric,
}
impl <'a> MetricView<'a> {

    pub fn try_new(id: &str, schema: &'a ResolvedTelemetrySchema) -> Result<MetricView<'a>, Error> {

        // TODO - we first must look up a MetricRef(index),
        // then pull rom scheam.catalog.metrics[index]

        let metric =
            schema.registries.iter().find_map(|r| {
                r.groups.iter().find(|g| g.id == id)
            })
            .filter(|g| g.r#type == GroupType::Metric);
            // TODO - Since metric isn't working, we just use group here.
            // .map(|g| {
            //     println!("Looking for metric {:?} in catalog!", g.metric_name.as_ref());
            //     schema.catalog.metrics.iter().find(|m| &m.name == g.metric_name.as_ref().unwrap())
            // }).flatten();

        match metric {
            Some(group) => Ok(MetricView{group}),
            None => Err(Error::GroupMustBeMetric { id: id.to_string() }),
        }
    }

    fn metric_name(&self) -> &str {
        self.group.metric_name.as_ref().map(|r| r.as_ref()).unwrap_or("")
    }
    fn instrument(&self) -> &'static str {        
        match self.group.instrument {
            Some(InstrumentSpec::UpDownCounter) => "UpDownCounter",
            Some(InstrumentSpec::Counter) => "Counter",
            Some(InstrumentSpec::Gauge) => "Gauge",
            Some(InstrumentSpec::Histogram) => "Histogram",
            None => "Unknown",
        }
    }
    fn unit(&self) -> &str {
        self.group.unit.as_ref().map(|x| x.as_str()).unwrap_or("1")
    }
    fn description(&self, ctx: &mut GenerateMarkdownContext) -> String {
        // TODO - add note if needed.
        if self.group.note.is_empty() {
            self.group.brief.clone()
        } else {
            format!("{} {}", &self.group.brief, ctx.add_note(self.group.note.clone()))
        }
    }

    // TODO - Does this belong here?
    pub fn generate_markdown(&self, ctx: &mut GenerateMarkdownContext) -> String {
        let mut result = String::new();
        result.push_str("| Name     | Instrument Type | Unit (UCUM) | Description    |\n");
        result.push_str("| -------- | --------------- | ----------- | -------------- |\n");
        result.push_str(&format!("| `{}` | {} | `{}` | {} |\n", 
          self.metric_name(),
          self.instrument(),
          self.unit(),
          self.description(ctx),
         ));

        // Add "note" footers
        result.push_str(&ctx.rendered_notes());

         result
    }
}




#[cfg(test)]
mod tests {
    use weaver_logger::TestLogger;
    use weaver_resolver::SchemaResolver;
    use weaver_semconv::SemConvRegistry;

    use crate::{update_markdown,Error};

    fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_http_semconv() -> Result<(), Error> {
        let logger = TestLogger::default();

        let mut registry =
            SemConvRegistry::try_from_path("data/**/*.yaml").expect("Failed to load registry");
        let schema =
            SchemaResolver::resolve_semantic_convention_registry(&mut registry, logger.clone())
                .expect("Failed to resolve registry");

        // Check our test files.
        force_print_error(update_markdown("data/http-span-full-attribute-table.md", &schema, true));
        force_print_error(update_markdown("data/http-metric-semconv.md", &schema, true));
        Ok(())
    }
}