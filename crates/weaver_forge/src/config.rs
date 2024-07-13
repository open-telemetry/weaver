// SPDX-License-Identifier: Apache-2.0

//! Configuration for the template crate.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use convert_case::Boundary::{DigitLower, DigitUpper, Hyphen, LowerDigit, Space, UpperDigit};
use convert_case::{Case, Casing, Converter, Pattern};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_yaml::Value;
use weaver_semconv::group::GroupType;
use weaver_semconv::stability::Stability;

use crate::error::Error;
use crate::error::Error::InvalidConfigFile;
use crate::file_loader::FileLoader;
use crate::WEAVER_YAML;

/// Case convention for naming of functions and structs.
#[derive(Deserialize, Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum CaseConvention {
    /// Lower case convention (e.g. lowercase).
    #[serde(rename = "lowercase")]
    LowerCase,
    /// Upper case convention (e.g. UPPERCASE).
    #[serde(rename = "UPPERCASE")]
    UpperCase,
    /// Title case convention (e.g. Title Case).
    #[serde(rename = "TitleCase")]
    TitleCase,
    /// Pascal case convention (e.g. PascalCase).
    #[serde(rename = "PascalCase")]
    PascalCase,
    /// Camel case convention (e.g. camelCase).
    #[serde(rename = "camelCase")]
    CamelCase,
    /// Snake case convention (e.g. snake_case).
    #[serde(rename = "snake_case")]
    SnakeCase,
    /// Screaming snake case convention (e.g. SCREAMING_SNAKE_CASE).
    #[serde(rename = "SCREAMING_SNAKE_CASE")]
    ScreamingSnakeCase,
    /// Kebab case convention (e.g. kebab-case).
    #[serde(rename = "kebab-case")]
    KebabCase,
    /// Screaming kebab case convention (e.g. SCREAMING-KEBAB-CASE).
    #[serde(rename = "SCREAMING-KEBAB-CASE")]
    ScreamingKebabCase,
}

/// Configuration for a group processing.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupProcessing {
    /// Retain groups with the specified group filter.
    pub retain_groups_with: Option<GroupOrCondition>,
    /// Remove groups with the specified group filter.
    pub remove_groups_with: Option<GroupOrCondition>,
    /// Retain attributes with the specified attribute filter.
    pub retain_attributes_with: Option<AttributeOrCondition>,
    /// Remove attributes with the specified attribute filter.
    pub remove_attributes_with: Option<AttributeOrCondition>,
    /// Whether to sort groups by ID.
    pub sort_groups_by_id: Option<bool>,
}

/// Simple group filter configuration.
/// There is a OR logic between the conditions.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupOrCondition {
    /// Regular expression on the id field.
    #[serde(with = "serde_regex")]
    #[serde(default)]
    pub id: Option<regex::Regex>,
    /// Set of group types.
    pub types_in: Option<HashSet<GroupType>>,
    /// Whether the group is deprecated.
    pub deprecated: Option<bool>,
    /// Set of stability values.
    pub stability_in: Option<HashSet<Stability>>,
    /// Whether the group has attribute.
    pub no_attribute: Option<bool>,
}

/// Simple attribute filter configuration.
/// There is a OR logic between the conditions.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct AttributeOrCondition {
    /// Regular expression on the id field.
    #[serde(with = "serde_regex")]
    #[serde(default)]
    pub name: Option<regex::Regex>,
    /// Whether the group is deprecated.
    pub deprecated: Option<bool>,
    /// Set of stability values.
    pub stability_in: Option<HashSet<Stability>>,
}

