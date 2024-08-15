// SPDX-License-Identifier: Apache-2.0

//! Weaver Configuration Definition.

#![allow(rustdoc::invalid_html_tags)]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

use convert_case::Boundary::{
    DigitLower, DigitUpper, Hyphen, LowerDigit, LowerUpper, Space, Underscore, UpperDigit,
};
use convert_case::{Converter, Pattern};
use dirs::home_dir;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::error::Error;
use crate::error::Error::InvalidConfigFile;
use crate::file_loader::{FileContent, FileLoader};
use crate::formats::html::HtmlRenderOptions;
use crate::formats::markdown::MarkdownRenderOptions;
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

/// Weaver configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
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

    /// Configuration for the comment formats.
    #[serde(default)]
    pub(crate) comment_formats: Option<HashMap<String, CommentFormat>>,
    /// Default comment format used by the comment filter.
    pub(crate) default_comment_format: Option<String>,

    /// Parameters for the templates.
    /// These parameters can be overridden by parameters passed to the CLI.
    /// Note: We use a `BTreeMap` to ensure that the parameters are sorted by key
    /// when serialized to YAML. This is useful for testing purposes.
    pub(crate) params: Option<BTreeMap<String, Value>>,

    /// Configuration for the templates.
    pub(crate) templates: Option<Vec<TemplateConfig>>,

    /// List of acronyms to be considered as unmodifiable words in the case
    /// conversion.
    pub(crate) acronyms: Option<Vec<String>>,
}

/// Parameters defined in the command line via the `--params` argument.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Params {
    /// Parameters for the templates.
    #[serde(default)]
    pub params: HashMap<String, Value>,
}

impl Params {
    /// Create a new `Params` struct from a slice of key-value pairs.
    #[must_use]
    pub fn from_key_value_pairs(params: &[(&str, Value)]) -> Self {
        Params {
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.to_owned()))
                .collect(),
        }
    }
}

/// Application mode defining how to apply a template on the result of a
/// filter applied on a registry.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationMode {
    /// Apply the template to the output of the filter as a whole.
    Single,
    /// Apply the template to each item of the list returned by the filter.
    Each,
}

/// A template configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) struct TemplateConfig {
    /// The template pattern used to identify when this template configuration
    /// must be applied to a specific template file.
    #[serde(alias = "pattern")] // Alias for backward compatibility.
    pub(crate) template: Glob,
    /// The filter to apply to the registry before applying the template.
    /// Applying a filter to a registry will return a list of elements from the
    /// registry that satisfy the filter.
    /// By default, the filter is set to "." which means that the whole registry
    /// is returned.
    #[serde(default = "default_filter")]
    pub(crate) filter: String,
    /// The mode to apply the template.
    /// `single`: Apply the template to the output of the filter as a whole.
    /// `each`: Apply the template to each item of the list returned by the filter.
    pub(crate) application_mode: ApplicationMode,
    /// Parameters for the current template. All the parameters defined here will
    /// override the parameters defined in the `params` section of the configuration.
    /// These parameters can be overridden by parameters passed to the CLI.
    /// Note: We use a `BTreeMap` to ensure that the parameters are sorted by key
    /// when serialized to YAML. This is useful for testing purposes.
    pub(crate) params: Option<BTreeMap<String, Value>>,
    /// An optional file name defining where to write the output of the template.
    /// This name is relative to the output directory.
    /// The default value of this path is the same as the input file path.
    /// This file path can be a Jinja expression referencing the parameters.
    pub(crate) file_name: Option<String>,
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
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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

/// Supported comment formats.
#[derive(Default, Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct RenderOptions {
    /// A comment header (e.g. in Java `/**`).
    pub(crate) header: Option<String>,
    /// A comment prefix (e.g. in Java ` * `).
    pub(crate) prefix: Option<String>,
    /// A comment footer (e.g. in Java ` */`).
    pub(crate) footer: Option<String>,
    /// Options for a specific format
    #[serde(flatten)]
    pub(crate) format: RenderFormat,
}

