// SPDX-License-Identifier: Apache-2.0

//! Markdown writing utilities.

use crate::{Error, GenerateMarkdownArgs, ResolvedSemconvRegistry};
use itertools::Itertools;
use std::fmt::Write;
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::registry::Group;
use weaver_semconv::attribute::{
    AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec,
    RequirementLevel, TemplateTypeSpec, ValueSpec,
};
use weaver_semconv::group::{GroupType, InstrumentSpec};
use weaver_semconv::stability::Stability;

// The size a string is allowed to be before it is pushed into notes.
const BREAK_COUNT: usize = 50;

/// Context around the generation of markdown that we use to avoid conflicts
/// between multiple templates within the same markdown file.
#[derive(Default)]
pub struct GenerateMarkdownContext {
    /// The notes that have been added to the current markdown snippet.
    notes: Vec<String>,
}

impl GenerateMarkdownContext {
    /// Adds a note to the context and returns a link to its index.
    fn add_note(&mut self, note: String) -> String {
        self.notes.push(note);
        let idx = self.notes.len();
        format!("[{idx}]")
    }

    /// Renders stored notes into markdown format.
    fn write_rendered_notes<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        for (counter, note) in self.notes.iter().enumerate() {
            write!(out, "\n**[{}]:** {}\n", counter + 1, note.trim())?;
        }
        Ok(())
    }
}

/// Determines an enum's type by the type of its values.
fn enum_type_string(members: &[EnumEntriesSpec]) -> &'static str {
    match members {
        [first, ..] => match first.value {
            ValueSpec::Double(_) => "double",
            ValueSpec::Int(_) => "int",
            ValueSpec::String(_) => "string",
        },
        // TODO - is this a failure scenario?
        _ => "enum",
    }
}

fn write_example_list<Out: Write, Element: std::fmt::Display>(
    out: &mut Out,
    list: &[Element],
) -> Result<(), Error> {
    let mut first = true;
    for e in list {
        if !first {
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

fn write_enum_examples_string<Out: Write>(
    out: &mut Out,
    members: &[EnumEntriesSpec],
) -> Result<(), Error> {
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

fn write_stability_badge<Out: Write>(out: &mut Out, stability: &Option<Stability>) -> Result<(), Error> {
    match stability {
        Some(Stability::Stable) => write!(out, "![Stable](https://img.shields.io/badge/-stable-lightgreen)")?,
        Some(Stability::Deprecated) => write!(out, "![Deprecated](https://img.shields.io/badge/-deprecated-red)")?,
        Some(Stability::Experimental) | None => write!(out, "![Experimental](https://img.shields.io/badge/-experimental-blue)")?,
    }
    Ok(()) 
}

struct AttributeView<'a> {
    attribute: &'a Attribute,
}

/// Helper method to write markdown of attributes.
impl<'a> AttributeView<'a> {
    fn write_name<T: Write>(&self, out: &mut T) -> Result<(), Error> {
        match &self.attribute.r#type {
            AttributeType::Template(_) => Ok(write!(out, "{}.<key>", self.attribute.name)?),
            _ => Ok(write!(out, "{}", self.attribute.name)?),
        }
    }

    fn write_registry_link<T: Write>(&self, out: &mut T, prefix: &str) -> Result<(), Error> {
        let reg_name = self.attribute.name.split('.').next().unwrap_or("");
        // TODO - We should try to link to the name itself, instead
        // of just the correct group.
        Ok(write!(out, "{prefix}/{reg_name}.md")?)
    }

    fn write_name_with_optional_link<Out: Write>(
        &self,
        out: &mut Out,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<(), Error> {
        match attribute_registry_base_url {
            Some(prefix) => {
                write!(out, "[`")?;
                self.write_name(out)?;
                write!(out, "`](")?;
                self.write_registry_link(out, prefix)?;
                write!(out, ")")?;
            }
            None => self.write_name(out)?,
        }
        Ok(())
    }

    fn is_enum(&self) -> bool {
        matches!(&self.attribute.r#type, AttributeType::Enum { .. })
    }

    fn is_sampling_relevant(&self) -> bool {
        self.attribute.sampling_relevant.unwrap_or(false)
    }

    fn has_tag(&self, tag: &str) -> bool {
        // TODO - Also handle "tags"?
        self.attribute.tag.as_ref().is_some_and(|t| t == tag)
    }

    fn write_enum_spec_table<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write!(
            out,
            "\n| Value  | Description | Stability |\n|---|---|---|\n"
        )?;
        if let AttributeType::Enum { members, .. } = &self.attribute.r#type {
            for m in members {
                write!(out, "| ")?;
                write_enum_value_string(out, &m.value)?;
                write!(out, " | ")?;
                if let Some(v) = m.brief.as_ref() {
                    write!(out, "{}", v.trim())?;
                }
                // Stability.
                write!(out, " | ")?;
                write_stability_badge(out, &m.stability)?;
                writeln!(out, " |")?;
            }
        }
        // TODO - error message on not enum?...
        Ok(())
    }

    fn type_string(&self) -> &'static str {
        match &self.attribute.r#type {
            AttributeType::Enum { members, .. } => enum_type_string(members),
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

    fn write_description<Out: Write>(
        &self,
        out: &mut Out,
        ctx: &mut GenerateMarkdownContext,
    ) -> Result<(), Error> {
        if self.attribute.note.is_empty() {
            write!(out, "{}", self.attribute.brief.trim())?;
            Ok(())
        } else {
            write!(
                out,
                "{} {}",
                self.attribute.brief.trim(),
                ctx.add_note(self.attribute.note.clone())
            )?;
            Ok(())
        }
    }

    fn write_requirement<Out: Write>(
        &self,
        out: &mut Out,
        ctx: &mut GenerateMarkdownContext,
    ) -> Result<(), Error> {
        match &self.attribute.requirement_level {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => {
                Ok(write!(out, "`Required`")?)
            }
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => {
                Ok(write!(out, "`Recommended`")?)
            }
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => Ok(write!(out, "`Opt-In`")?),
            RequirementLevel::ConditionallyRequired { text } => {
                if text.len() > BREAK_COUNT {
                    Ok(write!(
                        out,
                        "`Conditionally Required` {}",
                        ctx.add_note(text.clone())
                    )?)
                } else {
                    Ok(write!(out, "`Conditionally Required` {text}")?)
                }
            }
            RequirementLevel::Recommended { text } => {
                if text.len() > BREAK_COUNT {
                    Ok(write!(out, "`Recommended` {}", ctx.add_note(text.clone()))?)
                } else {
                    Ok(write!(out, "`Recommended` {text}")?)
                }
            }
        }
    }

    fn write_examples<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        match &self.attribute.examples {
            Some(examples) => write_examples_string(out, examples),
            None =>
            // Enums can pull examples from the enum if not otherwise specified.
            {
                match &self.attribute.r#type {
                    AttributeType::Enum { members, .. } => write_enum_examples_string(out, members),
                    _ => Ok(()),
                }
            }
        }
    }

    fn write_stability<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write_stability_badge(out, &self.attribute.stability)
    }
}

