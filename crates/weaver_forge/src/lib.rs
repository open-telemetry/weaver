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

use std::{fs, process};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::task::Context;

use glob::{glob, Paths};
use minijinja::{Environment, path_loader};
use minijinja::filters::abs;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;

use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_resolved_schema::registry::{Group, Registry, TypedGroup};
use weaver_resolver::SchemaResolver;
use weaver_schema::event::Event;
use weaver_schema::metric_group::MetricGroup;
use weaver_schema::span::Span;

use crate::config::{DynamicGlobalConfig, TargetConfig};
use crate::Error::{InternalError, InvalidTelemetrySchema, InvalidTemplateDir, InvalidTemplateDirectory, InvalidTemplateFile, TargetNotSupported, WriteGeneratedCodeFailed};
use crate::extensions::case_converter::case_converter;

mod config;
mod extensions;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Invalid config file.
    #[error("Invalid config file `{config_file}`: {error}")]
    InvalidConfigFile {
        /// Config file.
        config_file: PathBuf,
        /// Error message.
        error: String,
    },

    /// Target not found.
    #[error(
    "Target `{target}` not found in `{root_path}`. Use the command `targets` to list supported targets."
    )]
    TargetNotSupported {
        /// Root path.
        root_path: String,
        /// Target name.
        target: String,
    },

    /// Invalid template directory.
    #[error("Invalid template directory: {0}")]
    InvalidTemplateDir(PathBuf),

    /// Invalid telemetry schema.
    #[error("Invalid telemetry schema {schema}: {error}")]
    InvalidTelemetrySchema {
        /// Schema file.
        schema: PathBuf,
        /// Error message.
        error: String,
    },

    /// Invalid template file.
    #[error("Invalid template file '{template}': {error}")]
    InvalidTemplateFile {
        template: PathBuf,
        error: String,
    },

    /// Invalid template directory.
    #[error("Invalid template directory: {0}")]
    InvalidTemplateDirectory(PathBuf),

    /// Internal error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Template file name undefined.
    #[error("File name undefined in the template `{template}`. To resolve this, use the function `config(file_name = <file_name, filter, or expression>)` to set the file name.")]
    TemplateFileNameUndefined {
        /// Template path.
        template: PathBuf,
    },

    /// Write generated code failed.
    #[error("Writing of the generated code {template} failed: {error}")]
    WriteGeneratedCodeFailed {
        /// Template path.
        template: PathBuf,
        /// Error message.
        error: String,
    },

    /// A generic container for multiple errors.
    #[error("Errors:\n{0:#?}")]
    CompoundError(Vec<Error>),
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

/// A pair {template, object} to generate code for.
#[derive(Debug)]
enum TemplateObjectPair<'a> {
    Group {
        absolute_template_path: PathBuf,
        relative_template_path: PathBuf,
        group: &'a Group,
    },
    Groups {
        absolute_template_path: PathBuf,
        relative_template_path: PathBuf,
        groups: Vec<&'a Group>,
    },
    Registry {
        absolute_template_path: PathBuf,
        relative_template_path: PathBuf,
        registry: &'a Registry,
    },
}

/// Template engine for generating artifacts from a semantic convention
/// registry and telemetry schema.
pub struct TemplateEngine {
    /// Template path
    path: PathBuf,

