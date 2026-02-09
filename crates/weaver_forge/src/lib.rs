// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fmt, fs};

use minijinja::syntax::SyntaxConfig;
use minijinja::value::{from_args, Enumerator, Object};
use minijinja::{Environment, ErrorKind, State, Value};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use serde::Serialize;

use error::Error;
use error::Error::{
    ContextSerializationFailed, InvalidTemplateFile, TemplateEvaluationFailed,
    WriteGeneratedCodeFailed,
};
use weaver_common::error::handle_errors;
use weaver_common::log_success;

use crate::config::{ApplicationMode, Params, TemplateConfig, WeaverConfig};
use crate::debug::error_summary;
use crate::error::Error::{InvalidConfigFile, InvalidFilePath};
use crate::extensions::{ansi, case, code, otel, util};
use crate::file_loader::FileLoader;
use crate::filter::Filter;
use crate::registry::{ResolvedGroup, ResolvedRegistry};

pub mod config;
pub mod debug;
pub mod error;
pub mod extensions;
pub mod file_loader;
mod filter;
mod formats;
pub mod jq;
pub mod output_processor;
pub mod registry;
pub mod v2;

pub use output_processor::OutputProcessor;

/// Name of the Weaver configuration file.
pub const WEAVER_YAML: &str = "weaver.yaml";

/// Default jq filter for the semantic convention registry.
pub const SEMCONV_JQ: &str = include_str!("../../../defaults/jq/semconv.jq");

// Definition of the Jinja syntax delimiters

/// Constant defining the start of a Jinja block.
pub const BLOCK_START: &str = "{%";

/// Constant defining the end of a Jinja block.
pub const BLOCK_END: &str = "%}";

/// Constant defining the start of a Jinja variable.
pub const VARIABLE_START: &str = "{{";

/// Constant defining the end of a Jinja variable.
pub const VARIABLE_END: &str = "}}";

/// Constant defining the start of a Jinja comment.
pub const COMMENT_START: &str = "{#";

/// Constant defining the end of a Jinja comment.
pub const COMMEND_END: &str = "#}";

/// Enumeration defining where the output of program execution should be directed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputDirective {
    /// Write the generated content to the standard output.
    Stdout,
    /// Write the generated content to the standard error.
    Stderr,
    /// Write the generated content to a file.
    File,
}

/// A template object accessible from the template.
#[derive(Debug, Clone)]
struct TemplateObject {
    file_name: Arc<Mutex<String>>,
}

impl TemplateObject {
    /// Get the file name of the template.
    fn file_name(&self) -> PathBuf {
        PathBuf::from(self.file_name.lock().expect("Lock poisoned").clone())
    }
}

impl Object for TemplateObject {
    fn call_method(
        self: &Arc<Self>,
        _state: &State<'_, '_>,
        name: &str,
        args: &[Value],
    ) -> Result<Value, minijinja::Error> {
        if name == "set_file_name" {
            let (file_name,): (&str,) = from_args(args)?;
            file_name.clone_into(&mut self.file_name.lock().expect("Lock poisoned"));
            Ok(Value::from(""))
        } else {
            Err(minijinja::Error::new(
                ErrorKind::UnknownMethod,
                format!("template has no method named {name}"),
            ))
        }
    }
}

/// A params object accessible from the template.
#[derive(Debug, Clone)]
struct ParamsObject {
    params: BTreeMap<String, Value>,
}

impl ParamsObject {
    /// Creates a new params object.
    pub(crate) fn new(params: BTreeMap<String, serde_yaml::Value>) -> Self {
        let mut new_params = BTreeMap::new();
        for (key, value) in params {
            _ = new_params.insert(key, Value::from_serialize(&value));
        }
        Self { params: new_params }
    }
}

impl Display for ParamsObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&format!("{:#?}", self.params))
    }
}

impl Object for ParamsObject {
    /// Given a key, looks up the associated value.
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = key.to_string();
        self.params.get(&key).cloned()
    }

    /// Enumerates the keys of the object.
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let keys: Vec<_> = self.params.keys().map(Value::from).collect();
        Enumerator::Values(keys)
    }
}

