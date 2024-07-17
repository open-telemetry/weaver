// SPDX-License-Identifier: Apache-2.0

//! Weaver Configuration Definition.

#![allow(rustdoc::invalid_html_tags)]

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use convert_case::{Case, Casing, Converter, Pattern};
use convert_case::Boundary::{DigitLower, DigitUpper, Hyphen, LowerDigit, Space, UpperDigit};
use dirs::home_dir;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use weaver_semconv::group::GroupType;
use weaver_semconv::stability::Stability;

use crate::error::Error;
use crate::error::Error::InvalidConfigFile;
use crate::file_loader::{FileContent, FileLoader};
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

/// Configuration for a registry processing pipeline.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct RegistryProcessing {
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
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupOrCondition {
    /// Regular expression on the id field.
    #[serde(with = "serde_regex")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<regex::Regex>,
    /// Set of group types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types_in: Option<HashSet<GroupType>>,
    /// Whether the group is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    /// Set of stability values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability_in: Option<HashSet<Stability>>,
    /// Whether the group has attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_attribute: Option<bool>,
}

/// Simple attribute filter configuration.
/// There is a OR logic between the conditions.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct AttributeOrCondition {
    /// Regular expression on the id field.
    #[serde(with = "serde_regex")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<regex::Regex>,
    /// Whether the group is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    /// Set of stability values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability_in: Option<HashSet<Stability>>,
}