fn sort_ordinal_for_requirement(e: &RequirementLevel) -> i32 {
    // For now use ordinals from python.
    match e {
        RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => 1,
        RequirementLevel::ConditionallyRequired { .. } => 2,
        RequirementLevel::Recommended { .. } => 3,
        RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended) => 3,
        RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn) => 4,
    }
}

pub struct AttributeTableView<'a> {
    group: &'a Group,
    lookup: &'a ResolvedSemconvRegistry,
}

impl<'a> AttributeTableView<'a> {
    pub fn try_new(
        id: &str,
        lookup: &'a ResolvedSemconvRegistry,
    ) -> Result<AttributeTableView<'a>, Error> {
        match lookup.find_group(id) {
            Some(group) => Ok(AttributeTableView { group, lookup }),
            None => Err(Error::GroupNotFound { id: id.to_owned() }),
        }
    }

    fn event_name(&self) -> &str {
        // TODO - exception if group is not an event.
        match &self.group.name {
            Some(value) => value.as_str(),
            None =>
            // TODO - exception if prefix is empty.
            {
                self.group.prefix.as_str()
            }
        }
    }

    fn is_attribute_local(&self, id: &str) -> bool {
       self.lookup.is_attribute_local(&self.group.id, id)
    }

    /// Returns attributes sorted for rendering.
    /// is_full - denotes if all inhereted attributes should be included or just locally defined ones.
    fn attributes(&self, is_full: bool) -> impl Iterator<Item = AttributeView<'_>> {
        self.group
            .attributes
            .iter()
            .filter_map(|attr| self.lookup.attribute(attr))
            .filter(|a| is_full || self.is_attribute_local(&a.name))
            .sorted_by(|lhs, rhs| {
                match sort_ordinal_for_requirement(&lhs.requirement_level)
                    .cmp(&sort_ordinal_for_requirement(&rhs.requirement_level))
                {
                    // If requirement_level is the same, then we compare by string.
                    std::cmp::Ordering::Equal => lhs.name.cmp(&rhs.name),
                    other => other,
                }
            })
            .dedup_by(|x, y| x.name == y.name)
            .map(|attribute| AttributeView { attribute })
    }

    pub fn generate_markdown<Out: Write>(
        &self,
        out: &mut Out,
        args: &GenerateMarkdownArgs,
        ctx: &mut GenerateMarkdownContext,
        attribute_registry_base_url: Option<&str>,
    ) -> Result<(), Error> {
        if self.group.r#type == GroupType::Event {
            write!(out, "The event name MUST be `{}`\n\n", self.event_name())?;
        }

        if args.is_omit_requirement() {
            writeln!(out, "| Attribute  | Type | Description  | Examples  | Stability |")?;
            writeln!(out, "|---|---|---|---|---|")?;
        } else {
            writeln!(out, "| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |")?;
            writeln!(out, "|---|---|---|---|---|---|")?;
        }

        // If the user defined a tag, use it to filter attributes.
        let attributes: Vec<AttributeView<'_>> = match args.tag_filter() {
            Some(tag) => self.attributes(args.is_full()).filter(|a| a.has_tag(tag)).collect(),
            None => self.attributes(args.is_full()).collect(),
        };

        for attr in &attributes {
            write!(out, "| ")?;
            attr.write_name_with_optional_link(out, attribute_registry_base_url)?;
            write!(out, " | ")?;
            attr.write_type_string(out)?;
            write!(out, " | ")?;
            attr.write_description(out, ctx)?;
            write!(out, " | ")?;
            attr.write_examples(out)?;
            if args.is_omit_requirement() {
                write!(out, " | ")?;
            } else {
                write!(out, " | ")?;
                attr.write_requirement(out, ctx)?;
                write!(out, " | ")?;
            }
            attr.write_stability(out)?;
            writeln!(out, " |")?;
        }
        // Add "note" footers
        ctx.write_rendered_notes(out)?;

        // No longer doing - Add "constraints" notes.

        // Add sampling relevant callouts.
        let sampling_relevant: Vec<AttributeView<'_>> = self
            .attributes(args.is_full())
            .filter(|a| a.is_sampling_relevant())
            .collect();
        if !sampling_relevant.is_empty() {
            write!(
                out,
                "\nThe following attributes can be important for making sampling decisions "
            )?;
            write!(
                out,
                "and SHOULD be provided **at span creation time** (if provided at all):\n\n"
            )?;
            for a in sampling_relevant {
                write!(out, "* ")?;
                a.write_name_with_optional_link(out, attribute_registry_base_url)?;
                writeln!(out)?;
            }
        }

        // Add enum footers
        for e in attributes.iter().filter(|a| a.is_enum()) {
            write!(out, "\n`")?;
            e.write_name(out)?;
            writeln!(out, "` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.")?;
            e.write_enum_spec_table(out)?;
        }
        Ok(())
    }
}