/// Runs raw JQ filter on a context object.
pub fn run_filter_raw<T: Serialize>(context: &T, filter: &str) -> Result<serde_json::Value, Error> {
    // Create a read-only context for the filter evaluations
    let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
        error: e.to_string(),
    })?;
    // Apply the filter
    let filter = Filter::new(filter);
    // TODO - create real filter params
    let filter_params = BTreeMap::new();
    let filtered_context = filter.apply(context, &filter_params)?;
    Ok(filtered_context)
}

/// Template engine for generating artifacts from a semantic convention
/// registry and telemetry schema.
pub struct TemplateEngine {
    /// File loader used by the engine.
    file_loader: Arc<dyn FileLoader + Send + Sync + 'static>,

    /// Target configuration
    target_config: WeaverConfig,

    // Global parameters for snippet generation.
    snippet_params: Params,
}

/// Global context for the template engine.
#[derive(Serialize, Debug)]
pub struct Context<'a> {
    /// The semantic convention registry.
    pub registry: &'a ResolvedRegistry,
    /// The group to generate doc or code for.
    pub group: Option<&'a ResolvedGroup>,
    /// The groups to generate doc or code for.
    pub groups: Option<Vec<&'a ResolvedGroup>>,
}

/// Global context for the template engine.
#[derive(Serialize, Debug)]
pub struct NewContext<'a> {
    /// The semantic convention registry.
    pub ctx: &'a serde_json::Value,
}

/// Convert a context into a serde_json::Value.
impl TryInto<serde_json::Value> for NewContext<'_> {
    type Error = Error;

    fn try_into(self) -> Result<serde_json::Value, Self::Error> {
        serde_json::to_value(self).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })
    }
}

impl TemplateEngine {
    /// Create a new template engine for the given Weaver config.
    pub fn try_new(
        mut config: WeaverConfig,
        loader: impl FileLoader + Send + Sync + 'static,
        params: Params,
    ) -> Result<Self, Error> {
        // Compute the params for each template based on:
        // - CLI-level params
        // - Top-level params in the `weaver.yaml` file
        if let Some(templates) = config.templates.as_mut() {
            for template in templates {
                let template_params = template.params.get_or_insert_with(BTreeMap::new);

                // Add CLI-level params to the template params. If a param is already defined
                // in the template params, the CLI-level param will override it.
                for (name, value) in params.params.iter() {
                    // The result of the insert method is ignored because we don't care about
                    // the previous value of the param.
                    _ = template_params.insert(name.clone(), value.clone());
                }

                // Add the params defined at the top level of the `weaver.yaml` file
                // to the local params if they are not already defined locally.
                if let Some(top_level_params) = config.params.as_ref() {
                    for (key, value) in top_level_params {
                        if !template_params.contains_key(key) {
                            // Note: The result of the insert method is ignored because we don't
                            // care about the previous value of the param.
                            _ = template_params.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
        }

        // Validate template files exist
        let mut errors = Vec::new();
        if let Some(templates) = config.templates.as_ref() {
            let all_files = loader.all_files();
            for template in templates {
                // Check if any files match the template glob pattern
                let matcher = template.template.compile_matcher();
                let has_matches = all_files.iter().any(|file| matcher.is_match(file));

                if !has_matches {
                    errors.push(InvalidTemplateFile {
                        template: PathBuf::from(template.template.glob()),
                        error: format!(
                            "Template pattern '{}' did not match any files",
                            template.template.glob()
                        ),
                    });
                }
            }
        }

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            file_loader: Arc::new(loader),
            target_config: config,
            snippet_params: params,
        })
    }

    /// Generate a template snippet from serializable context and a snippet identifier.
    ///
    /// # Arguments
    ///
    /// * `context` - The context to use when generating snippets.
    /// * `filter` - The jq filter expression to use.
    /// * `snippet_id` - The template to use when rendering the snippet.
    pub fn generate_snippet<T: Serialize>(
        &self,
        context: &T,
        filter: &str,
        snippet_id: String,
    ) -> Result<String, Error> {
        // TODO - find the snippet by id.

        // Create a read-only context for the filter evaluations
        let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })?;

        let mut engine = self.template_engine()?;
        // Create snippet parameters
        let mut params = self
            .target_config
            .params
            .as_ref()
            .cloned()
            .unwrap_or_default();

        for (name, value) in self.snippet_params.params.iter() {
            // The result of the insert method is ignored because we don't care about
            // the previous value of the param.
            _ = params.insert(name.clone(), value.clone());
        }
        // Apply the filter
        let filter = Filter::new(filter);
        let filter_params = Self::prepare_jq_context(&params)?;
        let filtered_context = filter.apply(context, &filter_params)?;
        engine.add_global("params", Value::from_object(ParamsObject::new(params)));
        let template = engine
            .get_template(&snippet_id)
            .map_err(error::jinja_err_convert)?;
        let result = template
            .render(filtered_context)
            .map_err(error::jinja_err_convert)?;
        Ok(result)
    }

