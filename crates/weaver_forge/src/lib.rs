// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use minijinja::syntax::SyntaxConfig;
use minijinja::value::{from_args, Object};
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
use weaver_common::Logger;

use crate::config::{ApplicationMode, TargetConfig};
use crate::debug::error_summary;
use crate::error::Error::InvalidConfigFile;
use crate::extensions::{ansi, case, code, otel, util};
use crate::file_loader::FileLoader;
use crate::registry::{TemplateGroup, TemplateRegistry};

mod config;
pub mod debug;
pub mod error;
pub mod extensions;
pub mod file_loader;
mod filter;
pub mod registry;

/// Name of the Weaver configuration file.
pub const WEAVER_YAML: &str = "weaver.yaml";

/// Enumeration defining where the output of program execution should be directed.
#[derive(Debug, Clone)]
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

impl Display for TemplateObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "template file name: {}",
            self.file_name.lock().expect("Lock poisoned")
        ))
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

/// Template engine for generating artifacts from a semantic convention
/// registry and telemetry schema.
pub struct TemplateEngine {
    /// File loader used by the engine.
    file_loader: Arc<dyn FileLoader + Send + Sync + 'static>,

    /// Target configuration
    target_config: TargetConfig,
}

/// Global context for the template engine.
#[derive(Serialize, Debug)]
pub struct Context<'a> {
    /// The semantic convention registry.
    pub registry: &'a TemplateRegistry,
    /// The group to generate doc or code for.
    pub group: Option<&'a TemplateGroup>,
    /// The groups to generate doc or code for.
    pub groups: Option<Vec<&'a TemplateGroup>>,
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
    /// Create a new template engine for the given target or return an error if
    /// the target does not exist or is not a directory.
    pub fn try_new(loader: impl FileLoader + Send + Sync + 'static) -> Result<Self, Error> {
        let target_config = TargetConfig::try_new(&loader)?;
        Ok(Self {
            file_loader: Arc::new(loader),
            target_config,
        })
    }

    /// Generate a template snippet from serializable context and a snippet identifier.
    ///
    /// # Arguments
    ///
    /// * `log` - The logger to use for logging
    /// * `context` - The context to use when generating snippets.
    /// * `snippet_id` - The template to use when rendering the snippet.
    pub fn generate_snippet<T: Serialize>(
        &self,
        context: &T,
        snippet_id: String,
    ) -> Result<String, Error> {
        // TODO - find the snippet by id.

        // Create a read-only context for the filter evaluations
        let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })?;

        let engine = self.template_engine()?;
        let template = engine
            .get_template(&snippet_id)
            .map_err(error::jinja_err_convert)?;
        let result = template.render(context).map_err(error::jinja_err_convert)?;
        Ok(result)
    }

    /// Generate artifacts from a serializable context and a template directory,
    /// in parallel.
    ///
    /// # Arguments
    ///
    /// * `log` - The logger to use for logging.
    /// * `context` - The context to use for generating the artifacts.
    /// * `output_dir` - The directory where the generated artifacts will be saved.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the artifacts were generated successfully.
    /// * `Err(error)` if an error occurred during the generation of the artifacts.
    pub fn generate<T: Serialize>(
        &self,
        log: impl Logger + Clone + Sync,
        context: &T,
        output_dir: &Path,
        output_directive: &OutputDirective,
    ) -> Result<(), Error> {
        let files = self.file_loader.all_files();
        let tmpl_matcher = self.target_config.template_matcher()?;

        // Create a read-only context for the filter evaluations
        let context = serde_json::to_value(context).map_err(|e| ContextSerializationFailed {
            error: e.to_string(),
        })?;

        // Process all files in parallel
        // - Filter the files that match the template pattern
        // - Apply the filter to the context
        // - Evaluate the template with the filtered context based on the
        // application mode.
        //   - If the application mode is single, the filtered context is
        // evaluated as a single object.
        //   - If the application mode is each, the filtered context is
        // evaluated as an array of objects and each object is evaluated
        // independently and in parallel with the same template.
        let errs = files
            .into_par_iter()
            .filter_map(|relative_path| {
                for template in tmpl_matcher.matches(relative_path.clone()) {
                    let filtered_result = match template.filter.apply(context.clone()) {
                        Ok(result) => result,
                        Err(e) => return Some(e),
                    };

                    match template.application_mode {
                        // The filtered result is evaluated as a single object
                        ApplicationMode::Single => {
                            if let Err(e) = self.evaluate_template(
                                log.clone(),
                                NewContext {
                                    ctx: &filtered_result,
                                }
                                .try_into()
                                .ok()?,
                                relative_path.as_path(),
                                output_directive,
                                output_dir,
                            ) {
                                return Some(e);
                            }
                        }
                        // The filtered result is evaluated as an array of objects
                        // and each object is evaluated independently and in parallel
                        // with the same template.
                        ApplicationMode::Each => {
                            if let Some(values) = filtered_result.as_array() {
                                let errs = values
                                    .into_par_iter()
                                    .filter_map(|result| {
                                        if let Err(e) = self.evaluate_template(
                                            log.clone(),
                                            NewContext { ctx: result }.try_into().ok()?,
                                            relative_path.as_path(),
                                            output_directive,
                                            output_dir,
                                        ) {
                                            return Some(e);
                                        }
                                        None
                                    })
                                    .collect::<Vec<Error>>();
                                if !errs.is_empty() {
                                    return Some(Error::compound_error(errs));
                                }
                            } else if let Err(e) = self.evaluate_template(
                                log.clone(),
                                NewContext {
                                    ctx: &filtered_result,
                                }
                                .try_into()
                                .ok()?,
                                relative_path.as_path(),
                                output_directive,
                                output_dir,
                            ) {
                                return Some(e);
                            }
                        }
                    }
                }
                None
            })
            .collect::<Vec<Error>>();

        handle_errors(errs)
    }

    #[allow(clippy::print_stdout)] // This is used for the OutputDirective::Stdout variant
    #[allow(clippy::print_stderr)] // This is used for the OutputDirective::Stderr variant
    fn evaluate_template(
        &self,
        log: impl Logger + Clone + Sync,
        ctx: serde_json::Value,
        template_path: &Path,
        output_directive: &OutputDirective,
        output_dir: &Path,
    ) -> Result<(), Error> {
        // By default, the file name is the template file name without the extension ".j2"
        let file_name = template_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .trim_end_matches(".j2")
            .to_owned();
        let template_object = TemplateObject {
            file_name: Arc::new(Mutex::new(file_name)),
        };
        let mut engine = self.template_engine()?;
        let template_file = template_path.to_str().ok_or(InvalidTemplateFile {
            template: template_path.to_path_buf(),
            error: "".to_owned(),
        })?;

        engine.add_global("template", Value::from_object(template_object.clone()));

        let template = engine.get_template(template_file).map_err(|e| {
            let templates = engine
                .templates()
                .map(|(name, _)| name.to_owned())
                .collect::<Vec<_>>();
            let error = format!("{}. Available templates: {:?}", e, templates);
            InvalidTemplateFile {
                template: template_file.into(),
                error,
            }
        })?;

        let output = template
            .render(ctx.clone())
            .map_err(|e| TemplateEvaluationFailed {
                template: template_path.to_path_buf(),
                error_id: e.to_string(),
                error: error_summary(e),
            })?;
        match output_directive {
            OutputDirective::Stdout => {
                println!("{}", output);
            }
            OutputDirective::Stderr => {
                eprintln!("{}", output);
            }
            OutputDirective::File => {
                let generated_file =
                    Self::save_generated_code(output_dir, template_object.file_name(), output)?;
                log.success(&format!("Generated file {:?}", generated_file));
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
                Cow::Owned(template_syntax.block_start),
                Cow::Owned(template_syntax.block_end),
            )
            .variable_delimiters(
                Cow::Owned(template_syntax.variable_start),
                Cow::Owned(template_syntax.variable_end),
            )
            .comment_delimiters(
                Cow::Owned(template_syntax.comment_start),
                Cow::Owned(template_syntax.comment_end),
            )
            .build()
            .map_err(|e| InvalidConfigFile {
                config_file: PathBuf::from(OsString::from(&self.file_loader.root()))
                    .join(WEAVER_YAML),
                error: e.to_string(),
            })?;

        let file_loader = self.file_loader.clone();
        env.set_loader(move |name| {
            file_loader
                .load_file(name)
                .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))
        });
        env.set_syntax(syntax);

        code::add_filters(&mut env, &self.target_config);
        ansi::add_filters(&mut env);
        case::add_filters(&mut env, &self.target_config);
        otel::add_tests_and_filters(&mut env);
        util::add_filters(&mut env, &self.target_config);

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
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use globset::Glob;

    use weaver_common::TestLogger;
    use weaver_diff::diff_dir;
    use weaver_resolver::SchemaResolver;
    use weaver_semconv::registry::SemConvRegistry;

    use crate::config::{ApplicationMode, CaseConvention, TemplateConfig};
    use crate::debug::print_dedup_errors;
    use crate::extensions::case::case_converter;
    use crate::file_loader::FileSystemFileLoader;
    use crate::filter::Filter;
    use crate::registry::TemplateRegistry;
    use crate::OutputDirective;

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
    fn test() {
        let logger = TestLogger::default();
        let loader = FileSystemFileLoader::try_new("templates".into(), "test")
            .expect("Failed to create file system loader");
        let mut engine =
            super::TemplateEngine::try_new(loader).expect("Failed to create template engine");

        // Add a template configuration for converter.md on top
        // of the default template configuration. This is useful
        // for test coverage purposes.
        engine.target_config.templates.push(TemplateConfig {
            pattern: Glob::new("converter.md").unwrap(),
            filter: Filter::try_new(".").unwrap(),
            application_mode: ApplicationMode::Single,
        });

        let registry_id = "default";
        let mut registry = SemConvRegistry::try_from_path_pattern(registry_id, "data/*.yaml")
            .expect("Failed to load registry");
        let schema = SchemaResolver::resolve_semantic_convention_registry(&mut registry)
            .expect("Failed to resolve registry");

        let template_registry = TemplateRegistry::try_from_resolved_registry(
            schema.registry(registry_id).expect("registry not found"),
            schema.catalog(),
        )
        .unwrap_or_else(|e| {
            panic!(
                "Failed to create the context for the template evaluation: {:?}",
                e
            )
        });

        engine
            .generate(
                logger.clone(),
                &template_registry,
                Path::new("observed_output"),
                &OutputDirective::File,
            )
            .inspect_err(|e| {
                print_dedup_errors(logger.clone(), e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(diff_dir("expected_output", "observed_output").unwrap());
    }
}
