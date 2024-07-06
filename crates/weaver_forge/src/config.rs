// SPDX-License-Identifier: Apache-2.0

//! Weaver Configuration Definition.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use convert_case::{Case, Casing, Converter, Pattern};
use convert_case::Boundary::{DigitLower, DigitUpper, Hyphen, LowerDigit, Space, UpperDigit};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_yaml::Value;

use crate::error::Error;
use crate::error::Error::InvalidConfigFile;
use crate::file_loader::FileLoader;
use crate::WEAVER_YAML;

const DEFAULT_WEAVER_CONFIG: &'static str = include_str!("../../../defaults/weaver_config/weaver.yaml");

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
#[derive(Deserialize, Debug, Default)]
pub(crate) struct WeaverConfig {
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

    /// Configuration for the templates.
    #[serde(default = "default_templates")]
    pub(crate) templates: Vec<TemplateConfig>,

    /// List of acronyms to be considered as unmodifiable words in the case
    /// conversion.
    pub(crate) acronyms: Option<Vec<String>>,
}

fn default_templates() -> Vec<TemplateConfig> {
    vec![
        TemplateConfig {
            pattern: Glob::new("**/registry.md").expect("Invalid pattern"),
            filter: ".".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/attribute_group.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"attribute_group\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/attribute_groups.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"attribute_group\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/event.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"event\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/events.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"event\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/group.md").expect("Invalid pattern"),
            filter: ".groups".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/groups.md").expect("Invalid pattern"),
            filter: ".groups".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/metric.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"metric\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/metrics.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"metric\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/resource.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"resource\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/resources.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"resource\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/scope.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"scope\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/scopes.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"scope\")".to_owned(),
            application_mode: ApplicationMode::Single,
        },
        TemplateConfig {
            pattern: Glob::new("**/span.md").expect("Invalid pattern"),
            filter: ".groups[] | select(.type == \"span\")".to_owned(),
            application_mode: ApplicationMode::Each,
        },
        TemplateConfig {
            pattern: Glob::new("**/spans.md").expect("Invalid pattern"),
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

impl TemplateSyntax {
    /// Override the current `TemplateSyntax` with the `TemplateSyntax` passed as argument.
    /// The merge is done in place. The `TemplateSyntax` passed as argument will be consumed and
    /// used to override the current `TemplateSyntax`.
    pub fn override_with(&mut self, other: TemplateSyntax) {
        self.block_start = other.block_start;
        self.block_end = other.block_end;
        self.variable_start = other.variable_start;
        self.variable_end = other.variable_end;
        self.comment_start = other.comment_start;
        self.comment_end = other.comment_end;
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

impl WhitespaceControl {
    /// Override the current `WhitespaceControl` with the `WhitespaceControl` passed as argument.
    /// The merge is done in place. The `WhitespaceControl` passed as argument will be consumed and
    /// used to override the current `WhitespaceControl`.
    pub fn override_with(&mut self, other: WhitespaceControl) {
        self.trim_blocks = other.trim_blocks;
        self.lstrip_blocks = other.lstrip_blocks;
        self.keep_trailing_newline = other.keep_trailing_newline;
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

impl WeaverConfig {
    pub(crate) fn try_new(loader: &dyn FileLoader) -> Result<WeaverConfig, Error> {
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
            Ok(WeaverConfig::default())
        }
    }

    /// Loads the Weaver configuration based on the following order:
    ///
    /// - the target directory,
    /// - the parent directory (i.e., the directory containing all targets),
    /// - the $HOME/.weaver/weaver.yaml directory,
    /// - and finally the defaults/weaver_config directory embedded in the Weaver binary.
    ///
    /// A local definition should override any corresponding definition in the parent directory,
    /// which itself can override the corresponding entry in the home directory (i.e., defined per
    /// user), and finally, the Weaver application binary (i.e., defined by Weaver authors). This
    /// hierarchical structure should allow for configuration at the target level, across all
    /// targets, at the user level, and even reuse of configurations defined within the Weaver
    /// application binary.
    ///
    /// This method can fail if any of the configuration file is not a valid YAML file or if the
    /// configuration file can't be deserialized into a `WeaverConfig` struct.
    pub(crate) fn try_new_2(home_dir: Option<PathBuf>, loader: &dyn FileLoader) -> Result<WeaverConfig, Error> {
        // Init the weaver config with the embedded defaults
        let mut config = serde_yaml::from_str(DEFAULT_WEAVER_CONFIG).map_err(|e| InvalidConfigFile {
            config_file: WEAVER_YAML.into(),
            error: e.to_string(),
        })?;

        // Override the defaults with the weaver.yaml file present in the user's home directory
        // if it exists.
        if let Some(home_dir) = home_dir {
            let home_config = home_dir.join(".weaver/weaver.yaml");
            if home_config.exists() {
                let home_config_file = std::fs::File::open(home_config.clone()).map_err(|e| InvalidConfigFile {
                    config_file: home_config.clone(),
                    error: e.to_string(),
                })?;
                let home_config: WeaverConfig = serde_yaml::from_reader(home_config_file)
                    .map_err(|e| InvalidConfigFile {
                        config_file: home_config.clone(),
                        error: e.to_string(),
                    })?;
            }
        }

        // Override the config with the weaver.yaml file present in the parent directory of the
        // target directory if it exists.

        // Override the config with the weaver.yaml file present in the target directory if it
        // exists.
        // deserialize in place, i.e. in the existing config


        Ok(config)
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

    /// Override the current `WeaverConfig` with the `WeaverConfig` passed as argument.
    /// The merge is done in place. The `WeaverConfig` passed as argument will be consumed and used
    /// to override the current `WeaverConfig`.
    pub fn override_with(&mut self, other: WeaverConfig) {
        self.file_name = other.file_name;
        self.function_name = other.function_name;
        self.arg_name = other.arg_name;
        self.struct_name = other.struct_name;
        self.field_name = other.field_name;
        if other.type_mapping.is_some() {
            self.type_mapping = other.type_mapping;
        }
        if other.text_maps.is_some() {
            self.text_maps = other.text_maps;
        }
        self.template_syntax.override_with(other.template_syntax);
        self.whitespace_control.override_with(other.whitespace_control);
        if other.params.is_some() {
            self.params = other.params;
        }
        self.templates = other.templates;
        if other.acronyms.is_some() {
            self.acronyms = other.acronyms;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::config::{DEFAULT_WEAVER_CONFIG, WeaverConfig};
    use crate::file_loader::FileSystemFileLoader;

    #[test]
    fn test_override_with() {
        // Tests params overrides.
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("params: {a: 3}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.params, Some([("a".to_owned(), 3.into())].iter().cloned().collect()));
        let mut parent: WeaverConfig = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("params: {a: 3}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.params, Some([("a".to_owned(), 3.into())].iter().cloned().collect()));
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local= WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(parent.params, Some([("a".to_owned(), 1.into()), ("b".to_owned(), 2.into())].iter().cloned().collect()));
        let mut parent: WeaverConfig = serde_yaml::from_str("params: {a: 1, b: 2}").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("params: {}").unwrap();
        parent.override_with(local);
        assert_eq!(parent.params, Some(HashMap::default()));

        // Tests templates overrides.

        // Tests acronyms overrides.
        // If defined in both, the local configuration should override the parent configuration.
        let mut parent: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS']").unwrap();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec!["iOS".to_owned()]));
        let mut parent = WeaverConfig::default();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()]));
        let mut parent: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local = WeaverConfig::default();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()]));
        let mut parent: WeaverConfig = serde_yaml::from_str("acronyms: ['iOS', 'API', 'URL']").unwrap();
        let local: WeaverConfig = serde_yaml::from_str("acronyms: []").unwrap();
        parent.override_with(local);
        assert_eq!(parent.acronyms, Some(vec![]));
    }

    #[test]
    fn test_try_new() -> Result<(), Box<dyn std::error::Error>> {
        let loader = FileSystemFileLoader::try_new("templates/registry".into(), "test")?;
        let config = WeaverConfig::try_new_2(dirs::home_dir(), &loader)
            .expect("Failed to load the Weaver configuration");

        dbg!(config);
        Ok(())
    }
}