impl GroupProcessing {
    /// Override the current group processing with the provided one.
    pub fn override_with(&mut self, other: GroupProcessing) {
        if let Some(other) = other.remove_groups_with {
            self.remove_groups_with = Some(
                self.remove_groups_with
                    .take()
                    .map(|mut existing| {
                        existing.override_with(other.clone());
                        existing
                    })
                    .unwrap_or(other),
            );
        }
        if let Some(other) = other.retain_groups_with {
            self.retain_groups_with = Some(
                self.retain_groups_with
                    .take()
                    .map(|mut existing| {
                        existing.override_with(other.clone());
                        existing
                    })
                    .unwrap_or(other),
            );
        }
        if let Some(other) = other.remove_attributes_with {
            self.remove_attributes_with = Some(
                self.remove_attributes_with
                    .take()
                    .map(|mut existing| {
                        existing.override_with(other.clone());
                        existing
                    })
                    .unwrap_or(other),
            );
        }
        if let Some(other) = other.retain_attributes_with {
            self.retain_attributes_with = Some(
                self.retain_attributes_with
                    .take()
                    .map(|mut existing| {
                        existing.override_with(other.clone());
                        existing
                    })
                    .unwrap_or(other),
            );
        }
        if other.sort_groups_by_id.is_some() {
            self.sort_groups_by_id = other.sort_groups_by_id;
        }
    }
}

impl GroupOrCondition {
    /// Override the current simple filter with the provided one.
    pub fn override_with(&mut self, other: GroupOrCondition) {
        if other.id.is_some() {
            self.id = other.id;
        }
        if other.stability_in.is_some() {
            self.stability_in = other.stability_in;
        }
        if other.types_in.is_some() {
            self.types_in = other.types_in;
        }
        if other.no_attribute.is_some() {
            self.no_attribute = other.no_attribute;
        }
    }
}

impl AttributeOrCondition {
    /// Override the current simple filter with the provided one.
    pub fn override_with(&mut self, other: AttributeOrCondition) {
        if other.name.is_some() {
            self.name = other.name;
        }
        if other.stability_in.is_some() {
            self.stability_in = other.stability_in;
        }
    }
}

/// Target specific configuration.
#[derive(Deserialize, Debug, Default)]
pub struct TargetConfig {
    /// Case convention used to name a file.
    #[serde(default)]
    pub(crate) file_name: CaseConvention,
    /// Case convention used to name a function.
    #[serde(default)]
    pub(crate) function_name: CaseConvention,
    /// Case convention used to name a function argument.
    #[serde(default)]
    pub(crate) arg_name: CaseConvention,
    /// Case convention used to name a struct.
    #[serde(default)]
    pub(crate) struct_name: CaseConvention,
    /// Case convention used to name a struct field.
    #[serde(default)]
    pub(crate) field_name: CaseConvention,
    /// Type mapping for target specific types (OTel types -> Target language types).
    #[serde(default)]
    pub(crate) type_mapping: HashMap<String, String>,
    /// Configuration of the `text_map` filter.
    #[serde(default)]
    pub(crate) text_maps: HashMap<String, HashMap<String, String>>,
    /// Configuration for the template syntax.
    #[serde(default)]
    pub(crate) template_syntax: TemplateSyntax,
    /// Configuration for the whitespace behavior on the template engine.
    #[serde(default)]
    pub(crate) whitespace_control: WhitespaceControl,

    /// Parameters for the templates.
    /// These parameters can be overridden by parameters passed to the CLI.
    #[serde(default)]
    pub(crate) params: HashMap<String, Value>,

    /// Configuration for the group processing.
    #[serde(default)]
    pub group_processing: Option<GroupProcessing>,

    /// Configuration for the templates.
    #[serde(default = "default_templates")]
    pub(crate) templates: Vec<TemplateConfig>,

    /// List of acronyms to be considered as unmodifiable words in the case
    /// conversion.
    #[serde(default)]
    pub(crate) acronyms: Vec<String>,
}