    /// Target configuration
    target_config: TargetConfig,

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
            return Err(TargetNotSupported {
                root_path: config.root_dir.to_string_lossy().to_string(),
                target: target.to_string(),
            });
        }

        Ok(Self {
            path: target_path.clone(),
            target_config: TargetConfig::try_new(&target_path)?,
            config: Arc::new(DynamicGlobalConfig::default()),
        })
    }

    // ToDo Refactor InternalError
    // ToDo Use compound error

    /// Generate assets from a semantic convention registry.
    pub fn generate_registry(
        &self,
        log: impl Logger + Clone + Sync,
        registry: &Registry,
        output_dir: PathBuf,
    ) -> Result<(), Error> {
        let cache = Cache::try_new().unwrap_or_else(|e| {
            _ = log.error(&e.to_string());
            process::exit(1);
        });

        // Process recursively all files in the template directory
        let mut lang_path = self.path.to_str().unwrap_or_default().to_string();
        let paths = if lang_path.is_empty() {
            glob("**/*").map_err(|e| InternalError(e.to_string()))?
        } else {
            lang_path.push_str("/**/*");
            glob(lang_path.as_str()).map_err(|e| InternalError(e.to_string()))?
        };

        // List all {template, object} pairs to run in parallel the template
        // engine as all pairs are independent.
        self.list_registry_templates(&registry, paths)?
            .into_par_iter()
            .try_for_each(|pair| {
                match pair {
                    TemplateObjectPair::Group {
                        absolute_template_path,
                        relative_template_path,
                        group
                    } => {
                        let ctx: serde_json::Value = serde_json::to_value(group)
                            .map_err(|e| InternalError(e.to_string()))?;
                        self.evaluate_template(log.clone(), ctx, absolute_template_path, relative_template_path, &output_dir)
                    },
                    TemplateObjectPair::Groups {
                        absolute_template_path,
                        relative_template_path,
                        groups
                    } => {
                        let ctx: serde_json::Value = serde_json::to_value(groups)
                            .map_err(|e| InternalError(e.to_string()))?;
                        self.evaluate_template(log.clone(), ctx, absolute_template_path, relative_template_path, &output_dir)
                    },
                    TemplateObjectPair::Registry {
                        absolute_template_path,
                        relative_template_path,
                        registry,
                    } => {
                        let ctx: serde_json::Value = serde_json::to_value(registry)
                            .map_err(|e| InternalError(e.to_string()))?;
                        self.evaluate_template(log.clone(), ctx, absolute_template_path, relative_template_path, &output_dir)
                    }
                }
            })?;

        Ok(())
    }

    /// Generate assets from the templates.
    pub fn generate(
        &self,
        log: impl Logger + Clone + Sync,
        schema_path: PathBuf,
        output_dir: PathBuf,
    ) -> Result<(), Error> {
        let cache = Cache::try_new().unwrap_or_else(|e| {
            _ = log.error(&e.to_string());
            process::exit(1);
        });

        let schema = SchemaResolver::resolve_schema_file(schema_path.clone(), &cache, log.clone())
            .map_err(|e| InvalidTelemetrySchema {
                schema: schema_path.clone(),
                error: format!("{}", e),
            })?;

        // Process recursively all files in the template directory
        let mut lang_path = self.path.to_str().unwrap_or_default().to_string();
        let paths = if lang_path.is_empty() {
            glob("**/*.tera").map_err(|e| InternalError(e.to_string()))?
        } else {
            lang_path.push_str("/**/*.tera");
            glob(lang_path.as_str()).map_err(|e| InternalError(e.to_string()))?
        };

        // Build the list of all {template, object} pairs to generate code for
        // and process them in parallel.
        // All pairs are independent from each other so we can process them in parallel.
        // self.list_all_templates(&schema, paths)?
        //     .into_par_iter()
        //     .try_for_each(|pair| {
        //         match pair {
        //             TemplateObjectPair::Metric { template, metric } => self.process_metric(
        //                 log.clone(),
        //                 &template,
        //                 &schema_path,
        //                 metric,
        //                 &output_dir,
        //             ),
        //             TemplateObjectPair::MetricGroup {
        //                 template,
        //                 metric_group,
        //             } => self.process_metric_group(
        //                 log.clone(),
        //                 &template,
        //                 &schema_path,
        //                 metric_group,
        //                 &output_dir,
        //             ),
        //             TemplateObjectPair::Event { template, event } => {
        //                 self.process_event(log.clone(), &template, &schema_path, event, &output_dir)
        //             }
        //             TemplateObjectPair::Span { template, span } => {
        //                 self.process_span(log.clone(), &template, &schema_path, span, &output_dir)
        //             }
        //             TemplateObjectPair::Other {
        //                 template,
        //                 relative_path,
        //                 object,
        //             } => {
        //                 // Process other templates
        //                 // let context = &Context::from_serialize(object).map_err(|e| {
        //                 //     InvalidTelemetrySchema {
        //                 //         schema: schema_path.clone(),
        //                 //         error: format!("{}", e),
        //                 //     }
        //                 // })?;
        //                 //
        //                 // log.loading(&format!("Generating file {}", template));
        //                 // let generated_code = self.generate_code(log.clone(), &template, context)?;
        //                 // let relative_path = relative_path.to_path_buf();
        //                 // let generated_file =
        //                 //     Self::save_generated_code(&output_dir, relative_path, generated_code)?;
        //                 // log.success(&format!("Generated file {:?}", generated_file));
        //                 Ok(())
        //             }
        //         }
        //     })?;

        Ok(())
    }

    fn evaluate_template(&self,
                         log: impl Logger + Clone + Sync,
                         ctx: serde_json::Value,
                         absolute_template_path: PathBuf,
                         relative_template_path: PathBuf,
                         output_dir: &PathBuf,
    ) -> Result<(), Error> {
        let template_file_name = absolute_template_path.to_str().ok_or(InvalidTemplateFile{
            template: absolute_template_path.clone(),
            error: "".to_string()})?;
        let template_source = fs::read_to_string(&absolute_template_path).map_err(|e| InvalidTemplateFile{
            template: absolute_template_path.clone().into(),
            error: e.to_string()})?;
        let mut engine = self.template_engine();

        engine.add_template(template_file_name, &template_source).map_err(|e| InternalError(e.to_string()))?;

        _ = log.loading(&format!("Generating file {}", template_file_name));
        let output = engine.get_template(template_file_name).map_err(|e| InternalError(e.to_string()))?
            .render(ctx).map_err(|e| InternalError(e.to_string()))?;
        let generated_file =
            Self::save_generated_code(output_dir, relative_template_path, output)?;
        _ = log.success(&format!("Generated file {:?}", generated_file));
        Ok(())
    }

    fn template_engine(&self) -> Environment {
        let mut env = Environment::new();

        // Register custom filters
        env.add_filter("file_name", case_converter(self.target_config.file_name.clone()));
        env.add_filter("function_name", case_converter(self.target_config.function_name.clone()));
        env.add_filter("arg_name", case_converter(self.target_config.arg_name.clone()));
        env.add_filter("struct_name", case_converter(self.target_config.struct_name.clone()));
        env.add_filter("field_name", case_converter(self.target_config.field_name.clone()));
        // env.add_filter("unique_attributes", extensions::unique_attributes);
        // env.add_filter("instrument", extensions::instrument);
        // env.add_filter("required", extensions::required);
        // env.add_filter("not_required", extensions::not_required);
        // env.add_filter("value", extensions::value);
        // env.add_filter("with_value", extensions::with_value);
        // env.add_filter("without_value", extensions::without_value);
        // env.add_filter("with_enum", extensions::with_enum);
        // env.add_filter("without_enum", extensions::without_enum);
        // env.add_filter("comment", extensions::comment);
        // env.add_filter(
        //     "type_mapping",
        //     extensions::TypeMapping {
        //         type_mapping: target_config.type_mapping,
        //     },
        // );

        // Register custom functions
        // tera.register_function("config", functions::FunctionConfig::new(config.clone()));

        // Register custom testers
        // tera.register_tester("required", testers::is_required);
        // tera.register_tester("not_required", testers::is_not_required);
        env
    }

    /// Lists all {template, object} pairs derived from a template directory and a given
    /// semantic convention registry.
    fn list_registry_templates<'a>(
        &self,
        registry: &'a Registry,
        paths: Paths,
    ) -> Result<Vec<TemplateObjectPair<'a>>, Error> {
        let mut templates = Vec::new();

        for entry in paths {
            if let Ok(tmpl_file_path) = entry {
                if tmpl_file_path.is_dir() {
                    continue;
                }
                let relative_path = tmpl_file_path
                    .strip_prefix(&self.path)
                    .map_err(|e| InvalidTemplateDir(self.path.clone()))?;
                let tmpl_file = tmpl_file_path
                    .to_str()
                    .ok_or(InvalidTemplateFile{template: tmpl_file_path.clone(), error: "".to_string() })?;

                if tmpl_file.ends_with(".macro.j2") {
                    // Macro files are not templates.
                    // They are included in other templates.
                    // So we skip them.
                    continue;
                }

                if tmpl_file.ends_with("weaver.yaml") {
                    // Skip weaver configuration file.
                    continue;
                }

                match tmpl_file_path.file_stem().and_then(|s| s.to_str()) {
                    Some("attribute_group") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::AttributeGroup { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file_path.to_path_buf(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("event") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::Event { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file_path.to_path_buf(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("group") => {
                        registry.groups.iter()
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file_path.to_path_buf(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("metric") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::Metric { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file.into(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("metric_group") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::MetricGroup { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file.into(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("registry") => {
                        templates.push(TemplateObjectPair::Registry {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            registry,
                        });
                    }
                    Some("resource") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::Resource { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file.into(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("scope") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::Scope { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file.into(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("span") => {
                        registry.groups.iter()
                            .filter(|group| if let TypedGroup::Span { .. } = group.typed_group { true } else { false })
                            .for_each(|group| {
                                templates.push(TemplateObjectPair::Group {
                                    absolute_template_path: tmpl_file.into(),
                                    relative_template_path: relative_path.to_path_buf(),
                                    group,
                                })
                            });
                    }
                    Some("attribute_groups") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::AttributeGroup { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("events") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::Event { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("groups") => {
                        let groups = registry.groups.iter()
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("metrics") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::Metric { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("metric_groups") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::MetricGroup { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("resources") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::Resource { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("scopes") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::Scope { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    Some("spans") => {
                        let groups = registry.groups.iter()
                            .filter(|group| if let TypedGroup::Span { .. } = group.typed_group { true } else { false })
                            .collect::<Vec<&Group>>();
                        templates.push(TemplateObjectPair::Groups {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            groups,
                        })
                    }
                    _ => {
                        templates.push(TemplateObjectPair::Registry {
                            absolute_template_path: tmpl_file.into(),
                            relative_template_path: relative_path.to_path_buf(),
                            registry,
                        })
                    }
                }
            } else {
                return Err(InvalidTemplateDirectory(self.path.clone()));
            }
        }

        Ok(templates)
    }


    /// Generate code.
    fn generate_code(
        &self,
        log: impl Logger,
        tmpl_file: &str,
        context: &Context,
    ) -> Result<String, Error> {
        // let generated_code = self.tera.render(tmpl_file, context).unwrap_or_else(|err| {
        //     log.newline(1);
        //     log.error(&format!("{}", err));
        //     let mut cause = err.source();
        //     while let Some(e) = cause {
        //         log.error(&format!("- caused by: {}", e));
        //         cause = e.source();
        //     }
        //     process::exit(1);
        // });
        let generated_code = "".to_string();

        Ok(generated_code)
    }

    /// Save the generated code to the output directory.
    fn save_generated_code(
        output_dir: &Path,
        relative_path: PathBuf,
        generated_code: String,
    ) -> Result<PathBuf, Error> {
        // Create all intermediary directories if they don't exist
        let output_file_path = output_dir.join(relative_path);
        if let Some(parent_dir) = output_file_path.parent() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                return Err(WriteGeneratedCodeFailed {
                    template: output_file_path.clone(),
                    error: format!("{}", e),
                });
            }
        }

        // Write the generated code to the output directory
        fs::write(output_file_path.clone(), generated_code).map_err(|e| {
            WriteGeneratedCodeFailed {
                template: output_file_path.clone(),
                error: format!("{}", e),
            }
        })?;

        Ok(output_file_path)
    }

    /// Process a registry group.
    fn process_group(
        &self,
        log: impl Logger + Clone,
        tmpl_file: &PathBuf,
        group: &Group,
        output_dir: &Path,
    ) -> Result<(), Error> {
        // if let UnivariateMetric::Metric { name, .. } = metric {
        //     let context = &Context::from_serialize(metric).map_err(|e| InvalidTelemetrySchema {
        //         schema: schema_path.to_path_buf(),
        //         error: format!("{}", e),
        //     })?;
        //
        //     // Reset the config
        //     self.config.reset();
        //
        //     log.loading(&format!("Generating code for univariate metric `{}`", name));
        //     let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;
        //
        //     // Retrieve the file name from the config
        //     let relative_path = {
        //         match &self.config.get() {
        //             None => {
        //                 return Err(TemplateFileNameUndefined {
        //                     template: PathBuf::from(tmpl_file),
        //                 });
        //             }
        //             Some(file_name) => PathBuf::from(file_name.clone()),
        //         }
        //     };
        //
        //     // Save the generated code to the output directory
        //     let generated_file =
        //         Self::save_generated_code(output_dir, relative_path, generated_code)?;
        //     log.success(&format!("Generated file {:?}", generated_file));
        // }

        Ok(())
    }

    /// Process a metric group (multivariate).
    fn process_metric_group(
        &self,
        log: impl Logger + Clone,
        tmpl_file: &str,
        schema_path: &Path,
        metric: &MetricGroup,
        output_dir: &Path,
    ) -> Result<(), Error> {
        // let context = &Context::from_serialize(metric).map_err(|e| InvalidTelemetrySchema {
        //     schema: schema_path.to_path_buf(),
        //     error: format!("{}", e),
        // })?;
        //
        // // Reset the config
        // self.config.reset();
        //
        // log.loading(&format!(
        //     "Generating code for multivariate metric `{}`",
        //     metric.name
        // ));
        // let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;
        //
        // // Retrieve the file name from the config
        // let relative_path = {
        //     match self.config.get() {
        //         None => {
        //             return Err(TemplateFileNameUndefined {
        //                 template: PathBuf::from(tmpl_file),
        //             });
        //         }
        //         Some(file_name) => PathBuf::from(file_name.clone()),
        //     }
        // };
        //
        // // Save the generated code to the output directory
        // let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        // log.success(&format!("Generated file {:?}", generated_file));

        Ok(())
    }

    /// Process an event.
    fn process_event(
        &self,
        log: impl Logger + Clone,
        tmpl_file: &str,
        schema_path: &Path,
        event: &Event,
        output_dir: &Path,
    ) -> Result<(), Error> {
        // let context = &Context::from_serialize(event).map_err(|e| InvalidTelemetrySchema {
        //     schema: schema_path.to_path_buf(),
        //     error: format!("{}", e),
        // })?;
        //
        // // Reset the config
        // self.config.reset();
        //
        // log.loading(&format!("Generating code for log `{}`", event.event_name));
        // let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;
        //
        // // Retrieve the file name from the config
        // let relative_path = {
        //     match self.config.get() {
        //         None => {
        //             return Err(TemplateFileNameUndefined {
        //                 template: PathBuf::from(tmpl_file),
        //             });
        //         }
        //         Some(file_name) => PathBuf::from(file_name.clone()),
        //     }
        // };
        //
        // // Save the generated code to the output directory
        // let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        // log.success(&format!("Generated file {:?}", generated_file));

        Ok(())
    }

    /// Process a span.
    fn process_span(
        &self,
        log: impl Logger + Clone,
        tmpl_file: &str,
        schema_path: &Path,
        span: &Span,
        output_dir: &Path,
    ) -> Result<(), Error> {
        // let context = &Context::from_serialize(span).map_err(|e| InvalidTelemetrySchema {
        //     schema: schema_path.to_path_buf(),
        //     error: format!("{}", e),
        // })?;
        //
        // // Reset the config
        // self.config.reset();
        //
        // log.loading(&format!("Generating code for span `{}`", span.span_name));
        // let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;
        //
        // // Retrieve the file name from the config
        // let relative_path = {
        //     match self.config.get() {
        //         None => {
        //             return Err(TemplateFileNameUndefined {
        //                 template: PathBuf::from(tmpl_file),
        //             });
        //         }
        //         Some(file_name) => PathBuf::from(file_name.clone()),
        //     }
        // };
        //
        // // Save the generated code to the output directory
        // let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        // log.success(&format!("Generated file {:?}", generated_file));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use weaver_logger::TestLogger;
    use weaver_resolver::attribute::AttributeCatalog;
    use weaver_resolver::registry::resolve_semconv_registry;
    use weaver_semconv::SemConvRegistry;

    #[test]
    fn test() {
        let logger = TestLogger::default();
        let engine = super::TemplateEngine::try_new(
            "test",
            super::GeneratorConfig::default(),
        ).expect("Failed to create template engine");

        let registry = SemConvRegistry::try_from_path("data/*.yaml").expect("Failed to load registry");
        let mut attr_catalog = AttributeCatalog::default();
        let resolved_registry =
            resolve_semconv_registry(&mut attr_catalog, "https://127.0.0.1", &registry)
                .expect("Failed to resolve registry");

        engine.generate_registry(
            logger,
            &resolved_registry,
            "output".into(),
        ).expect("Failed to generate registry assets");

        // let mut env = Environment::new();
        // env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
        // let template = env.get_template("hello.txt").unwrap();
        // println!("{}", template.render(context! { name => "World" }).unwrap());
    }
}