pub struct MetricView<'a> {
    group: &'a Group,
    // metric: &'a Metric,
}
impl<'a> MetricView<'a> {
    pub fn try_new(id: &str, lookup: &'a ResolvedSemconvRegistry) -> Result<MetricView<'a>, Error> {
        let metric = lookup
            .find_group(id)
            .filter(|g| g.r#type == GroupType::Metric);

        match metric {
            Some(group) => Ok(MetricView { group }),
            None => Err(Error::GroupMustBeMetric { id: id.to_owned() }),
        }
    }

    fn metric_name(&self) -> &str {
        self.group
            .metric_name
            .as_ref()
            .map(|r| r.as_ref())
            .unwrap_or("")
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
    fn write_description<Out: Write>(
        &self,
        out: &mut Out,
        ctx: &mut GenerateMarkdownContext,
    ) -> Result<(), Error> {
        // TODO - add note if needed.
        if self.group.note.is_empty() {
            write!(out, "{}", &self.group.brief)?;
        } else {
            write!(
                out,
                "{} {}",
                &self.group.brief,
                ctx.add_note(self.group.note.clone())
            )?;
        }
        Ok(())
    }
    fn write_stability<Out: Write>(&self, out: &mut Out) -> Result<(), Error> {
        write_stability_badge(out, &self.group.stability)
    }
    pub fn generate_markdown<Out: Write>(
        &self,
        out: &mut Out,
        ctx: &mut GenerateMarkdownContext,
    ) -> Result<(), Error> {
        writeln!(
            out,
            "| Name     | Instrument Type | Unit (UCUM) | Description    | Stability |"
        )?;
        writeln!(
            out,
            "| -------- | --------------- | ----------- | -------------- | --------- |"
        )?;
        write!(
            out,
            "| `{}` | {} | `",
            self.metric_name(),
            self.instrument()
        )?;
        self.write_unit(out)?;
        write!(out, "` | ")?;
        self.write_description(out, ctx)?;
        write!(out, " | ")?;
        self.write_stability(out)?;
        writeln!(out, " |")?;
        // Add "note" footers
        ctx.write_rendered_notes(out)?;
        Ok(())
    }
}
