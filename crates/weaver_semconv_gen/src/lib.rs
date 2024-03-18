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

use weaver_logger::Logger;
use weaver_resolver::SchemaResolver;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_resolved_schema::registry::{Group, Registry};
use weaver_resolved_schema::attribute::{Attribute, AttributeRef};
use weaver_semconv::SemConvRegistry;
use weaver_semconv::attribute::{AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel, TemplateTypeSpec, ValueSpec};
use weaver_semconv::group::{GroupType, InstrumentSpec};
use itertools::Itertools;
use std::fs;
use std::fmt::Write;

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

    /// Errors from using std fmt library.
    #[error(transparent)]
    StdFmtError(#[from] std::fmt::Error),

    /// Errors from using weaver_semconv.
    #[error(transparent)]
    SemconvError(#[from] weaver_semconv::Error),

    /// Errors from using weaver_resolver.
    #[error(transparent)]
    ResolverError(#[from] weaver_resolver::Error),
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
    /// Returns true if a metric table should be rendered.
    fn is_metric_table(&self) -> bool {
        self.args.iter().any(|a| match a {
            MarkdownGenParameters::MetricTable => true,
            _ => false,
        })
    }
}


// The size a string is allowed to be before it is pushed into notes.
const BREAK_COUNT: usize = 50;

/// Context around the generation of markdown that we use to avoid conflicts
/// between multiple templates within the same markdown file.
#[derive(Default)]
struct GenerateMarkdownContext {
    /// The notes that have been added to the current markdown snippet.
    notes: Vec<String>
}

impl GenerateMarkdownContext {
    /// Adds a note to the context and returns a link to its index.
    fn add_note(&mut self, note: String) -> String {
        self.notes.push(note);
        let idx = self.notes.len();
        format!("[{idx}]")
    }

    /// Returns a string which redners the markdown notes.
    fn rendered_notes(&self) -> String {
        let mut result = String::new();
        for (counter, note) in self.notes.iter().enumerate() {
            result.push_str(&format!("\n**[{}]:** {}\n", counter+1, note.trim()));
        }
        result
    }

    /// Renderes stored notes into markdown format.
    fn write_rendered_notes<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        for (counter, note) in self.notes.iter().enumerate() {
            write!(out, "\n**[{}]:** {}\n", counter+1, note.trim())?;
        }
        Ok(())
    }
}


/// Constructs a markdown snippet (without header/closer)
fn generate_markdown_snippet<'a>(lookup: &ResolvedSemconvRegistry, args: GenerateMarkdownArgs) -> Result<String, Error> {
    let mut ctx = GenerateMarkdownContext::default();
    let mut result = String::new();
    if args.is_metric_table() {
        let view = MetricView::try_new(args.id.as_str(), lookup)?;
        view.generate_markdown(&mut result, &mut ctx)?;
    } else {
        let other = AttributeTableView::try_new(args.id.as_str(), lookup)?;        
        other.generate_markdown(&mut result, &args, &mut ctx)?;
    }
    Ok(result)
}


// TODO - This entire function could be optimised and reworked.
fn update_markdown_contents<'a>(contents: &str, lookup: &ResolvedSemconvRegistry) -> Result<String, Error> {
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
                let snippet = generate_markdown_snippet(lookup, arg)?;
                result.push_str(&snippet);
            }
        }
    }
    Ok(result)
}