impl RegistryProcessing {
    /// Override the current group processing with the provided one.
    pub fn override_with(&mut self, other: RegistryProcessing) {
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

/// Weaver configuration.
#[derive(Deserialize, Debug)]
pub struct WeaverConfig {
    /// Type mapping for target specific types (OTel types -> Target language types).
    /// Deprecated: Use `text_maps` instead.
    pub(crate) type_mapping: Option<HashMap<String, String>>,
    /// Configuration of the `text_map` filter.
    pub(crate) text_maps: Option<HashMap<String, HashMap<String, String>>>,
    /// Configuration for the template syntax.
    #[serde(default)]
    pub(crate) template_syntax: TemplateSyntax,
    /// Configuration for the whitespace behavior on the template engine.
    #[serde(default)]
    pub(crate) whitespace_control: WhitespaceControl,

    /// Parameters for the templates.
    /// These parameters can be overridden by parameters passed to the CLI.
    pub(crate) params: Option<HashMap<String, Value>>,

    /// Configuration for the registry processing pipeline.
    #[serde(default)]
    pub registry_processing: Option<RegistryProcessing>,

    /// Configuration for the templates.
    pub(crate) templates: Option<Vec<TemplateConfig>>,

    /// List of acronyms to be considered as unmodifiable words in the case
    /// conversion.
    #[serde(default)]
    pub(crate) acronyms: Option<Vec<String>>,
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
#[derive(Deserialize, Debug, PartialEq)]
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
    /// Configuration of a registry processing pipeline. This processing is applied to
    /// the registry before applying the JQ expression and the template.
    /// The registry_processing is simpler to use than JQ but less powerful.
    #[serde(default)]
    pub registry_processing: Option<RegistryProcessing>,
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
#[derive(Deserialize, Debug, Clone, Default)]
pub struct TemplateSyntax {
    /// The start of a block.
    pub block_start: Option<String>,
    /// The end of a block.
    pub block_end: Option<String>,
    /// The start of a variable.
    pub variable_start: Option<String>,
    /// The end of a variable.
    pub variable_end: Option<String>,
    /// The start of a comment.
    pub comment_start: Option<String>,
    /// The end of a comment.
    pub comment_end: Option<String>,
}

impl TemplateSyntax {
    /// Override the current `TemplateSyntax` with the `TemplateSyntax` passed as argument.
    /// The merge is done in place. The `TemplateSyntax` passed as argument will be consumed and
    /// used to override the current `TemplateSyntax`.
    pub fn override_with(&mut self, other: TemplateSyntax) {
        if other.block_start.is_some() {
            self.block_start = other.block_start;
        }
        if other.block_end.is_some() {
            self.block_end = other.block_end;
        }
        if other.variable_start.is_some() {
            self.variable_start = other.variable_start;
        }
        if other.variable_end.is_some() {
            self.variable_end = other.variable_end;
        }
        if other.comment_start.is_some() {
            self.comment_start = other.comment_start;
        }
        if other.comment_end.is_some() {
            self.comment_end = other.comment_end;
        }
    }
}

/// Whitespace control configuration for the template engine.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct WhitespaceControl {
    /// Configures the behavior of the first newline after a block.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_trim_blocks>
    pub trim_blocks: Option<bool>,
    /// Configures the behavior of leading spaces and tabs from the start of a line to a block.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_lstrip_blocks>
    pub lstrip_blocks: Option<bool>,
    /// Configures whether trailing newline are preserved when rendering templates.
    /// See <https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.set_keep_trailing_newline>
    pub keep_trailing_newline: Option<bool>,
}

impl WhitespaceControl {
    /// Override the current `WhitespaceControl` with the `WhitespaceControl` passed as argument.
    /// The merge is done in place. The `WhitespaceControl` passed as argument will be consumed and
    /// used to override the current `WhitespaceControl`.
    pub fn override_with(&mut self, other: WhitespaceControl) {
        if other.trim_blocks.is_some() {
            self.trim_blocks = other.trim_blocks;
        }
        if other.lstrip_blocks.is_some() {
            self.lstrip_blocks = other.lstrip_blocks;
        }
        if other.keep_trailing_newline.is_some() {
            self.keep_trailing_newline = other.keep_trailing_newline;
        }
    }
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

impl Default for WeaverConfig {
    fn default() -> Self {
        Self {
            type_mapping: None,
            text_maps: None,
            template_syntax: TemplateSyntax {
                block_start: Some("{%".to_owned()),
                block_end: Some("%}".to_owned()),
                variable_start: Some("{{".to_owned()),
                variable_end: Some("}}".to_owned()),
                comment_start: Some("{#".to_owned()),
                comment_end: Some("#}".to_owned()),
            },
            whitespace_control: Default::default(),
            params: None,
            registry_processing: None,
            templates: None,
            acronyms: None,
        }
    }
}

impl WeaverConfig {
    /// Attempts to load and build a `WeaverConfig` from configuration files found in the specified
    /// path. Configuration files are loaded in the following order of precedence:
    ///
    /// 1. The `<path>/weaver.yaml` file.
    /// 2. Any `weaver.yaml` files found in parent directories of the specified path, up to the root
    /// directory.
    /// 3. The `$HOME/.weaver/weaver.yaml` file.
    pub fn try_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let configs = Self::collect_from_path(path);
        Self::resolve_from(&configs)
    }

    /// Attempts to load all the configuration files and build a unique `WeaverConfig` from the
    /// specified configuration files. The last files in the list will override the first ones.
    ///
    /// This method can fail if any of the configuration content is not a valid YAML file or if the
    /// configuration content can't be deserialized into a `WeaverConfig` struct.
    pub fn try_from_config_files<P: AsRef<Path>>(config_files: &[P]) -> Result<Self, Error> {
        let mut configs = Vec::new();
        for config in config_files {
            configs.push(FileContent::try_from_path(config)?);
        }
        WeaverConfig::resolve_from(&configs)
    }

    /// Attempts to load and build a `weaver.yaml` file from the specified file loader. This
    /// constructor is only initializing the configuration from a single weaver.yaml file found
    /// in the loader. If no file is found, the default configuration is returned.
    pub fn try_from_loader(
        loader: &(impl FileLoader + Send + Sync + 'static),
    ) -> Result<Self, Error> {
        match loader.load_file(WEAVER_YAML)? {
            Some(config) => Self::resolve_from(&[config]),
            None => Ok(WeaverConfig::default()),
        }
    }

    /// Builds the Weaver configuration from a collection of configurations passed in parameter.
    /// The first configuration in the slice is loaded, then if present, the second configuration
    /// overrides the first one, and so on. This process is named "configuration resolution".
    ///
    /// This method can fail if any of the configuration content is not a valid YAML file or if the
    /// configuration content can't be deserialized into a `WeaverConfig` struct.
    fn resolve_from(configs: &[FileContent]) -> Result<WeaverConfig, Error> {
        // The default configuration is used as a base for the resolution.
        let mut config = WeaverConfig::default();
        if configs.is_empty() {
            return Ok(WeaverConfig::default());
        }

        // Each configuration is loaded and merged into the current configuration.
        for conf in configs {
            let conf: WeaverConfig =
                serde_yaml::from_str(&conf.content).map_err(|e| InvalidConfigFile {
                    config_file: conf.path.clone(),
                    error: e.to_string(),
                })?;
            config.override_with(conf);
        }

        Ok(config)
    }

    fn collect_from_path<P: AsRef<Path>>(path: P) -> Vec<FileContent> {
        let mut file_contents = Vec::new();

        // Detect all the weaver.yaml files in the path and parent folder.
        let mut current_path = path.as_ref();
        loop {
            if let Ok(file_content) = FileContent::try_from_path(current_path.join("weaver.yaml")) {
                file_contents.push(file_content);
            }

            if let Some(parent) = current_path.parent() {
                current_path = parent;
            } else {
                break;
            }
        }

        // Add the configuration from the home directory.
        if let Some(home_dir) = home_dir() {
            if let Ok(file_content) =
                FileContent::try_from_path(home_dir.join(".weaver/weaver.yaml"))
            {
                file_contents.push(file_content);
            }
        }

        file_contents.reverse();
        file_contents
    }

    /// Return a template matcher for the target configuration.
    pub fn template_matcher(&self) -> Result<TemplateMatcher<'_>, Error> {
        if let Some(templates) = &self.templates {
            let mut builder = GlobSetBuilder::new();

            for template in templates.iter() {
                _ = builder.add(template.pattern.clone());
            }

            builder
                .build()
                .map_err(|e| Error::InvalidTemplatePattern {
                    error: e.to_string(),
                })
                .map(|glob_set| TemplateMatcher {
                    templates,
                    glob_set,
                })
        } else {
            Ok(TemplateMatcher {
                templates: &[],
                glob_set: GlobSet::empty(),
            })
        }
    }

    /// Override the current `WeaverConfig` with the `WeaverConfig` passed as argument.
    /// The merge is done in place. The `WeaverConfig` passed as argument will be consumed and used
    /// to override the current `WeaverConfig`.
    pub fn override_with(&mut self, other: WeaverConfig) {
        if other.type_mapping.is_some() {
            self.type_mapping = other.type_mapping;
        }
        if other.text_maps.is_some() {
            self.text_maps = other.text_maps;
        }
        self.template_syntax.override_with(other.template_syntax);
        self.whitespace_control
            .override_with(other.whitespace_control);
        if other.params.is_some() {
            self.params = other.params;
        }
        if other.templates.is_some() {
            self.templates = other.templates;
        }
        if other.acronyms.is_some() {
            self.acronyms = other.acronyms;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{ApplicationMode, WeaverConfig};
    use crate::file_loader::FileContent;

    #[test]
    fn test_type_mapping_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig =
            serde_yaml::from_str("type_mapping: {a: \"b\", c: \"d\"}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("type_mapping: {a: \"e\"}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.type_mapping,
            Some([("a".to_owned(), "e".to_owned())].iter().cloned().collect())
        );
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("type_mapping: {a: \"e\"}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.type_mapping,
            Some([("a".to_owned(), "e".to_owned())].iter().cloned().collect())
        );
        let mut parent: WeaverConfig =
            serde_yaml::from_str("type_mapping: {a: \"b\", c: \"d\"}").unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(
            parent.type_mapping,
            Some(
                [
                    ("a".to_owned(), "b".to_owned()),
                    ("c".to_owned(), "d".to_owned())
                ]
                    .iter()
                    .cloned()
                    .collect()
            )
        );
    }

    #[test]
    fn test_text_maps_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig =
            serde_yaml::from_str("text_maps: {a: {b: \"c\"}, d: {e: \"f\"}}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("text_maps: {a: {b: \"g\"}}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.text_maps,
            Some(
                [(
                    "a".to_owned(),
                    [("b".to_owned(), "g".to_owned())].iter().cloned().collect()
                )]
                    .iter()
                    .cloned()
                    .collect()
            )
        );
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("text_maps: {a: {b: \"g\"}}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.text_maps,
            Some(
                [(
                    "a".to_owned(),
                    [("b".to_owned(), "g".to_owned())].iter().cloned().collect()
                )]
                    .iter()
                    .cloned()
                    .collect()
            )
        );
        let mut parent: WeaverConfig =
            serde_yaml::from_str("text_maps: {a: {b: \"c\"}, d: {e: \"f\"}}").unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(
            parent.text_maps,
            Some(
                [
                    (
                        "a".to_owned(),
                        [("b".to_owned(), "c".to_owned())].iter().cloned().collect()
                    ),
                    (
                        "d".to_owned(),
                        [("e".to_owned(), "f".to_owned())].iter().cloned().collect()
                    )
                ]
                    .iter()
                    .cloned()
                    .collect()
            )
        );
    }

    #[test]
    fn test_template_syntax_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "template_syntax: {block_start: \"{{\", block_end: \"}}\", variable_start: \"#\"}",
        )
            .unwrap();
        let local: WeaverConfig =
            serde_yaml::from_str("template_syntax: {block_start: \"[[\", block_end: \"]]\"}")
                .unwrap();
        parent.override_with(local);
        assert_eq!(parent.template_syntax.block_start, Some("[[".to_owned()));
        assert_eq!(parent.template_syntax.block_end, Some("]]".to_owned()));
        assert_eq!(parent.template_syntax.variable_start, Some("#".to_owned()));
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig =
            serde_yaml::from_str("template_syntax: {block_start: \"[[\", block_end: \"]]\"}")
                .unwrap();
        parent.override_with(local);
        assert_eq!(parent.template_syntax.block_start, Some("[[".to_owned()));
        assert_eq!(parent.template_syntax.block_end, Some("]]".to_owned()));
        assert_eq!(parent.template_syntax.variable_start, Some("{{".to_owned()));
    }

    #[test]
    fn test_whitespace_control_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig =
            serde_yaml::from_str("whitespace_control: {trim_blocks: true, lstrip_blocks: true}")
                .unwrap();
        let local: WeaverConfig =
            serde_yaml::from_str("whitespace_control: {lstrip_blocks: false}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.whitespace_control.trim_blocks, Some(true));
        assert_eq!(parent.whitespace_control.lstrip_blocks, Some(false));
        assert_eq!(parent.whitespace_control.keep_trailing_newline, None);
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("whitespace_control: {trim_blocks: true, lstrip_blocks: true, keep_trailing_newline: true}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.whitespace_control.trim_blocks, Some(true));
        assert_eq!(parent.whitespace_control.lstrip_blocks, Some(true));
        assert_eq!(parent.whitespace_control.keep_trailing_newline, Some(true));
        let mut parent: WeaverConfig =
            serde_yaml::from_str("whitespace_control: {trim_blocks: true, lstrip_blocks: true}")
                .unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(parent.whitespace_control.trim_blocks, Some(true));
        assert_eq!(parent.whitespace_control.lstrip_blocks, Some(true));
        assert_eq!(parent.whitespace_control.keep_trailing_newline, None);
    }

    #[test]
    fn test_params_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("params: {a: 3}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.params,
            Some([("a".to_owned(), 3.into())].iter().cloned().collect())
        );
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("params: {a: 3}").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.params,
            Some([("a".to_owned(), 3.into())].iter().cloned().collect())
        );
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(
            parent.params,
            Some(
                [("a".to_owned(), 1.into()), ("b".to_owned(), 2.into())]
                    .iter()
                    .cloned()
                    .collect()
            )
        );
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("params: {}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.params, Some(HashMap::default()));
    }

    #[test]
    fn test_templates_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{pattern: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
        )
            .unwrap();
        let local: WeaverConfig = serde_yaml::from_str(
            "templates: [{pattern: \"**/local.md\", filter: \".\", application_mode: \"each\"}]",
        )
            .unwrap();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].pattern.to_string(), "**/local.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Each);
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str(
            "templates: [{pattern: \"**/local.md\", filter: \".\", application_mode: \"each\"}]",
        )
            .unwrap();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].pattern.to_string(), "**/local.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Each);
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{pattern: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
        )
            .unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].pattern.to_string(), "**/parent.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Single);
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{pattern: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
        )
            .unwrap();
        let local: WeaverConfig = serde_yaml::from_str("templates: []").unwrap();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 0);
    }

    #[test]
    fn test_acronyms_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig =
            serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS']").unwrap();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec!["iOS".to_owned()]));
        let mut parent = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.acronyms,
            Some(vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()])
        );
        let mut parent: WeaverConfig =
            serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(
            parent.acronyms,
            Some(vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()])
        );
        let mut parent: WeaverConfig =
            serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: []").unwrap();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec![]));
    }

    #[test]
    fn test_try_new() -> Result<(), Box<dyn std::error::Error>> {
        let configs = vec![
            FileContent::try_from_path("templates/registry/weaver.yaml").unwrap(),
            FileContent::try_from_path("templates/registry/xyz/weaver.yaml").unwrap(),
        ];
        let config =
            WeaverConfig::resolve_from(&configs).expect("Failed to load the Weaver configuration");

        assert!(config.text_maps.is_none());

        assert_eq!(config.template_syntax.block_start, Some(">".to_owned()));
        assert_eq!(config.template_syntax.block_end, Some("<<".to_owned()));
        assert_eq!(config.template_syntax.variable_start, Some("{{".to_owned()));
        assert_eq!(config.template_syntax.variable_end, Some("}}".to_owned()));
        assert_eq!(config.template_syntax.comment_start, Some("{#".to_owned()));
        assert_eq!(config.template_syntax.comment_end, Some("#}".to_owned()));

        assert_eq!(config.whitespace_control.trim_blocks, Some(true));
        assert_eq!(config.whitespace_control.lstrip_blocks, Some(false));
        assert_eq!(config.whitespace_control.keep_trailing_newline, Some(true));

        assert!(config.params.is_none());
        assert!(config.templates.is_none());
        assert!(config.acronyms.is_none());

        Ok(())
    }
}