fn default_templates() -> Vec<TemplateConfig> {
    vec![
        TemplateConfig {
            pattern: Glob::new("**/registry.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/attribute_group.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"attribute_group\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/attribute_groups.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"attribute_group\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/event.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"event\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/events.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"event\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/group.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/groups.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/metric.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"metric\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/metrics.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"metric\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/resource.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"resource\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/resources.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"resource\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/scope.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"scope\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/scopes.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"scope\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/span.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"span\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/spans.md").expect("Invalid pattern"),
            group_processing: None,
            filter: ".groups[] | select(.type == \"span\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
    ]
}

/// Parameters defined in the command line via the `--params` argument.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Params {
    /// Parameters for the templates.
    #[serde(default)]
    pub params: HashMap<String, Value>,
}

/// Application mode defining how to apply a template on the result of a
/// filter applied on a registry.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationMode {
    /// Apply the template to the output of the filter as a whole.
    Single,
    /// Apply the template to each item of the list returned by the filter.
    Each,
}

/// A template configuration.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) struct TemplateConfig {
    /// The pattern used to identify when this template configuration must be
    /// applied to a specific template file.
    pub(crate) pattern: Glob,
    /// Configuration of a group processing. This processing is applied to
    /// the registry before applying the JQ expression and the template.
    /// The group_processing is simpler to use than JQ but less powerful.
    #[serde(default)]
    pub group_processing: Option<GroupProcessing>,
    /// The filter to apply to the registry before applying the template.
    /// Applying a filter to a registry will return a list of elements from the
    /// registry that satisfy the filter.
    /// By default, the filter is set to "." which means that the whole registry
    /// is returned.
    #[serde(default = "default_filter", alias = "jq_expr")]
    pub(crate) filter: String,
    /// The mode to apply the template.
    /// `single`: Apply the template to the output of the filter as a whole.
    /// `each`: Apply the template to each item of the list returned by the filter.
    pub(crate) application_mode: ApplicationMode,
}

fn default_filter() -> String {
    ".".to_owned()
}

/// A template matcher.
pub struct TemplateMatcher<'a> {
    templates: &'a [TemplateConfig],
    glob_set: GlobSet,
}

impl<'a> TemplateMatcher<'a> {
    pub(crate) fn matches<P: AsRef<Path>>(&self, path: P) -> Vec<&'a TemplateConfig> {
        self.glob_set
            .matches(path)
            .into_iter()
            .map(|i| &self.templates[i])
            .collect()
    }
}

/// Syntax configuration for the template engine.
#[derive(Deserialize, Debug, Clone)]
pub struct TemplateSyntax {
    /// The start of a block.
    #[serde(default = "default_block_start")]
    pub block_start: String,
    /// The end of a block.
    #[serde(default = "default_block_end")]
    pub block_end: String,
    /// The start of a variable.
    #[serde(default = "default_variable_start")]
    pub variable_start: String,
    /// The end of a variable.
    #[serde(default = "default_variable_end")]
    pub variable_end: String,
    /// The start of a comment.
    #[serde(default = "default_comment_start")]
    pub comment_start: String,
    /// The end of a comment.
    #[serde(default = "default_comment_end")]
    pub comment_end: String,
}

/// Default block start delimiter.
fn default_block_start() -> String {
    "{%".to_owned()
}

/// Default block end delimiter.
fn default_block_end() -> String {
    "%}".to_owned()
}

/// Default variable start delimiter.
fn default_variable_start() -> String {
    "{{".to_owned()
}

/// Default variable end delimiter.
fn default_variable_end() -> String {
    "}}".to_owned()
}

/// Default comment start delimiter.
fn default_comment_start() -> String {
    "{#".to_owned()
}

/// Default comment end delimiter.
fn default_comment_end() -> String {
    "#}".to_owned()
}

impl Default for TemplateSyntax {
    fn default() -> Self {
        TemplateSyntax {
            block_start: default_block_start(),
            block_end: default_block_end(),
            variable_start: default_variable_start(),
            variable_end: default_variable_end(),
            comment_start: default_comment_start(),
            comment_end: default_comment_end(),
        }
    }
}

/// Whitespace control configuration for the template engine.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct WhitespaceControl {
    /// Configures the behavior of the first newline after a block.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_trim_blocks>
    pub trim_blocks: bool,
    /// Configures the behavior of leading spaces and tabs from the start of a line to a block.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_lstrip_blocks>
    pub lstrip_blocks: bool,
    /// Configures whether trailing newline are preserved when rendering templates.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_keep_trailing_newline>
    pub keep_trailing_newline: bool,
}