/// Updates a single markdown file using the resolved schema.
pub fn update_markdown<'a>(file: &str,
                       lookup: &ResolvedSemconvRegistry,
                       dry_run: bool) -> Result<(), Error> {
    // TODO - throw error.
    let original_markdown = fs::read_to_string(file).expect("Unable to read file");
    let updated_markdown = update_markdown_contents(&original_markdown, lookup)?;
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

fn write_example_list<Out: Write, Element: std::fmt::Display>(out: &mut Out, list: &Vec<Element>) -> Result<(), Error> {
    let mut first = true;
    for e in list {
        if ! first {
            write!(out, "; ")?;
        }
        write!(out, "`{e}`")?;
        first = false;
    }
    Ok(())
}

fn write_examples_string<Out: Write>(out: &mut Out, examples: &Examples) -> Result<(), Error> {
    match examples {
        Examples::Bool(value) => Ok(write!(out, "`{value}`")?),
        Examples::Int(value) => Ok(write!(out, "`{value}`")?),
        Examples::Double(value) => Ok(write!(out, "`{value}`")?),
        Examples::String(value) => Ok(write!(out, "`{value}`")?),
        Examples::Ints(values) => write_example_list(out, values),
        Examples::Doubles(values) => write_example_list(out, values),
        Examples::Bools(values) => write_example_list(out, values),
        Examples::Strings(values) => write_example_list(out, values),
    }
}

fn write_enum_value_string<Out: Write>(out: &mut Out, value: &ValueSpec) -> Result<(), Error> {
    match value {
        ValueSpec::Double(v) => write!(out, "`{v}`")?,
        ValueSpec::Int(v) => write!(out, "`{v}`")?,
        ValueSpec::String(v) => write!(out, "`{v}`")?,
    }
    Ok(())
}

fn write_enum_examples_string<Out: Write>(out: &mut Out, members: &Vec<EnumEntriesSpec>) -> Result<(), Error> {
    let mut first = true;
    for entry in members {
        if !first {
            write!(out, "; ")?;
        }
        write_enum_value_string(out, &entry.value)?;
        first = false;
    }
    Ok(())
}


struct AttributeView<'a> {
    attribute: &'a Attribute,
}

/// Helper method to write markdown of attributes.
impl <'a> AttributeView<'a> {

    fn write_name<T : Write>(&self, out: &mut T) -> Result<(), Error> {
        match &self.attribute.r#type {
            AttributeType::Template(_) => Ok(write!(out, "{}.<key>", self.attribute.name)?),
            _ => Ok(write!(out, "{}", self.attribute.name)?),
        }
    }

    fn write_registry_link<T : Write>(&self, out: &mut T) -> Result<(), Error> {
        let reg_name = self.attribute.name.split(".").next().unwrap_or("");
        // TODO - the existing build-tools semconv will look at currently
        // generating markdown location to see if it's the same structure
        // as where the attribute originated from.
        //
        // Going forward, link vs. not link should be an option in generation.
        // OR we should move this to a template-render scenario.
        Ok(write!(out, "../attributes-registry/{reg_name}.md")?)
    }

    fn write_name_with_optional_link<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write!(out, "[`")?;
        self.write_name(out)?;
        write!(out, "`](")?;
        self.write_registry_link(out)?;
        write!(out, ")")?;
        Ok(())
    }

    fn is_enum(&self) -> bool {
        match &self.attribute.r#type {
            AttributeType::Enum{..} => true,
            _ => false,
        }
    }

    fn write_enum_spec_table<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write!(out, "\n| Value  | Description |\n|---|---|\n")?;
        match &self.attribute.r#type {
            AttributeType::Enum{members,..} =>
            for m in members {
                write!(out, "| ")?;
                write_enum_value_string(out, &m.value)?;
                write!(out, " | ")?;
                match m.brief.as_ref() {
                    Some(v) => write!(out, "{}", v.trim())?,
                    None => (),
                }
                write!(out, " |\n")?;
            }
            _ => (),
        }
        Ok(())
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

    fn write_type_string<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write!(out, "{}", self.type_string())?;
        Ok(())
    }

    fn write_description<Out: Write>(&self, out: &mut Out, ctx: &mut GenerateMarkdownContext) -> Result<(), Error> {
        if self.attribute.note.is_empty() {
            write!(out, "{}", self.attribute.brief.trim())?;
            Ok(())
        } else {
            write!(out, "{} {}", self.attribute.brief.trim(), ctx.add_note(self.attribute.note.clone()))?;
            Ok(())
        }
    }

    fn write_requirement<Out: Write>(&self, out: &mut Out, ctx: &mut GenerateMarkdownContext) -> Result<(), Error> {
        match &self.attribute.requirement_level {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => Ok(write!(out, "Required")?),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => Ok(write!(out, "Recommended")?),
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => Ok(write!(out, "Opt-In")?),
            RequirementLevel::ConditionallyRequired { text } => {
                if text.len() > BREAK_COUNT {
                    Ok(write!(out, "Conditionally Required: {}", ctx.add_note(text.clone()))?)
                } else {
                    Ok(write!(out, "Conditionally Required: {text}")?)
                }
            },
            RequirementLevel::Recommended { text } => {
                if text.len() > BREAK_COUNT {
                    Ok(write!(out, "Recommended: {}", ctx.add_note(text.clone()))?)
                } else {
                    Ok(write!(out, "Recommended: {text}")?)
                }
            },
        }
    }

    fn write_examples<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        match &self.attribute.examples {
            Some(examples) => write_examples_string(out, examples),
            None => 
                // Enums can pull examples from the enum if not otherwise specified.
                match &self.attribute.r#type {
                    AttributeType::Enum{members, ..} => write_enum_examples_string(out, members),
                    _ => Ok(()),
            },
        }
    }
}