/// The different supported formats for rendering comments.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "format")]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    /// Markdown format.
    Markdown(MarkdownRenderOptions),
    /// HTML format.
    Html(HtmlRenderOptions),
}

impl Default for RenderFormat {
    fn default() -> Self {
        RenderFormat::Markdown(MarkdownRenderOptions::default())
    }
}

/// Transform options for the comment filter.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct TransformOptions {
    /// Flag to trim the comment content.
    #[serde(default = "default_bool::<true>")]
    pub trim: bool,
    /// Flag to remove trailing dots from the comment content.
    #[serde(default = "default_bool::<false>")]
    pub remove_trailing_dots: bool,
    /// List of strong words to highlight in the comment.
    /// e.g. ["MUST", "SHOULD", "TODO", "FIXME"]
    #[serde(default = "Vec::default")]
    pub strong_words: Vec<String>,
    /// Jinja expression to specify the style of the strong words.
    pub strong_word_style: Option<String>,
}

impl Default for TransformOptions {
    fn default() -> Self {
        TransformOptions {
            trim: true,
            remove_trailing_dots: false,
            strong_words: Vec::default(),
            strong_word_style: None,
        }
    }
}

/// Used to set a default value for a boolean field in a struct.
#[must_use]
pub const fn default_bool<const V: bool>() -> bool {
    V
}

/// Configuration for the comment format. This configuration is used
/// by the comment filter to format comments.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct CommentFormat {
    /// The low-level comment syntax.
    #[serde(default)]
    pub render_options: RenderOptions,
    /// Transform options for the comment filter.
    #[serde(default)]
    pub transform_options: TransformOptions,
}

impl Default for CaseConvention {
    /// Default case convention is PascalCase
    fn default() -> Self {
        CaseConvention::PascalCase
    }
}

