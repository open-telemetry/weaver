// SPDX-License-Identifier: Apache-2.0

//! Client SDK generator

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, process};

use glob::{glob, Paths};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use tera::{Context, Tera};
use weaver_cache::Cache;

use weaver_common::Logger;
use weaver_resolver::SchemaResolver;
use weaver_schema::event::Event;
use weaver_schema::metric_group::MetricGroup;
use weaver_schema::span::Span;
use weaver_schema::univariate_metric::UnivariateMetric;
use weaver_schema::TelemetrySchema;

use crate::config::{DynamicGlobalConfig, LanguageConfig};
use crate::Error::{
    InternalError, InvalidTelemetrySchema, InvalidTemplate, InvalidTemplateDirectory,
    InvalidTemplateFile, LanguageNotSupported, TemplateFileNameUndefined, WriteGeneratedCodeFailed,
};
use crate::{filters, functions, testers, GeneratorConfig};

/// Client SDK generator
pub struct ClientSdkGenerator {
    /// Language path
    lang_path: PathBuf,

    /// Tera template engine
    tera: Tera,

    /// Global configuration
    config: Arc<DynamicGlobalConfig>,
}

/// A pair {template, object} to generate code for.
enum TemplateObjectPair<'a> {
    Metric {
        template: String,
        metric: &'a UnivariateMetric,
    },
    MetricGroup {
        template: String,
        metric_group: &'a MetricGroup,
    },
    Event {
        template: String,
        event: &'a Event,
    },
    Span {
        template: String,
        span: &'a Span,
    },
    Other {
        template: String,
        relative_path: PathBuf,
        object: &'a TelemetrySchema,
    },
}

impl ClientSdkGenerator {
    /// Create a new client SDK generator for the given language
    /// or return an error if the language is not supported.
    pub fn try_new(language: &str, config: GeneratorConfig) -> Result<Self, crate::Error> {
        // Check if the language is supported
        // A language is supported if a template directory exists for it.
        let lang_path = config.template_dir.join(language);

        if !lang_path.exists() {
            return Err(LanguageNotSupported(language.to_owned()));
        }

        let lang_dir_tree = match lang_path.to_str() {
            None => {
                return Err(InvalidTemplateDirectory(lang_path));
            }
            Some(dir) => {
                format!("{}/**/*.tera", dir)
            }
        };

        let mut tera = match Tera::new(&lang_dir_tree) {
            Ok(tera) => tera,
            Err(e) => {
                return Err(InvalidTemplate {
                    template: lang_path,
                    error: format!("{}", e),
                });
            }
        };

        let lang_config = LanguageConfig::try_new(&lang_path)?;

        let config = Arc::new(DynamicGlobalConfig::default());

        // Register custom filters
        tera.register_filter(
            "file_name",
            filters::CaseConverter::new(lang_config.file_name, "file_name"),
        );
        tera.register_filter(
            "function_name",
            filters::CaseConverter::new(lang_config.function_name, "function_name"),
        );
        tera.register_filter(
            "arg_name",
            filters::CaseConverter::new(lang_config.arg_name, "arg_name"),
        );
        tera.register_filter(
            "struct_name",
            filters::CaseConverter::new(lang_config.struct_name, "struct_name"),
        );
        tera.register_filter(
            "field_name",
            filters::CaseConverter::new(lang_config.field_name, "field_name"),
        );
        tera.register_filter("unique_attributes", filters::unique_attributes);
        tera.register_filter("instrument", filters::instrument);
        tera.register_filter("required", filters::required);
        tera.register_filter("not_required", filters::not_required);
        tera.register_filter("value", filters::value);
        tera.register_filter("with_value", filters::with_value);
        tera.register_filter("without_value", filters::without_value);
        tera.register_filter("with_enum", filters::with_enum);
        tera.register_filter("without_enum", filters::without_enum);
        tera.register_filter("comment", filters::comment);
        tera.register_filter(
            "type_mapping",
            filters::TypeMapping {
                type_mapping: lang_config.type_mapping,
            },
        );

        // Register custom functions
        tera.register_function("config", functions::FunctionConfig::new(config.clone()));

        // Register custom testers
        tera.register_tester("required", testers::is_required);
        tera.register_tester("not_required", testers::is_not_required);

        Ok(Self {
            lang_path,
            tera,
            config,
        })
    }