    /// Generate artifacts from a serializable context and return the rendered
    /// output as a String instead of writing to files or stdout.
    ///
    /// This is useful when the output needs to be captured (e.g., for HTTP responses).
    pub fn generate_to_string<T: Serialize>(&self, context: &T) -> Result<String, Error> {
        let files = self.file_loader.all_files();
        let tmpl_matcher = self.target_config.template_matcher()?;

        let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })?;

        let mut results = Vec::new();
        for file_to_process in files {
            for template in tmpl_matcher.matches(file_to_process.clone()) {
                let yaml_params = Self::init_params(template.params.clone())?;
                let params = Self::prepare_jq_context(&yaml_params)?;
                let filter = Filter::new(template.filter.as_str());
                let filtered_result = filter.apply(context.clone(), &params)?;

                match template.application_mode {
                    ApplicationMode::Single => {
                        let is_empty = filtered_result.is_null()
                            || (filtered_result.is_array()
                                && filtered_result.as_array().expect("is_array").is_empty());
                        if !is_empty {
                            let (output, _) = self.render_template(
                                NewContext {
                                    ctx: &filtered_result,
                                }
                                .try_into()?,
                                &yaml_params,
                                &file_to_process,
                                None,
                            )?;
                            results.push(output);
                        }
                    }
                    ApplicationMode::Each => {
                        if let serde_json::Value::Array(values) = &filtered_result {
                            for value in values {
                                let (output, _) = self.render_template(
                                    NewContext { ctx: value }.try_into()?,
                                    &yaml_params,
                                    &file_to_process,
                                    None,
                                )?;
                                results.push(output);
                            }
                        } else {
                            let (output, _) = self.render_template(
                                NewContext {
                                    ctx: &filtered_result,
                                }
                                .try_into()?,
                                &yaml_params,
                                &file_to_process,
                                None,
                            )?;
                            results.push(output);
                        }
                    }
                }
            }
        }

        Ok(results.join(""))
    }

    /// Generate artifacts from a serializable context and a template directory,
    /// in parallel.
    ///
    /// # Arguments
    ///
    /// * `context` - The context to use for generating the artifacts.
    /// * `output_dir` - The directory where the generated artifacts will be saved.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the artifacts were generated successfully.
    /// * `Err(error)` if an error occurred during the generation of the artifacts.
    pub fn generate<T: Serialize>(
        &self,
        context: &T,
        output_dir: &Path,
        output_directive: &OutputDirective,
    ) -> Result<(), Error> {
        let files = self.file_loader.all_files();
        let tmpl_matcher = self.target_config.template_matcher()?;

        // Serialize the context in JSON
        let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })?;

        // Process each file and collect any errors.
        // The files are processed in parallel.
        let errs = files
            .into_par_iter()
            .flat_map(|file_to_process| {
                // Iterate over the all the template configurations that match the file
                // to process in parallel.
                tmpl_matcher
                    .matches(file_to_process.clone())
                    .into_par_iter()
                    .filter_map(|template| {
                        self.process_template(
                            &file_to_process,
                            template,
                            &context,
                            output_dir,
                            output_directive,
                        )
                        .err()
                    })
                    .collect::<Vec<Error>>()
            })
            .collect::<Vec<Error>>();

        handle_errors(errs)
    }

    /// Process a single template file with the given template configuration,
    /// context, output directory, and output directive.
    fn process_template(
        &self,
        template_file: &Path,
        template: &TemplateConfig,
        context: &serde_json::Value,
        output_dir: &Path,
        output_directive: &OutputDirective,
    ) -> Result<(), Error> {
        log::debug!(
            "Processing template file: {template_file:#?}, output directory: {output_dir:#?}"
        );

        let yaml_params = Self::init_params(template.params.clone())?;
        let params = Self::prepare_jq_context(&yaml_params)?;
        let filter = Filter::new(template.filter.as_str());
        let filtered_result = filter.apply(context.clone(), &params)?;

        match template.application_mode {
            ApplicationMode::Single => self.process_single_mode(
                &filtered_result,
                template.file_name.as_ref(),
                &yaml_params,
                template_file,
                output_dir,
                output_directive,
            ),
            ApplicationMode::Each => self.process_each_mode(
                &filtered_result,
                template.file_name.as_ref(),
                &yaml_params,
                template_file,
                output_dir,
                output_directive,
            ),
        }
    }

    /// Evaluate the template for each object in the context if the context is an array, otherwise
    /// evaluate the template for the context entire object.
    /// The evaluation is done in parallel.
    fn process_each_mode(
        &self,
        ctx: &serde_json::Value,
        file_path: Option<&String>,
        params: &BTreeMap<String, serde_yaml::Value>,
        template_file: &Path,
        output_dir: &Path,
        output_directive: &OutputDirective,
    ) -> Result<(), Error> {
        match ctx {
            serde_json::Value::Array(values) => {
                // Evaluate the template for each object in the array context in parallel
                let errs = values
                    .into_par_iter()
                    .filter_map(|result| {
                        self.evaluate_template(
                            NewContext { ctx: result }.try_into().ok()?,
                            file_path,
                            params,
                            template_file,
                            output_directive,
                            output_dir,
                        )
                        .err()
                    })
                    .collect::<Vec<Error>>();
                handle_errors(errs)
            }
            _ => self.evaluate_template(
                NewContext { ctx }.try_into()?,
                file_path,
                params,
                template_file,
                output_directive,
                output_dir,
            ),
        }
    }

    /// Evaluate the template for the entire context.
    fn process_single_mode(
        &self,
        ctx: &serde_json::Value,
        file_path: Option<&String>,
        params: &BTreeMap<String, serde_yaml::Value>,
        template_file: &Path,
        output_dir: &Path,
        output_directive: &OutputDirective,
    ) -> Result<(), Error> {
        if ctx.is_null() || (ctx.is_array() && ctx.as_array().expect("is_array").is_empty()) {
            // Skip the template evaluation if the filtered result is null or an empty array
            return Ok(());
        }
        self.evaluate_template(
            NewContext { ctx }.try_into()?,
            file_path,
            params,
            template_file,
            output_directive,
            output_dir,
        )
    }

    /// Build a JQ context from the Weaver parameters.
    fn prepare_jq_context(
        params: &BTreeMap<String, serde_yaml::Value>,
    ) -> Result<BTreeMap<String, serde_json::Value>, Error> {
        let mut errs = Vec::new();
        let jq_ctx: BTreeMap<String, serde_json::Value> = params
            .iter()
            .filter_map(|(k, v)| {
                let json_value = match serde_json::to_value(v) {
                    Ok(json_value) => json_value,
                    Err(e) => {
                        errs.push(ContextSerializationFailed {
                            error: e.to_string(),
                        });
                        return None;
                    }
                };
                Some((k.clone(), json_value))
            })
            .collect();
        handle_errors(errs)?;
        Ok(jq_ctx)
    }

    /// Initialize a map of parameters from the template parameters.
    /// If there are template parameters then the map returned contains the entry `params`
    /// initialized with an in-memory yaml representation of the template parameters.
    /// Otherwise, an empty map is returned if there is no template parameter.
    fn init_params(
        template_params: Option<BTreeMap<String, serde_yaml::Value>>,
    ) -> Result<BTreeMap<String, serde_yaml::Value>, Error> {
        if let Some(mut params) = template_params.clone() {
            let value =
                serde_yaml::to_value(template_params).map_err(|e| ContextSerializationFailed {
                    error: e.to_string(),
                })?;
            // The `params` parameter is a reserved entry within the `params` sections.
            // This parameter is automatically injected by Weaver to allow passing all parameters
            // to a JQ function or a Jinja filter without having to explicitly enumerate all the
            // parameters.
            // e.g. `semconv_grouped_attributes($params)` will pass all the parameters to the
            // `semconv_grouped_attributes` JQ function.
            if params.insert("params".to_owned(), value).is_some() {
                return Err(Error::DuplicateParamKey {
                    key: "params".to_owned(),
                    error: "The parameter `params` is a reserved parameter name".to_owned(),
                });
            }
            Ok(params)
        } else {
            Ok(BTreeMap::new())
        }
    }

    /// Set up a Jinja engine, render a template, and return the output string
    /// along with the `TemplateObject` (which may have been mutated by the
    /// template to override the file name).
    fn render_template(
        &self,
        ctx: serde_json::Value,
        params: &BTreeMap<String, serde_yaml::Value>,
        template_path: &Path,
        file_path_config: Option<&String>,
    ) -> Result<(String, TemplateObject), Error> {
        let mut engine = self.template_engine()?;

        // Add the Weaver parameters to the template context
        engine.add_global(
            "params",
            Value::from_object(ParamsObject::new(params.clone())),
        );

        // Pre-determine the file path for the generated file based on the template file path
        // if defined, otherwise use the default file path based on the template file name.
        let file_path = match file_path_config {
            Some(file_path) => {
                engine
                    .render_str(file_path, ctx.clone())
                    .map_err(|e| InvalidFilePath {
                        file_path: file_path.clone(),
                        error: e.to_string(),
                    })?
            }
            None => {
                // By default, the file name is the template file name without
                // the extension ".j2"
                template_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .trim_end_matches(".j2")
                    .to_owned()
            }
        };
        let template_object = TemplateObject {
            file_name: Arc::new(Mutex::new(file_path)),
        };
        let template_file = template_path.to_str().ok_or(InvalidTemplateFile {
            template: template_path.to_path_buf(),
            error: "".to_owned(),
        })?;

        // Add the handler to programmatically set the file name of the generated file
        // from the template.
        engine.add_global("template", Value::from_object(template_object.clone()));

        let template = engine.get_template(template_file).map_err(|e| {
            let templates = engine
                .templates()
                .map(|(name, _)| name.to_owned())
                .collect::<Vec<_>>();
            let error = format!("{e}. Available templates: {templates:?}");
            InvalidTemplateFile {
                template: template_file.into(),
                error,
            }
        })?;

        let output = template
            .render(ctx)
            .map_err(|e| TemplateEvaluationFailed {
                template: template_path.to_path_buf(),
                error_id: e.to_string(),
                error: error_summary(e),
            })?;

        Ok((output, template_object))
    }

    #[allow(clippy::print_stdout)] // This is used for the OutputDirective::Stdout variant
    #[allow(clippy::print_stderr)] // This is used for the OutputDirective::Stderr variant
    fn evaluate_template(
        &self,
        ctx: serde_json::Value,
        file_path: Option<&String>,
        params: &BTreeMap<String, serde_yaml::Value>,
        template_path: &Path,
        output_directive: &OutputDirective,
        output_dir: &Path,
    ) -> Result<(), Error> {
        let (output, template_object) =
            self.render_template(ctx, params, template_path, file_path)?;
        match output_directive {
            OutputDirective::Stdout => {
                println!("{output}");
            }
            OutputDirective::Stderr => {
                eprintln!("{output}");
            }
            OutputDirective::File => {
                let generated_file =
                    Self::save_generated_code(output_dir, template_object.file_name(), output)?;
                log_success(format!("Generated file {generated_file:?}"));
            }
        }
        Ok(())
    }

    /// Create a new template engine based on the target configuration.
    fn template_engine(&self) -> Result<Environment<'_>, Error> {
        let mut env = Environment::new();
        let template_syntax = self.target_config.template_syntax.clone();

        let syntax = SyntaxConfig::builder()
            .block_delimiters(
                Cow::Owned(
                    template_syntax
                        .block_start
                        .unwrap_or_else(|| BLOCK_START.to_owned()),
                ),
                Cow::Owned(
                    template_syntax
                        .block_end
                        .unwrap_or_else(|| BLOCK_END.to_owned()),
                ),
            )
            .variable_delimiters(
                Cow::Owned(
                    template_syntax
                        .variable_start
                        .unwrap_or_else(|| VARIABLE_START.to_owned()),
                ),
                Cow::Owned(
                    template_syntax
                        .variable_end
                        .unwrap_or_else(|| VARIABLE_END.to_owned()),
                ),
            )
            .comment_delimiters(
                Cow::Owned(
                    template_syntax
                        .comment_start
                        .unwrap_or_else(|| COMMENT_START.to_owned()),
                ),
                Cow::Owned(
                    template_syntax
                        .comment_end
                        .unwrap_or_else(|| COMMEND_END.to_owned()),
                ),
            )
            .build()
            .map_err(|e| InvalidConfigFile {
                config_file: PathBuf::from(OsString::from(&self.file_loader.root()))
                    .join(WEAVER_YAML),
                error: e.to_string(),
            })?;

        // Add minijinja contrib support
        minijinja_contrib::add_to_environment(&mut env);
        // Add minijinja py-compat support to improve compatibility with Python Jinja2
        env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);

        let file_loader = self.file_loader.clone();
        env.set_loader(move |name| {
            file_loader
                .load_file(name)
                .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))
                .map(|opt_file_content| opt_file_content.map(|file_content| file_content.content))
        });
        env.set_syntax(syntax);

        // Jinja whitespace control
        // https://docs.rs/minijinja/latest/minijinja/syntax/index.html#whitespace-control
        let whitespace_control = self.target_config.whitespace_control.clone();
        env.set_trim_blocks(whitespace_control.trim_blocks.unwrap_or_default());
        env.set_lstrip_blocks(whitespace_control.lstrip_blocks.unwrap_or_default());
        env.set_keep_trailing_newline(whitespace_control.keep_trailing_newline.unwrap_or_default());

        install_weaver_extensions(&mut env, &self.target_config, true)?;

        Ok(env)
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
                    error: format!("{e}"),
                });
            }
        }

        // Write the generated code to the output directory
        fs::write(output_file_path.clone(), generated_code).map_err(|e| {
            WriteGeneratedCodeFailed {
                template: output_file_path.clone(),
                error: format!("{e}"),
            }
        })?;

        Ok(output_file_path)
    }
}

