// SPDX-License-Identifier: Apache-2.0

//! This crate will generate code for markdown files.

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
    }
}

// TODO - this is based on https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/src/opentelemetry/semconv/templating/markdown/__init__.py#L503
// We can likely model this much better.
/// Parameters users can specify for generating markdown.
#[derive(Debug)]
pub enum MarkdownGenParameters {
    /// Don't display constraints
    RemoveConstraints,
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
    /// TODO
    fn is_remove_constraint(&self) -> bool {
        self.args.iter().any(|a| match a {
            MarkdownGenParameters::RemoveConstraints => true,
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

/// TODO - doc
pub fn generate_markdown(schema: &ResolvedTelemetrySchema, args: GenerateMarkdownArgs) -> Result<String, Error> {

    if args.is_metric_table() {
        let view = MetricView::try_new(args.id.as_str(), schema)?;
        Ok(view.generate_markdown())
    } else {
        let other = AttributeTableView::try_new(args.id.as_str(), schema)?;
        Ok(other.generate_markdown(&args)?)
    }
}

// --- Existing logic to chose render type based on enums. ---
// if self.render_ctx.is_metric_table:
// self.to_markdown_metric_table(semconv, output)
// else:
//     if isinstance(semconv, EventSemanticConvention):
//         output.write(f"The event name MUST be `{semconv.name}`.\n\n")
//     self.to_markdown_attribute_table(semconv, output)

// if not self.render_ctx.is_remove_constraint:
//     for cnst in semconv.constraints:
//         self.to_markdown_constraint(cnst, output)
// self.to_markdown_enum(output)

// if isinstance(semconv, UnitSemanticConvention):
//     self.to_markdown_unit_table(semconv.members, output)


struct AttributeView<'a> {
    attribute: &'a Attribute,
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

impl <'a> AttributeView<'a> {
    fn name(&self) -> &str {
        self.attribute.name.as_str()
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

    fn description(&self) -> &str {
        self.attribute.brief.as_str() // TODO - deal with notes?
    }

    fn requirement(&self) -> String {
        // TODO - deal with notes:
        match &self.attribute.requirement_level {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => "Required".to_string(),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => "Recommended".to_string(),
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => "Opt-In".to_string(),
            // TODO - Add text to notes if it's too long.
            RequirementLevel::ConditionallyRequired { text } => format!("`Conditionally Required` {text}"),
            RequirementLevel::Recommended { text } => format!("`Recommended` {text}"),
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

    pub fn generate_markdown(&self, args: &GenerateMarkdownArgs) -> Result<String, Error> {
        let mut result = String::new();
        if self.group.r#type == GroupType::Event {
            result.push_str(&format!("The event name MUST be `{}`\n\n", self.event_name()))
        }

        // TODO - deal with
        // - local
        // - full
        // - tag filter

        result.push_str("| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) |\n");
        result.push_str("|---|---|---|---|---|\n");

        
        for attr in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .map(|attribute| AttributeView { attribute }) {
            // TODO - deal with notes.
            result.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                                     attr.name(),
                                    attr.type_string(),
                                    attr.description(),
                                    attr.examples(),
                                    attr.requirement()));
        }
        // Add "note" footers

        // Add sampling relevant callouts.
        let sampling_relevant: Vec<&str> =
          self.attributes()
          .filter(|a| a.sampling_relevant.unwrap_or(false))
          .map(|a| a.name.as_str())
          .collect();
        if sampling_relevant.len() > 0 {
            result.push_str("\nThe following attributes can be important for making sampling decisions ");
            result.push_str("and SHOULD be provided **at span creation time** (if provided at all):\n\n");
            for name in sampling_relevant {
                result.push_str(&format!(" * {name}\n"))
            }
            result.push_str("\n");
        }

        // Add enum footers
        for e in self.attributes()
                    .sorted_by_key(|a| a.name.as_str())
                    .map(|attribute| AttributeView { attribute })
                    .filter(|a| a.is_enum()) {
           result.push_str("\n");
           result.push_str(e.name());
           result.push_str(" has the following list of well-known values. If one of them applies, then the respective value MUST be used, otherwise a custom value MAY be used.\n");
           result.push_str("\n| Value  | Description |\n|---|---|\n");
           // TODO - enum table.
           for (value, description) in e.enum_spec_values() {
            result.push_str(&format!("| {value} | {description} |\n"));
           }
        }
        Ok(result)
    }
}


struct MetricView<'a> {
    metric: &'a Metric,
}
impl <'a> MetricView<'a> {

    pub fn try_new(id: &str, schema: &'a ResolvedTelemetrySchema) -> Result<MetricView<'a>, Error> {

        // TODO - we first must look up a MetricRef(index),
        // then pull rom scheam.catalog.metrics[index]

        let metric =
            schema.registries.iter().find_map(|r| {
                r.groups.iter().find(|g| g.id == id)
            })
            .filter(|g| g.r#type == GroupType::Metric)
            // TODO - Since metric isn't working, we could just use group here.
            .map(|g| {
                println!("Looking for metric {:?} in catalog!", g.metric_name.as_ref());
                schema.catalog.metrics.iter().find(|m| &m.name == g.metric_name.as_ref().unwrap())
            }).flatten();

        match metric {
            Some(metric) => Ok(MetricView{metric}),
            None => Err(Error::GroupMustBeMetric { id: id.to_string() }),
        }
    }

    fn metric_name(&self) -> &str {
        &self.metric.name
    }
    fn instrument(&self) -> &'static str {        
        match self.metric.instrument {
            Instrument::UpDownCounter => "UpDownCounter",
            Instrument::Counter => "Counter",
            Instrument::Gauge => "Gauge",
            Instrument::Histogram => "Histogram",
        }
    }
    fn unit(&self) -> &str {
        self.metric.unit.as_ref().map(|x| x.as_str()).unwrap_or("1")
    }
    fn description(&self) -> &str {
        &self.metric.brief
    }

    // TODO - Does this belong here?
    pub fn generate_markdown(&self) -> String {
        let mut result = String::new();
        result.push_str("| Name     | Instrument Type | Unit (UCUM) | Description    |\n");
        result.push_str("| -------- | --------------- | ----------- | -------------- |\n");
        result.push_str(&format!("| {} | {} | {} | {} |\n", 
          self.metric_name(),
          self.instrument(),
          self.unit(),
          self.description(),
         ));

         // TODO - Render notes.
         if !self.metric.note.is_empty() {

         }

         result
    }
}




#[cfg(test)]
mod tests {
    use weaver_logger::TestLogger;
    use weaver_resolver::SchemaResolver;
    use weaver_semconv::SemConvRegistry;

    use crate::{generate_markdown,GenerateMarkdownArgs,MarkdownGenParameters};


    #[test]
    fn test_metric_table() {
        let logger = TestLogger::default();

        let mut registry =
            SemConvRegistry::try_from_path("data/**/*.yaml").expect("Failed to load registry");
        let schema =
            SchemaResolver::resolve_semantic_convention_registry(&mut registry, logger.clone())
                .expect("Failed to resolve registry");

        let args = GenerateMarkdownArgs {
            id: "trace.http.common".into(),
            args: vec!(),
        };
        let result = generate_markdown(&schema, args).unwrap();
        println!("{}", result);
        assert_eq!(result, 
r#"| Attribute  | Type | Description  | Examples  | Requirement Level |
|---|---|---|---|---|
| request.method_original |
| request.body.size |
| response.body.size |
| http.request.method |
| network.transport |
| network.type |
| user_agent.original |
"#);

        // TODO - We're still figuring out best way to adapt existing API.
        let args = GenerateMarkdownArgs {
            id: "metric.http.server.request.duration".into(),
            args: vec!(MarkdownGenParameters::MetricTable),
        };
        let result = generate_markdown(&schema, args).unwrap();
        println!("{}", result);
        assert_eq!(result,
r#"| Name     | Instrument Type | Unit (UCUM) | Description    |
| -------- | --------------- | ----------- | -------------- |
| jvm.memory.used | UpDownCounter | By | Measure of memory used. |"#);
    }
}