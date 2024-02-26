// SPDX-License-Identifier: Apache-2.0

//! This crate extends the MiniJinja template engine to add helper functions
//! and filters for working with semantic convention registries and telemetry
//! schemas.

#![deny(
missing_docs,
clippy::print_stdout,
unstable_features,
unused_import_braces,
unused_qualifications,
unused_results,
unused_extern_crates
)]

use std::path::PathBuf;
use std::sync::Arc;

use minijinja::{Environment, filters, functions, path_loader};

use crate::config::{DynamicGlobalConfig, TargetConfig};
use crate::Error::{InvalidTemplateDir, TargetNotSupported};

mod config;
mod extensions;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Target not found.
    #[error(
    "Target `{0}` is not supported. Use the command `targets` to list supported targets."
    )]
    TargetNotSupported(String),

    /// Invalid template directory.
    #[error("Invalid template directory: {0}")]
    InvalidTemplateDir(PathBuf),
}

/// General configuration for the generator.
pub struct GeneratorConfig {
    /// Root directory for the templates.
    root_dir: PathBuf,
}

impl Default for GeneratorConfig {
    /// Create a new generator configuration with default values.
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("templates"),
        }
    }
}

/// Template engine for generating artifacts from a semantic convention
/// registry and telemetry schema.
pub struct TemplateEngine {
    /// Template path
    path: PathBuf,

    /// Global configuration
    config: Arc<DynamicGlobalConfig>,
}

impl TemplateEngine {
    /// Create a new template engine for the given target or return an error if
    /// the target does not exist or is not a directory.
    pub fn try_new(target: &str, config: GeneratorConfig) -> Result<Self, Error> {
        // Check if the target is supported.
        // A target is supported if a template directory exists for it.
        let target_path = config.root_dir.join(target);

        if !target_path.exists() {
            return Err(TargetNotSupported(target.to_string()));
        }

        let mut env = Environment::new();
        env.set_loader(path_loader(target_path
            .to_str()
            .ok_or(Err(InvalidTemplateDir(target_path)))?
        ));

        let target_config = TargetConfig::try_new(&target_path)?;

        let config = Arc::new(DynamicGlobalConfig::default());

        // Register custom filters
        env.add_filter(
            "file_name",
            extensions::CaseConverter::new(target_config.file_name, "file_name"),
        );
        tera.register_filter(
            "function_name",
            extensions::CaseConverter::new(target_config.function_name, "function_name"),
        );
        tera.register_filter(
            "arg_name",
            extensions::CaseConverter::new(target_config.arg_name, "arg_name"),
        );
        tera.register_filter(
            "struct_name",
            extensions::CaseConverter::new(target_config.struct_name, "struct_name"),
        );
        tera.register_filter(
            "field_name",
            extensions::CaseConverter::new(target_config.field_name, "field_name"),
        );
        tera.register_filter("unique_attributes", extensions::unique_attributes);
        tera.register_filter("instrument", extensions::instrument);
        tera.register_filter("required", extensions::required);
        tera.register_filter("not_required", extensions::not_required);
        tera.register_filter("value", extensions::value);
        tera.register_filter("with_value", extensions::with_value);
        tera.register_filter("without_value", extensions::without_value);
        tera.register_filter("with_enum", extensions::with_enum);
        tera.register_filter("without_enum", extensions::without_enum);
        tera.register_filter("comment", extensions::comment);
        tera.register_filter(
            "type_mapping",
            extensions::TypeMapping {
                type_mapping: target_config.type_mapping,
            },
        );

        // Register custom functions
        tera.register_function("config", functions::FunctionConfig::new(config.clone()));

        // Register custom testers
        tera.register_tester("required", testers::is_required);
        tera.register_tester("not_required", testers::is_not_required);

        Ok(Self {
            lang_path: target_path,
            tera,
            config,
        })
    }
}

#[cfg(test)]
mod tests {
    use minijinja::{context, Environment};

    use super::*;

    #[test]
    fn test() {
        let mut env = Environment::new();
        env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
        let template = env.get_template("hello.txt").unwrap();
        println!("{}", template.render(context! { name => "World" }).unwrap());
    }
}