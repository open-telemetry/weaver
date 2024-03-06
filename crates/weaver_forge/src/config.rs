// SPDX-License-Identifier: Apache-2.0

//! Configuration for the template crate.

use std::collections::HashMap;
use std::path::Path;

use convert_case::{Case, Casing};
use serde::Deserialize;

use crate::Error;
use crate::Error::InvalidConfigFile;

/// Case convention for naming of functions and structs.
#[derive(Deserialize, Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum CaseConvention {
    #[serde(rename = "lowercase")]
    LowerCase,
    #[serde(rename = "UPPERCASE")]
    UpperCase,
    #[serde(rename = "PascalCase")]
    PascalCase,
    #[serde(rename = "camelCase")]
    CamelCase,
    #[serde(rename = "snake_case")]
    SnakeCase,
    #[serde(rename = "SCREAMING_SNAKE_CASE")]
    ScreamingSnakeCase,
    #[serde(rename = "kebab-case")]
    KebabCase,
    #[serde(rename = "SCREAMING-KEBAB-CASE")]
    ScreamingKebabCase,
}

/// Target specific configuration.
#[derive(Deserialize, Debug, Default)]
pub struct TargetConfig {
    /// Case convention used to name a file.
    #[serde(default)]
    pub file_name: CaseConvention,
    /// Case convention used to name a function.
    #[serde(default)]
    pub function_name: CaseConvention,
    /// Case convention used to name a function argument.
    #[serde(default)]
    pub arg_name: CaseConvention,
    /// Case convention used to name a struct.
    #[serde(default)]
    pub struct_name: CaseConvention,
    /// Case convention used to name a struct field.
    #[serde(default)]
    pub field_name: CaseConvention,
    /// Type mapping for target specific types (OTel types -> Target language types).
    #[serde(default)]
    pub type_mapping: HashMap<String, String>,
    /// Configuration for the template syntax.
    #[serde(default)]
    pub template_syntax: TemplateSyntax,
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
    "{%".to_string()
}

/// Default block end delimiter.
fn default_block_end() -> String {
    "%}".to_string()
}

/// Default variable start delimiter.
fn default_variable_start() -> String {
    "{{".to_string()
}

/// Default variable end delimiter.
fn default_variable_end() -> String {
    "}}".to_string()
}

/// Default comment start delimiter.
fn default_comment_start() -> String {
    "{#".to_string()
}

/// Default comment end delimiter.
fn default_comment_end() -> String {
    "#}".to_string()
}

impl From<TemplateSyntax> for minijinja::Syntax {
    fn from(syntax: TemplateSyntax) -> Self {
        minijinja::Syntax {
            block_start: syntax.block_start.into(),
            block_end: syntax.block_end.into(),
            variable_start: syntax.variable_start.into(),
            variable_end: syntax.variable_end.into(),
            comment_start: syntax.comment_start.into(),
            comment_end: syntax.comment_end.into(),
        }
    }
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

impl Default for CaseConvention {
    /// Default case convention is PascalCase
    fn default() -> Self {
        CaseConvention::PascalCase
    }
}

impl CaseConvention {
    pub fn convert(&self, text: &str) -> String {
        let text = text.replace('.', "_");
        match self {
            CaseConvention::LowerCase => text.to_case(Case::Lower),
            CaseConvention::UpperCase => text.to_case(Case::Upper),
            CaseConvention::PascalCase => text.to_case(Case::Pascal),
            CaseConvention::CamelCase => text.to_case(Case::Camel),
            CaseConvention::SnakeCase => text.to_case(Case::Snake),
            CaseConvention::ScreamingSnakeCase => text.to_case(Case::ScreamingSnake),
            CaseConvention::KebabCase => text.to_case(Case::Kebab),
            CaseConvention::ScreamingKebabCase => text.to_case(Case::Cobol),
        }
    }
}

impl TargetConfig {
    pub fn try_new(lang_path: &Path) -> Result<TargetConfig, Error> {
        let config_file = lang_path.join("weaver.yaml");
        if config_file.exists() {
            let reader =
                std::fs::File::open(config_file.clone()).map_err(|e| InvalidConfigFile {
                    config_file: config_file.clone(),
                    error: e.to_string(),
                })?;
            serde_yaml::from_reader(reader).map_err(|e| InvalidConfigFile {
                config_file: config_file.clone(),
                error: e.to_string(),
            })
        } else {
            Ok(TargetConfig::default())
        }
    }
}