impl Default for CaseConvention {
    /// Default case convention is PascalCase
    fn default() -> Self {
        CaseConvention::PascalCase
    }
}

impl CaseConvention {
    pub(crate) fn convert(&self, text: &str) -> String {
        static LOWER_CASE: OnceLock<Converter> = OnceLock::new();
        static TITLE_CASE: OnceLock<Converter> = OnceLock::new();
        static KEBAB_CASE: OnceLock<Converter> = OnceLock::new();

        let text = text.replace('.', "_");
        match self {
            CaseConvention::LowerCase => {
                // Convert to lower case but do not consider digits
                // as boundaries. So that `k8s` will stay `k8s` and
                // not `k-8-s`.
                let conv = LOWER_CASE.get_or_init(|| {
                    Converter::new()
                        .add_boundary(Space)
                        .remove_boundary(DigitLower)
                        .remove_boundary(DigitUpper)
                        .remove_boundary(UpperDigit)
                        .remove_boundary(LowerDigit)
                        .set_pattern(Pattern::Lowercase)
                        .set_delim(" ")
                });
                conv.convert(&text)
            }
            CaseConvention::UpperCase => text.to_case(Case::Upper),
            CaseConvention::TitleCase => {
                // Convert to title case but do not consider digits
                // as boundaries.
                let conv = TITLE_CASE.get_or_init(|| {
                    Converter::new()
                        .add_boundary(Space)
                        .remove_boundary(DigitLower)
                        .remove_boundary(DigitUpper)
                        .remove_boundary(UpperDigit)
                        .remove_boundary(LowerDigit)
                        .set_pattern(Pattern::Capital)
                        .set_delim(" ")
                });
                conv.convert(&text)
            }
            CaseConvention::PascalCase => text.to_case(Case::Pascal),
            CaseConvention::CamelCase => text.to_case(Case::Camel),
            CaseConvention::SnakeCase => text.to_case(Case::Snake),
            CaseConvention::ScreamingSnakeCase => text.to_case(Case::ScreamingSnake),
            CaseConvention::KebabCase => {
                // Convert to kebab case but do not consider digits
                // as boundaries. So that `k8s` will stay `k8s` and
                // not `k-8-s`.
                let conv = KEBAB_CASE.get_or_init(|| {
                    Converter::new()
                        .add_boundary(Hyphen)
                        .remove_boundary(DigitLower)
                        .remove_boundary(DigitUpper)
                        .remove_boundary(UpperDigit)
                        .remove_boundary(LowerDigit)
                        .set_pattern(Pattern::Lowercase)
                        .set_delim("-")
                });
                conv.convert(&text)
            }
            CaseConvention::ScreamingKebabCase => text.to_case(Case::Cobol),
        }
    }
}

impl TargetConfig {
    pub(crate) fn try_new(loader: &dyn FileLoader) -> Result<TargetConfig, Error> {
        let weaver_file = loader
            .load_file(WEAVER_YAML)
            .map_err(|e| InvalidConfigFile {
                config_file: WEAVER_YAML.into(),
                error: e.to_string(),
            })?;
        if let Some(weaver_file) = weaver_file {
            serde_yaml::from_str(&weaver_file).map_err(|e| InvalidConfigFile {
                config_file: WEAVER_YAML.into(),
                error: e.to_string(),
            })
        } else {
            Ok(TargetConfig::default())
        }
    }

    /// Return a template matcher for the target configuration.
    pub fn template_matcher(&self) -> Result<TemplateMatcher<'_>, Error> {
        let mut builder = GlobSetBuilder::new();

        self.templates.iter().for_each(|template| {
            _ = builder.add(template.pattern.clone());
        });

        builder
            .build()
            .map_err(|e| Error::InvalidTemplatePattern {
                error: e.to_string(),
            })
            .map(|glob_set| TemplateMatcher {
                templates: &self.templates,
                glob_set,
            })
    }
}
