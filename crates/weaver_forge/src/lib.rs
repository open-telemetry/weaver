// SPDX-License-Identifier: Apache-2.0

//! This crate extends the MiniJinja template engine to add helper functions
//! and filters for working with semantic convention registries and telemetry
//! schemas.

use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use minijinja::value::{from_args, Object};
use minijinja::{path_loader, Environment, State, Value};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use error::Error;
use error::Error::{
    ContextSerializationFailed, InvalidTemplateDir, InvalidTemplateFile, TargetNotSupported,
    TemplateEvaluationFailed, WriteGeneratedCodeFailed,
};
use weaver_logger::Logger;

use crate::config::{ApplicationMode, TargetConfig};
use crate::debug::error_summary;
use crate::error::Error::InvalidConfigFile;
use crate::extensions::case_converter::case_converter;
use crate::registry::{TemplateGroup, TemplateRegistry};

mod config;
pub mod debug;
pub mod error;
mod extensions;
mod filter;
pub mod registry;

/// Name of the Weaver configuration file.
pub const WEAVER_YAML: &str = "weaver.yaml";

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
        &self,
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
                minijinja::ErrorKind::UnknownMethod,
                format!("template has no method named {name}"),
            ))
        }
    }
}

/// Template engine for generating artifacts from a semantic convention
/// registry and telemetry schema.
pub struct TemplateEngine {
    /// Template path
    path: PathBuf,

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
    pub fn try_new(target: &str, config: GeneratorConfig) -> Result<Self, Error> {
        // Check if the target is supported.
        // A target is supported if a template directory exists for it.
        let target_path = config.root_dir.join(target);

        if !target_path.exists() {
            return Err(TargetNotSupported {
                root_path: config.root_dir.to_string_lossy().to_string(),
                target: target.to_owned(),
            });
        }

        Ok(Self {
            path: target_path.clone(),
            target_config: TargetConfig::try_new(&target_path)?,
        })
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
    ) -> Result<(), Error> {
        // List all files in the target directory and its subdirectories
        let files: Vec<DirEntry> = WalkDir::new(self.path.clone())
            .into_iter()
            .filter_map(|e| {
                // Skip directories that the owner of the running process does not
                // have permission to access
                e.ok()
            })
            .filter(|dir_entry| dir_entry.path().is_file())
            .collect();

        let config = TargetConfig::try_new(&self.path)?;
        let tmpl_matcher = config.template_matcher()?;

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
            .filter_map(|file| {
                let relative_path = match file.path().strip_prefix(&self.path) {
                    Ok(relative_path) => relative_path,
                    Err(e) => {
                        return Some(InvalidTemplateDir {
                            template_dir: self.path.clone(),
                            error: e.to_string(),
                        });
                    }
                };

                for template in tmpl_matcher.matches(relative_path) {
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
                                relative_path,
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
                                            relative_path,
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
                                relative_path,
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

        error::handle_errors(errs)
    }

    fn evaluate_template(
        &self,
        log: impl Logger + Clone + Sync,
        ctx: serde_json::Value,
        template_path: &Path,
        output_dir: &Path,
    ) -> Result<(), Error> {
        let template_object = TemplateObject {
            file_name: Arc::new(Mutex::new(
                template_path.to_str().unwrap_or_default().to_owned(),
            )),
        };
        let mut engine = self.template_engine()?;
        let template_file = template_path.to_str().ok_or(InvalidTemplateFile {
            template: template_path.to_path_buf(),
            error: "".to_owned(),
        })?;

        engine.add_global("template", Value::from_object(template_object.clone()));

        log.loading(&format!("Generating file {}", template_file));
        let template = engine
            .get_template(template_file)
            .map_err(|e| InvalidTemplateFile {
                template: template_path.to_path_buf(),
                error: e.to_string(),
            })?;

        let output = template
            .render(ctx.clone())
            .map_err(|e| TemplateEvaluationFailed {
                template: template_path.to_path_buf(),
                error_id: e.to_string(),
                error: error_summary(e),
            })?;
        let generated_file =
            Self::save_generated_code(output_dir, template_object.file_name(), output)?;
        log.success(&format!("Generated file {:?}", generated_file));
        Ok(())
    }

    /// Create a new template engine based on the target configuration.
    fn template_engine(&self) -> Result<Environment<'_>, Error> {
        let mut env = Environment::new();
        env.set_loader(path_loader(&self.path));
        env.set_syntax(self.target_config.template_syntax.clone().into())
            .map_err(|e| InvalidConfigFile {
                config_file: self.path.join(WEAVER_YAML),
                error: e.to_string(),
            })?;

        // Register case conversion filters based on the target configuration
        env.add_filter(
            "file_name",
            case_converter(self.target_config.file_name.clone()),
        );
        env.add_filter(
            "function_name",
            case_converter(self.target_config.function_name.clone()),
        );
        env.add_filter(
            "arg_name",
            case_converter(self.target_config.arg_name.clone()),
        );
        env.add_filter(
            "struct_name",
            case_converter(self.target_config.struct_name.clone()),
        );
        env.add_filter(
            "field_name",
            case_converter(self.target_config.field_name.clone()),
        );
        env.add_filter("flatten", flatten);
        env.add_filter("split_id", split_id);

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

        // Register custom testers
        // tera.register_tester("required", testers::is_required);
        // tera.register_tester("not_required", testers::is_not_required);
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

// Helper filter to work around lack of `list.append()` support in minijinja.
// Will take a list of lists and return a new list containing only elements of sublists.
fn flatten(value: Value) -> Result<Value, minijinja::Error> {
    let mut result = Vec::new();
    for sublist in value.try_iter()? {
        for item in sublist.try_iter()? {
            result.push(item);
        }
    }
    Ok(Value::from(result))
}

// Helper function to take an "id" and split it by '.' into namespaces.
fn split_id(value: Value) -> Result<Vec<Value>, minijinja::Error> {
    match value.as_str() {
        Some(id) => {
            let values: Vec<Value> = id
                .split('.')
                .map(|s| Value::from_safe_string(s.to_owned()))
                .collect();
            Ok(Value::from(values))
        }
        None => Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Expected string, found: {value}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    use walkdir::WalkDir;

    use crate::debug::print_dedup_errors;
    use weaver_diff::diff_output;
    use weaver_logger::TestLogger;
    use weaver_resolver::SchemaResolver;
    use weaver_semconv::SemConvRegistry;

    use crate::registry::TemplateRegistry;

    #[test]
    fn test() {
        let logger = TestLogger::default();
        let engine = super::TemplateEngine::try_new("test", super::GeneratorConfig::default())
            .expect("Failed to create template engine");

        let registry_id = "default";
        let mut registry = SemConvRegistry::try_from_path(registry_id, "data/*.yaml")
            .expect("Failed to load registry");
        let schema =
            SchemaResolver::resolve_semantic_convention_registry(&mut registry, logger.clone())
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
            )
            .inspect_err(|e| {
                print_dedup_errors(logger.clone(), e.clone());
            })
            .expect("Failed to generate registry assets");

        assert!(cmp_dir("expected_output", "observed_output").unwrap());
    }

    #[allow(clippy::print_stderr)]
    fn cmp_dir<P: AsRef<Path>>(expected_dir: P, observed_dir: P) -> std::io::Result<bool> {
        let mut expected_files = HashSet::new();
        let mut observed_files = HashSet::new();

        // Walk through the first directory and add files to files1 set
        for entry in WalkDir::new(&expected_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(&expected_dir).unwrap();
                _ = expected_files.insert(relative_path.to_path_buf());
            }
        }

        // Walk through the second directory and add files to files2 set
        for entry in WalkDir::new(&observed_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(&observed_dir).unwrap();
                _ = observed_files.insert(relative_path.to_path_buf());
            }
        }

        // Assume directories are identical until proven otherwise
        let mut are_identical = true;

        // Compare files in both sets
        for file in expected_files.intersection(&observed_files) {
            let file1_content =
                fs::read_to_string(expected_dir.as_ref().join(file))?.replace("\r\n", "\n");
            let file2_content =
                fs::read_to_string(observed_dir.as_ref().join(file))?.replace("\r\n", "\n");

            if file1_content != file2_content {
                are_identical = false;
                eprintln!(
                    "Files {:?} and {:?} are different",
                    expected_dir.as_ref().join(file),
                    observed_dir.as_ref().join(file)
                );

                eprintln!(
                    "Found differences:\n{}",
                    diff_output(&file1_content, &file2_content)
                );
                break;
            }
        }
        // If any file is unique to one directory, they are not identical
        let not_in_observed = expected_files
            .difference(&observed_files)
            .collect::<Vec<_>>();
        if !not_in_observed.is_empty() {
            are_identical = false;
            eprintln!("Observed output is missing files: {:?}", not_in_observed);
        }
        let not_in_expected = observed_files
            .difference(&expected_files)
            .collect::<Vec<_>>();
        if !not_in_expected.is_empty() {
            are_identical = false;
            eprintln!(
                "Observed output has unexpected files: {:?}",
                not_in_expected
            );
        }

        Ok(are_identical)
    }
}
