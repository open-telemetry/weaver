// SPDX-License-Identifier: Apache-2.0

//! Configuration for the template crate.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use convert_case::{Case, Casing};
use serde::Deserialize;
use thread_local::ThreadLocal;

use crate::Error;
use crate::Error::InvalidConfigFile;

/// Case convention for naming of functions and structs.
#[derive(Deserialize, Debug)]
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
}

/// Dynamic global configuration.
#[derive(Debug, Default)]
pub struct DynamicGlobalConfig {
    /// File name for the current generated code.
    pub file_name: ThreadLocal<RefCell<Option<String>>>,
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
        let config_file = lang_path.join("config.yaml");
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

impl DynamicGlobalConfig {
    /// Set the file name for the current generated code.
    /// This method uses a thread local variable to store the file name.
    pub fn set(&self, file_name: &str) {
        _ = self.file_name
            .get_or(|| RefCell::new(None))
            .borrow_mut()
            .replace(file_name.to_string());
    }

    /// Get the file name for the current generated code.
    /// This method uses a thread local variable to store the file name.
    pub fn get(&self) -> Option<String> {
        self.file_name
            .get_or(|| RefCell::new(None))
            .borrow()
            .clone()
    }

    /// Reset the file name for the current generated code.
    /// This method uses a thread local variable to store the file name.
    pub fn reset(&self) {
        _ = self.file_name
            .get_or(|| RefCell::new(None))
            .borrow_mut()
            .take();
    }
}