struct AttributeTableView<'a> {
    group: &'a Group,
    lookup: &'a ResolvedSemconvRegistry,
}

impl <'a> AttributeTableView<'a> {
    pub fn try_new(id: &str, lookup: &'a ResolvedSemconvRegistry) -> Result<AttributeTableView<'a>, Error> {
        match lookup.find_group(id)  {
            Some(group) => Ok(AttributeTableView{group, lookup}),
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
        .filter_map(|attr| self.lookup.attribute(attr))
    }

    fn generate_markdown<Out: Write>(&self, out: &mut Out, args: &GenerateMarkdownArgs, ctx: &mut GenerateMarkdownContext) -> Result<(), Error> {        
        if self.group.r#type == GroupType::Event {
            write!(out, "The event name MUST be `{}`\n\n", self.event_name())?;
        }

        // TODO - deal with
        // - local / full (do we still support this?)
        // - tag filter

        if args.is_omit_requirement() {
            write!(out, "| Attribute  | Type | Description  | Examples  |\n")?;
            write!(out, "|---|---|---|---|\n")?;
        } else {
            // TODO - we should use link version and update tests/semconv upstream.
            //result.push_str("| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) |\n");
            write!(out, "| Attribute  | Type | Description  | Examples  | Requirement Level |\n")?;
            write!(out, "|---|---|---|---|---|\n")?;
        }

        
        for attr in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .dedup_by(|x,y| x.name == y.name)
                    .map(|attribute| AttributeView { attribute }) {
                write!(out, "| ")?;
                attr.write_name_with_optional_link(out)?;
                write!(out, " | ")?;
                attr.write_type_string(out)?;
                write!(out, " | ")?;
                attr.write_description(out, ctx)?;
                write!(out, " | ")?;
                attr.write_examples(out)?;
            if args.is_omit_requirement() {
                write!(out, " |\n")?;
            } else {
                write!(out, " | ")?;
                attr.write_requirement(out, ctx)?;
                write!(out, " |\n")?;
            }
        }
        // Add "note" footers
        ctx.write_rendered_notes(out)?;


        // Add sampling relevant callouts.
        let sampling_relevant: Vec<AttributeView> =
          self.attributes()
          .filter(|a| a.sampling_relevant.unwrap_or(false))
          .map(|attribute| AttributeView { attribute })
          .collect();
        if sampling_relevant.len() > 0 {
            write!(out, "\nThe following attributes can be important for making sampling decisions ")?;
            write!(out, "and SHOULD be provided **at span creation time** (if provided at all):\n\n")?;
            for a in sampling_relevant {
                // TODO - existing output uses registry-link-name.
                write!(out, "* ")?;
                a.write_name_with_optional_link(out)?;
                write!(out, "\n")?;
            }
        }

        // Add enum footers
        for e in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .dedup_by(|x,y| x.name == y.name)
                    .map(|attribute| AttributeView { attribute })
                    .filter(|a| a.is_enum()) {
           write!(out, "\n`")?;
           e.write_name(out)?;
           write!(out, "` has the following list of well-known values. If one of them applies, then the respective value MUST be used, otherwise a custom value MAY be used.\n")?;
          e.write_enum_spec_table(out)?;
        }
        Ok(())
    }
}


struct MetricView<'a> {
    group: &'a Group,
    // metric: &'a Metric,
}
impl <'a> MetricView<'a> {