    /// Generate a client SDK for the given schema
    pub fn generate(
        &self,
        log: impl Logger + Clone + Sync,
        schema_path: PathBuf,
        output_dir: PathBuf,
    ) -> Result<(), crate::Error> {
        let cache = Cache::try_new().unwrap_or_else(|e| {
            log.error(&e.to_string());
            process::exit(1);
        });

        let schema = SchemaResolver::resolve_schema_file(schema_path.clone(), &cache, log.clone())
            .map_err(|e| InvalidTelemetrySchema {
                schema: schema_path.clone(),
                error: format!("{}", e),
            })?;

        // Process recursively all files in the template directory
        let mut lang_path = self.lang_path.to_str().unwrap_or_default().to_owned();
        let paths = if lang_path.is_empty() {
            glob("**/*.tera").map_err(|e| InternalError(e.to_string()))?
        } else {
            lang_path.push_str("/**/*.tera");
            glob(lang_path.as_str()).map_err(|e| InternalError(e.to_string()))?
        };

        // Build the list of all {template, object} pairs to generate code for
        // and process them in parallel.
        // All pairs are independent from each other so we can process them in parallel.
        self.list_all_templates(&schema, paths)?
            .into_par_iter()
            .try_for_each(|pair| {
                match pair {
                    TemplateObjectPair::Metric { template, metric } => self.process_metric(
                        log.clone(),
                        &template,
                        &schema_path,
                        metric,
                        &output_dir,
                    ),
                    TemplateObjectPair::MetricGroup {
                        template,
                        metric_group,
                    } => self.process_metric_group(
                        log.clone(),
                        &template,
                        &schema_path,
                        metric_group,
                        &output_dir,
                    ),
                    TemplateObjectPair::Event { template, event } => {
                        self.process_event(log.clone(), &template, &schema_path, event, &output_dir)
                    }
                    TemplateObjectPair::Span { template, span } => {
                        self.process_span(log.clone(), &template, &schema_path, span, &output_dir)
                    }
                    TemplateObjectPair::Other {
                        template,
                        relative_path,
                        object,
                    } => {
                        // Process other templates
                        let context = &Context::from_serialize(object).map_err(|e| {
                            InvalidTelemetrySchema {
                                schema: schema_path.clone(),
                                error: format!("{}", e),
                            }
                        })?;

                        log.loading(&format!("Generating file {}", template));
                        let generated_code = self.generate_code(log.clone(), &template, context)?;
                        let relative_path = relative_path.to_path_buf();
                        let generated_file =
                            Self::save_generated_code(&output_dir, relative_path, generated_code)?;
                        log.success(&format!("Generated file {:?}", generated_file));
                        Ok(())
                    }
                }
            })?;

        Ok(())
    }