/// Install all the Weaver extensions into the Jinja environment.
/// This includes the filters, functions, and tests.
pub(crate) fn install_weaver_extensions(
    env: &mut Environment<'_>,
    config: &WeaverConfig,
    comment_flag: bool,
) -> Result<(), Error> {
    code::add_filters(env, config, comment_flag)?;
    ansi::add_filters(env);
    case::add_filters(env);
    otel::add_filters(env);
    util::add_filters(env, config);
    util::add_functions(env);
    otel::add_tests(env);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use globset::Glob;
    use serde::Serialize;

    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_diff::diff_dir;
    use weaver_resolver::{LoadedSemconvRegistry, SchemaResolver};
    use weaver_semconv::registry_repo::RegistryRepo;

    use crate::config::{ApplicationMode, CaseConvention, Params, TemplateConfig, WeaverConfig};
    use crate::debug::print_dedup_errors;
    use crate::extensions::case::case_converter;
    use crate::file_loader::FileSystemFileLoader;
    use crate::registry::ResolvedRegistry;
    use crate::{run_filter_raw, OutputDirective, TemplateEngine};

    fn prepare_test(
        target: &str,
        cli_params: Params,
        ignore_non_fatal_errors: bool,
    ) -> (TemplateEngine, ResolvedRegistry, PathBuf, PathBuf) {
        let registry_id = "default";
        let path: VirtualDirectoryPath = "data/registry"
            .try_into()
            .expect("Invalid virtual directory path string");
        let repo =
            RegistryRepo::try_new(registry_id, &path).expect("Failed to construct repository");
        let registry_result = SchemaResolver::load_semconv_repository(repo, false);
        // SemConvRegistry::try_from_path_pattern(registry_id, "data/*.yaml");
        let registry = if ignore_non_fatal_errors {
            registry_result
                .into_result_with_non_fatal()
                .expect("Failed to load the registry")
                .0
        } else {
            registry_result
                .into_result_failing_non_fatal()
                .expect("Failed to load the registry")
        };
        prepare_test_with_registry(target, cli_params, registry)
    }

    fn prepare_test_with_registry(
        target: &str,
        cli_params: Params,
        registry: LoadedSemconvRegistry,
    ) -> (TemplateEngine, ResolvedRegistry, PathBuf, PathBuf) {
        let loader = FileSystemFileLoader::try_new("templates".into(), target)
            .expect("Failed to create file system loader");
        let config = WeaverConfig::try_from_path(format!("templates/{target}")).unwrap();
        let engine = TemplateEngine::try_new(config, loader, cli_params)
            .expect("Failed to create template engine");
        let schema = SchemaResolver::resolve(registry, false)
            .into_result_failing_non_fatal()
            .expect("Failed to resolve registry");

        let template_registry =
            ResolvedRegistry::try_from_resolved_registry(&schema.registry, schema.catalog())
                .unwrap_or_else(|e| {
                    panic!("Failed to create the context for the template evaluation: {e:?}")
                });

        // Delete all the files in the observed_output/target directory
        // before generating the new files.
        fs::remove_dir_all(format!("observed_output/{target}")).unwrap_or_default();

        (
            engine,
            template_registry,
            PathBuf::from(format!("observed_output/{target}")),
            PathBuf::from(format!("expected_output/{target}")),
        )
    }

    #[test]
    fn test_case_converter() {
        struct TestCase {
            input: &'static str,
            expected: &'static str,
            case: CaseConvention,
        }

        let test_cases = vec![
            TestCase {
                input: "ThisIsATest",
                expected: "this is a test",
                case: CaseConvention::LowerCase,
            },
            TestCase {
                input: "This is a K8S TEST",
                expected: "this is a k8s test",
                case: CaseConvention::LowerCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "THIS IS A TEST",
                case: CaseConvention::UpperCase,
            },
            TestCase {
                input: "This is a TEST",
                expected: "THIS IS A TEST",
                case: CaseConvention::UpperCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "This Is A Test",
                case: CaseConvention::TitleCase,
            },
            TestCase {
                input: "This is a k8s TEST",
                expected: "This Is A K8s Test",
                case: CaseConvention::TitleCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "this_is_a_test",
                case: CaseConvention::SnakeCase,
            },
            TestCase {
                input: "This is a test",
                expected: "this_is_a_test",
                case: CaseConvention::SnakeCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "ThisIsATest",
                case: CaseConvention::PascalCase,
            },
            TestCase {
                input: "This is a test",
                expected: "ThisIsATest",
                case: CaseConvention::PascalCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "thisIsATest",
                case: CaseConvention::CamelCase,
            },
            TestCase {
                input: "This is a test",
                expected: "thisIsATest",
                case: CaseConvention::CamelCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "this-is-a-test",
                case: CaseConvention::KebabCase,
            },
            TestCase {
                input: "This is a test",
                expected: "this-is-a-test",
                case: CaseConvention::KebabCase,
            },
            TestCase {
                input: "This is a k8s test",
                expected: "this-is-a-k8s-test",
                case: CaseConvention::KebabCase,
            },
            TestCase {
                input: "This is a K8S test",
                expected: "this-is-a-k8s-test",
                case: CaseConvention::KebabCase,
            },
            TestCase {
                input: "This is 2 K8S test",
                expected: "this-is-2-k8s-test",
                case: CaseConvention::KebabCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "THIS_IS_A_TEST",
                case: CaseConvention::ScreamingSnakeCase,
            },
            TestCase {
                input: "This is a test",
                expected: "THIS_IS_A_TEST",
                case: CaseConvention::ScreamingSnakeCase,
            },
            TestCase {
                input: "ThisIsATest",
                expected: "THIS-IS-A-TEST",
                case: CaseConvention::ScreamingKebabCase,
            },
            TestCase {
                input: "This is a test",
                expected: "THIS-IS-A-TEST",
                case: CaseConvention::ScreamingKebabCase,
            },
        ];

        for test_case in test_cases {
            let result = case_converter(test_case.case)(test_case.input);
            assert_eq!(result, test_case.expected);
        }
    }

    #[test]
    fn test_template_engine() {
        // we need to first clean "observed_output/test" of previous runs
        // We ignore failures because the directory may not exist yet.
        let _ = fs::remove_dir_all("observed_output/test");

        let loader = FileSystemFileLoader::try_new("templates".into(), "test")
            .expect("Failed to create file system loader");
        let config =
            WeaverConfig::try_from_loader(&loader).expect("Failed to load `templates/weaver.yaml`");
        let mut engine = TemplateEngine::try_new(config, loader, Params::default())
            .expect("Failed to create template engine");

        // Add a template configuration for converter.md on top
        // of the default template configuration. This is useful
        // for test coverage purposes.
        let mut templates = engine.target_config.templates.unwrap_or_default();
        templates.push(TemplateConfig {
            template: Glob::new("converter.md").unwrap(),
            filter: ".".to_owned(),
            application_mode: ApplicationMode::Single,
            params: None,
            file_name: None,
        });
        engine.target_config.templates = Some(templates);

        let registry_id = "default";
        let path: VirtualDirectoryPath = "data/registry"
            .try_into()
            .expect("Invalid virtual directory path string");
        let repo =
            RegistryRepo::try_new(registry_id, &path).expect("Failed to construct repository");
        let loaded = SchemaResolver::load_semconv_repository(repo, false)
            .into_result_with_non_fatal()
            .expect("Failed to load registry")
            .0;
        let schema = SchemaResolver::resolve(loaded, false)
            .into_result_failing_non_fatal()
            .expect("Failed to resolve registry");

        let template_registry =
            ResolvedRegistry::try_from_resolved_registry(&schema.registry, schema.catalog())
                .unwrap_or_else(|e| {
                    panic!("Failed to create the context for the template evaluation: {e:?}")
                });

        engine
            .generate(
                &template_registry,
                Path::new("observed_output/test"),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir("expected_output/test", "observed_output/test").unwrap());
    }

    #[test]
    fn test_whitespace_control() {
        let (engine, template_registry, observed_output, expected_output) =
            prepare_test("whitespace_control", Params::default(), true);

        engine
            .generate(
                &template_registry,
                observed_output.as_path(),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir(expected_output, observed_output).unwrap());
    }

    #[test]
    fn test_py_compat() {
        #[derive(Serialize)]
        struct Context {
            text: String,
        }

        let loader = FileSystemFileLoader::try_new("templates".into(), "py_compat")
            .expect("Failed to create file system loader");
        let config = WeaverConfig::try_from_loader(&loader).unwrap();
        let engine = TemplateEngine::try_new(config, loader, Params::default())
            .expect("Failed to create template engine");
        let context = Context {
            text: "Hello, World!".to_owned(),
        };

        engine
            .generate(
                &context,
                Path::new("observed_output/py_compat"),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir("expected_output/py_compat", "observed_output/py_compat").unwrap());
    }

    #[test]
    fn test_semconv_jq_functions() {
        let (engine, template_registry, observed_output, expected_output) =
            prepare_test("semconv_jq_fn", Params::default(), true);

        engine
            .generate(
                &template_registry,
                observed_output.as_path(),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir(expected_output, observed_output).unwrap());
    }

    #[test]
    fn test_template_params() {
        let cli_params = Params::from_key_value_pairs(&[
            (
                "param_config",
                serde_yaml::Value::String("cli_value".to_owned()),
            ),
            ("shared_2", serde_yaml::Value::Bool(true)),
        ]);
        let (engine, template_registry, observed_output, expected_output) =
            prepare_test("template_params", cli_params, true);

        engine
            .generate(
                &template_registry,
                observed_output.as_path(),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir(expected_output, observed_output).unwrap());
    }

    #[test]
    fn test_comment_format() {
        let registry_id = "default";
        let path: VirtualDirectoryPath = "data/mini_registry_for_comments"
            .try_into()
            .expect("Invalid virtual directory path string");
        let repo =
            RegistryRepo::try_new(registry_id, &path).expect("Failed to construct repository");
        let loaded = SchemaResolver::load_semconv_repository(repo, false)
            .into_result_with_non_fatal()
            .expect("Failed to load registry")
            .0;
        let (engine, template_registry, observed_output, expected_output) =
            prepare_test_with_registry("comment_format", Params::default(), loaded);

        engine
            .generate(
                &template_registry,
                observed_output.as_path(),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir(expected_output, observed_output).unwrap());
    }

    #[test]
    fn test_wrong_config() {
        let loader = FileSystemFileLoader::try_new("templates".into(), "wrong_config")
            .expect("Failed to create file system loader");
        let config = WeaverConfig::try_from_path("templates/wrong_config").unwrap();
        let result = TemplateEngine::try_new(config, loader, Params::default());
        assert!(result.is_err());
        let error = result.err().unwrap();

        let msg = format!("{error}");
        assert!(
            msg.contains("Template pattern 'does-not-exist.j2' did not match any files"),
            "Unexpected error message - {msg}"
        );
    }

    #[test]
    fn test_run_filter_raw() {
        let expected = serde_json::json!({ "one": 1 });
        let input = serde_json::json!({
            "test": expected.clone()
        });
        let result = run_filter_raw(&input, ".test").expect("failed to run raw filter `.test`");
        assert_eq!(result, expected);
    }
}