impl CaseConvention {
    pub(crate) fn convert(&self, text: &str) -> String {
        // The converters are cached to avoid re-creating them for each conversion.
        // We use a `OnceLock` to ensure that the converters are created only once and
        // are thread-safe.
        static LOWER_CASE: OnceLock<Converter> = OnceLock::new();
        static UPPER_CASE: OnceLock<Converter> = OnceLock::new();
        static CAMEL_CASE: OnceLock<Converter> = OnceLock::new();
        static TITLE_CASE: OnceLock<Converter> = OnceLock::new();
        static KEBAB_CASE: OnceLock<Converter> = OnceLock::new();
        static SCREAMING_KEBAB_CASE: OnceLock<Converter> = OnceLock::new();
        static PASCAL_CASE: OnceLock<Converter> = OnceLock::new();
        static SNAKE_CASE: OnceLock<Converter> = OnceLock::new();
        static SCREAMING_SNAKE_CASE: OnceLock<Converter> = OnceLock::new();

        fn new_converter<T: ToString>(pattern: Pattern, delim: T) -> Converter {
            // For all case converters, we do not consider digits
            // as boundaries.
            Converter::new()
                .remove_boundary(DigitLower)
                .remove_boundary(DigitUpper)
                .remove_boundary(UpperDigit)
                .remove_boundary(LowerDigit)
                .set_pattern(pattern)
                .set_delim(delim)
        }

        let text = text.replace('.', "_");
        match self {
            CaseConvention::LowerCase => LOWER_CASE
                .get_or_init(|| new_converter(Pattern::Lowercase, " ").add_boundary(Space))
                .convert(&text),
            CaseConvention::UpperCase => UPPER_CASE
                .get_or_init(|| new_converter(Pattern::Uppercase, " ").add_boundary(Space))
                .convert(&text),
            CaseConvention::TitleCase => TITLE_CASE
                .get_or_init(|| new_converter(Pattern::Capital, " ").add_boundary(Space))
                .convert(&text),
            CaseConvention::PascalCase => PASCAL_CASE
                .get_or_init(|| new_converter(Pattern::Capital, "").add_boundary(LowerUpper))
                .convert(&text),
            CaseConvention::CamelCase => CAMEL_CASE
                .get_or_init(|| new_converter(Pattern::Camel, "").add_boundary(LowerUpper))
                .convert(&text),
            CaseConvention::SnakeCase => SNAKE_CASE
                .get_or_init(|| new_converter(Pattern::Lowercase, "_").add_boundary(Underscore))
                .convert(&text),
            CaseConvention::ScreamingSnakeCase => SCREAMING_SNAKE_CASE
                .get_or_init(|| new_converter(Pattern::Uppercase, "_").add_boundary(Underscore))
                .convert(&text),
            CaseConvention::KebabCase => KEBAB_CASE
                .get_or_init(|| new_converter(Pattern::Lowercase, "-").add_boundary(Hyphen))
                .convert(&text),
            CaseConvention::ScreamingKebabCase => SCREAMING_KEBAB_CASE
                .get_or_init(|| new_converter(Pattern::Uppercase, "-").add_boundary(Hyphen))
                .convert(&text),
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
            comment_formats: None,
            default_comment_format: None,
            params: None,
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
    ///    directory.
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
                _ = builder.add(template.template.clone());
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
    pub fn override_with(&mut self, child: WeaverConfig) {
        if child.type_mapping.is_some() {
            self.type_mapping = child.type_mapping;
        }
        if child.text_maps.is_some() {
            self.text_maps = child.text_maps;
        }
        self.template_syntax.override_with(child.template_syntax);
        self.whitespace_control
            .override_with(child.whitespace_control);

        // If the `comment_formats` are defined in the child configuration, they override the
        // parent configuration.
        if child.comment_formats.is_some() {
            self.comment_formats = child.comment_formats;
        }
        if child.default_comment_format.is_some() {
            self.default_comment_format = child.default_comment_format;
        }

        if let Some(other_params) = child.params {
            // `params` are merged in an additive way. For example, if a parameter is defined in
            // the `params` section of a template, then the final `params` section for this template
            // will include both the `params` inherited from the file-level configuration and the
            // `params` defined in the template configuration.
            // If the override is a parameter set to null, then this parameter is removed from the
            // final `params` section.
            for (key, value) in other_params {
                if value.is_null() {
                    // If the value is null, the key is removed from the parameters.
                    if let Some(params) = &mut self.params {
                        // We don't need to do anything with the previous value, so we can ignore
                        // the result of the remove operation.
                        _ = params.remove(&key);
                    }
                } else {
                    // If the key is already defined, the value is overridden. So no need to check if
                    // the key is already present.
                    _ = self
                        .params
                        .get_or_insert_with(BTreeMap::new)
                        .insert(key, value);
                }
            }
        }
        if child.templates.is_some() {
            self.templates = child.templates;
        }
        if child.acronyms.is_some() {
            self.acronyms = child.acronyms;
        }
    }
}

#[cfg(test)]
mod tests {
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
            Some(
                [("a".to_owned(), 3.into()), ("b".to_owned(), 2.into())]
                    .iter()
                    .cloned()
                    .collect()
            )
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
        let local: WeaverConfig = serde_yaml::from_str(r#"params: {b: Null}"#).unwrap();
        parent.override_with(local);
        assert_eq!(
            parent.params,
            Some([("a".to_owned(), 1.into())].iter().cloned().collect())
        );
    }

    #[test]
    fn test_templates_override_with() {
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{template: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
        )
        .unwrap();
        let local: WeaverConfig = serde_yaml::from_str(
            "templates: [{template: \"**/local.md\", filter: \".\", application_mode: \"each\"}]",
        )
        .unwrap();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].template.to_string(), "**/local.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Each);
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str(
            "templates: [{template: \"**/local.md\", filter: \".\", application_mode: \"each\"}]",
        )
        .unwrap();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].template.to_string(), "**/local.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Each);
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{template: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
        )
        .unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert!(parent.templates.is_some());
        let templates = parent.templates.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].template.to_string(), "**/parent.md");
        assert_eq!(templates[0].filter, ".");
        assert_eq!(templates[0].application_mode, ApplicationMode::Single);
        let mut parent: WeaverConfig = serde_yaml::from_str(
            "templates: [{template: \"**/parent.md\", filter: \".\", application_mode: \"single\"}]",
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