    /// Lists all {template, object} pairs derived from a template directory and a given
    /// schema specification.
    fn list_all_templates<'a>(
        &self,
        schema: &'a TelemetrySchema,
        paths: Paths,
    ) -> Result<Vec<TemplateObjectPair<'a>>, crate::Error> {
        let mut templates = Vec::new();
        if let Some(schema_spec) = &schema.schema {
            for entry in paths {
                if let Ok(tmpl_file_path) = entry {
                    if tmpl_file_path.is_dir() {
                        continue;
                    }
                    let relative_path = tmpl_file_path
                        .strip_prefix(&self.lang_path)
                        .map_err(|e| InternalError(e.to_string()))?;
                    let tmpl_file = relative_path
                        .to_str()
                        .map(|path| path.replace('\\', "/"))
                        .ok_or(InvalidTemplateFile(tmpl_file_path.clone()))?;

                    if tmpl_file.ends_with(".macro.tera") {
                        // Macro files are not templates.
                        // They are included in other templates.
                        // So we skip them.
                        continue;
                    }

                    match tmpl_file_path.file_stem().and_then(|s| s.to_str()) {
                        Some("metric") => {
                            if let Some(resource_metrics) = schema_spec.resource_metrics.as_ref() {
                                for metric in resource_metrics.metrics.iter() {
                                    templates.push(TemplateObjectPair::Metric {
                                        template: tmpl_file.clone(),
                                        metric,
                                    });
                                }
                            }
                        }
                        Some("metric_group") => {
                            if let Some(resource_metrics) = schema_spec.resource_metrics.as_ref() {
                                for metric_group in resource_metrics.metric_groups.iter() {
                                    templates.push(TemplateObjectPair::MetricGroup {
                                        template: tmpl_file.clone(),
                                        metric_group,
                                    });
                                }
                            }
                        }
                        Some("event") => {
                            if let Some(events) = schema_spec.resource_events.as_ref() {
                                for event in events.events.iter() {
                                    templates.push(TemplateObjectPair::Event {
                                        template: tmpl_file.clone(),
                                        event,
                                    });
                                }
                            }
                        }
                        Some("span") => {
                            if let Some(spans) = schema_spec.resource_spans.as_ref() {
                                for span in spans.spans.iter() {
                                    templates.push(TemplateObjectPair::Span {
                                        template: tmpl_file.clone(),
                                        span,
                                    });
                                }
                            }
                        }
                        _ => {
                            // Remove the `tera` extension from the relative path
                            let mut relative_path = relative_path.to_path_buf();
                            _ = relative_path.set_extension("");

                            templates.push(TemplateObjectPair::Other {
                                template: tmpl_file.clone(),
                                relative_path,
                                object: schema,
                            });
                        }
                    }
                } else {
                    return Err(InvalidTemplateDirectory(self.lang_path.clone()));
                }
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
    ) -> Result<String, crate::Error> {
        let generated_code = self.tera.render(tmpl_file, context).unwrap_or_else(|err| {
            log.newline(1);
            log.error(&format!("{}", err));
            let mut cause = err.source();
            while let Some(e) = cause {
                log.error(&format!("- caused by: {}", e));
                cause = e.source();
            }
            process::exit(1);
        });

        Ok(generated_code)
    }

    /// Save the generated code to the output directory.
    fn save_generated_code(
        output_dir: &Path,
        relative_path: PathBuf,
        generated_code: String,
    ) -> Result<PathBuf, crate::Error> {
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

    /// Process an univariate metric.
    fn process_metric(
        &self,
        log: impl Logger + Clone,
        tmpl_file: &str,
        schema_path: &Path,
        metric: &UnivariateMetric,
        output_dir: &Path,
    ) -> Result<(), crate::Error> {
        if let UnivariateMetric::Metric { name, .. } = metric {
            let context = &Context::from_serialize(metric).map_err(|e| InvalidTelemetrySchema {
                schema: schema_path.to_path_buf(),
                error: format!("{}", e),
            })?;

            // Reset the config
            self.config.reset();

            log.loading(&format!("Generating code for univariate metric `{}`", name));
            let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;

            // Retrieve the file name from the config
            let relative_path = {
                match &self.config.get() {
                    None => {
                        return Err(TemplateFileNameUndefined {
                            template: PathBuf::from(tmpl_file),
                        });
                    }
                    Some(file_name) => PathBuf::from(file_name.clone()),
                }
            };

            // Save the generated code to the output directory
            let generated_file =
                Self::save_generated_code(output_dir, relative_path, generated_code)?;
            log.success(&format!("Generated file {:?}", generated_file));
        }

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
    ) -> Result<(), crate::Error> {
        let context = &Context::from_serialize(metric).map_err(|e| InvalidTelemetrySchema {
            schema: schema_path.to_path_buf(),
            error: format!("{}", e),
        })?;

        // Reset the config
        self.config.reset();

        log.loading(&format!(
            "Generating code for multivariate metric `{}`",
            metric.name
        ));
        let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;

        // Retrieve the file name from the config
        let relative_path = {
            match self.config.get() {
                None => {
                    return Err(TemplateFileNameUndefined {
                        template: PathBuf::from(tmpl_file),
                    });
                }
                Some(file_name) => PathBuf::from(file_name.clone()),
            }
        };

        // Save the generated code to the output directory
        let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        log.success(&format!("Generated file {:?}", generated_file));

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
    ) -> Result<(), crate::Error> {
        let context = &Context::from_serialize(event).map_err(|e| InvalidTelemetrySchema {
            schema: schema_path.to_path_buf(),
            error: format!("{}", e),
        })?;

        // Reset the config
        self.config.reset();

        log.loading(&format!("Generating code for log `{}`", event.event_name));
        let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;

        // Retrieve the file name from the config
        let relative_path = {
            match self.config.get() {
                None => {
                    return Err(TemplateFileNameUndefined {
                        template: PathBuf::from(tmpl_file),
                    });
                }
                Some(file_name) => PathBuf::from(file_name.clone()),
            }
        };

        // Save the generated code to the output directory
        let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        log.success(&format!("Generated file {:?}", generated_file));

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
    ) -> Result<(), crate::Error> {
        let context = &Context::from_serialize(span).map_err(|e| InvalidTelemetrySchema {
            schema: schema_path.to_path_buf(),
            error: format!("{}", e),
        })?;

        // Reset the config
        self.config.reset();

        log.loading(&format!("Generating code for span `{}`", span.span_name));
        let generated_code = self.generate_code(log.clone(), tmpl_file, context)?;

        // Retrieve the file name from the config
        let relative_path = {
            match self.config.get() {
                None => {
                    return Err(TemplateFileNameUndefined {
                        template: PathBuf::from(tmpl_file),
                    });
                }
                Some(file_name) => PathBuf::from(file_name.clone()),
            }
        };

        // Save the generated code to the output directory
        let generated_file = Self::save_generated_code(output_dir, relative_path, generated_code)?;
        log.success(&format!("Generated file {:?}", generated_file));

        Ok(())
    }
}