    pub fn try_new(id: &str, lookup: &'a ResolvedSemconvRegistry) -> Result<MetricView<'a>, Error> {

        // TODO - we first must look up a MetricRef(index),
        // then pull rom scheam.catalog.metrics[index]

        let metric =
            lookup.find_group(id)
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
    fn write_unit<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        match self.group.unit.as_ref() {
            Some(value) => write!(out, "{value}")?,
            None => write!(out, "1")?,
        }
        Ok(())
    }
    fn write_description<Out: Write>(&self, out: &mut Out, ctx: &mut GenerateMarkdownContext) -> Result<(), Error> {
        // TODO - add note if needed.
        if self.group.note.is_empty() {
            write!(out, "{}", &self.group.brief)?
        } else {
            write!(out, "{} {}", &self.group.brief, ctx.add_note(self.group.note.clone()))?
        }
        Ok(())
    }
    pub fn generate_markdown<Out: Write>(&self, out: &mut Out, ctx: &mut GenerateMarkdownContext) -> Result<(), Error> {
        write!(out, "| Name     | Instrument Type | Unit (UCUM) | Description    |\n")?;
        write!(out, "| -------- | --------------- | ----------- | -------------- |\n")?;
        write!(out, "| `{}` | {} | `", self.metric_name(), self.instrument())?;
        self.write_unit(out)?;
        write!(out, "` | ")?;
        self.write_description(out, ctx)?;
        write!(out, " |\n")?;
        // Add "note" footers
        ctx.write_rendered_notes(out)?;
        Ok(())
    }
}

/// The resolved Semantic Convention repository that is used to drive snipper generation.
pub struct ResolvedSemconvRegistry {
    schema: ResolvedTelemetrySchema,
    registry_id: String,
}
impl ResolvedSemconvRegistry {
    /// Resolve the semantic convention registry and make it available for rendering markdown snippets.
    pub fn try_from_path(path_pattern: &str, log: impl Logger + Clone + Sync) -> Result<ResolvedSemconvRegistry, Error> {
        let registry_id = "semantic_conventions";
        let mut registry =
            SemConvRegistry::try_from_path(registry_id, path_pattern)?;
        let schema =
            SchemaResolver::resolve_semantic_convention_registry(&mut registry, log)?;
        let lookup = ResolvedSemconvRegistry { schema, registry_id: registry_id.into()};
        Ok(lookup)
    }

    fn my_registry(&self) -> Option<&Registry> {
        self.schema.registry(self.registry_id.as_str())
    }

    fn find_group(&self, id: &str) -> Option<&Group> {
        self.my_registry().and_then(|r| {
            r.groups.iter().find(|g| g.id == id)
        })
    }

    /// Finds an attribute by reference.
    fn attribute(&self, attr: &AttributeRef) -> Option<&Attribute> {
        self.schema.catalog.attribute(attr)
    }
}


#[cfg(test)]
mod tests {
    use weaver_logger::TestLogger;

    use crate::{update_markdown,Error, ResolvedSemconvRegistry};

    fn force_print_error<T>(result: Result<T, Error>) -> T {
        match result {
            Err(err) => panic!("{}", err),
            Ok(v) => v,
        }
    }

    #[test]
    fn test_http_semconv() -> Result<(), Error> {
        let logger = TestLogger::default();
        let lookup = ResolvedSemconvRegistry::try_from_path("data/**/*.yaml", logger.clone())?;


        // Check our test files.
        force_print_error(update_markdown("data/http-span-full-attribute-table.md", &lookup, true));
        force_print_error(update_markdown("data/http-metric-semconv.md", &lookup, true));
        Ok(())
    }
}