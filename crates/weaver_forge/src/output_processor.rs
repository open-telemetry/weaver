// SPDX-License-Identifier: Apache-2.0

//! General-purpose output processor supporting builtin formats and templates.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use include_dir::Dir;
use serde::Serialize;

use crate::config::{Params, WeaverConfig};
use crate::error::Error;
use crate::file_loader::EmbeddedFileLoader;
use crate::{OutputDirective, TemplateEngine};

/// Output processor - handles output generation with builtin formats or templates
#[allow(clippy::large_enum_variant)]
pub enum OutputProcessor {
    /// JSON format - pretty-printed
    Json {
        /// Filename prefix (e.g., "live_check" -> "live_check.json")
        prefix: String,
        /// Output path
        path: PathBuf,
        /// Output directive (Stdout or File)
        directive: OutputDirective,
        /// Open file handle (for file output)
        file: Option<File>,
    },
    /// YAML format
    Yaml {
        /// Filename prefix (e.g., "live_check" -> "live_check.yaml")
        prefix: String,
        /// Output path
        path: PathBuf,
        /// Output directive (Stdout or File)
        directive: OutputDirective,
        /// Open file handle (for file output)
        file: Option<File>,
    },
    /// JSONL format - compact, one object per line
    Jsonl {
        /// Filename prefix (e.g., "live_check" -> "live_check.jsonl")
        prefix: String,
        /// Output path
        path: PathBuf,
        /// Output directive (Stdout or File)
        directive: OutputDirective,
        /// Open file handle (for file output)
        file: Option<File>,
    },
    /// Template-based format
    Template {
        /// Template engine
        engine: TemplateEngine,
        /// Output path
        path: PathBuf,
        /// Output directive (Stdout or File)
        directive: OutputDirective,
    },
    /// No output
    Mute,
}

impl OutputProcessor {
    /// Create an OutputProcessor from format string and configuration
    ///
    /// * `format` - Format name: "json", "yaml", "jsonl", "mute", or a template name
    /// * `prefix` - Base filename prefix (e.g., "live_check" -> "live_check.json")
    /// * `embedded_templates` - Embedded template directory (required only for template formats)
    /// * `templates_path` - Path to override templates (required only for template formats)
    /// * `output` - Output path (None for stdout, Some("none") for mute)
    pub fn new(
        format: &str,
        prefix: &str,
        embedded_templates: Option<&'static Dir<'static>>,
        templates_path: Option<PathBuf>,
        output: Option<&PathBuf>,
    ) -> Result<Self, Error> {
        // Check for mute output
        if output.is_some_and(|p| p.to_str() == Some("none")) {
            return Ok(OutputProcessor::Mute);
        }

        // Determine output path and directive
        let (path, directive) = match output {
            Some(p) => (p.clone(), OutputDirective::File),
            None => (PathBuf::from("output"), OutputDirective::Stdout),
        };

        let prefix = prefix.to_owned();

        match format.to_lowercase().as_str() {
            "mute" => Ok(OutputProcessor::Mute),
            "json" => Ok(OutputProcessor::Json {
                prefix,
                path,
                directive,
                file: None,
            }),
            "yaml" => Ok(OutputProcessor::Yaml {
                prefix,
                path,
                directive,
                file: None,
            }),
            "jsonl" => Ok(OutputProcessor::Jsonl {
                prefix,
                path,
                directive,
                file: None,
            }),
            template_name => {
                // Templates require embedded_templates and templates_path
                let embedded = embedded_templates.ok_or_else(|| Error::InvalidTemplateDir {
                    template_dir: PathBuf::from(template_name),
                    error: "Template format requires embedded_templates parameter".to_owned(),
                })?;
                let templates = templates_path.unwrap_or_default();

                let loader = EmbeddedFileLoader::try_new(embedded, templates, template_name)?;
                let config = WeaverConfig::try_from_loader(&loader)?;
                let engine = TemplateEngine::try_new(config, loader, Params::default())?;
                Ok(OutputProcessor::Template {
                    engine,
                    path,
                    directive,
                })
            }
        }
    }

    /// Open/create the output file if not already open.
    /// For stdout, this is a no-op. Idempotent - safe to call multiple times.
    fn open(&mut self) -> Result<(), Error> {
        match self {
            OutputProcessor::Json {
                prefix,
                path,
                directive,
                file,
            } => {
                if *directive == OutputDirective::File && file.is_none() {
                    *file = Some(Self::create_file(path, &format!("{prefix}.json"))?);
                }
                Ok(())
            }
            OutputProcessor::Yaml {
                prefix,
                path,
                directive,
                file,
            } => {
                if *directive == OutputDirective::File && file.is_none() {
                    *file = Some(Self::create_file(path, &format!("{prefix}.yaml"))?);
                }
                Ok(())
            }
            OutputProcessor::Jsonl {
                prefix,
                path,
                directive,
                file,
            } => {
                if *directive == OutputDirective::File && file.is_none() {
                    *file = Some(Self::create_file(path, &format!("{prefix}.jsonl"))?);
                }
                Ok(())
            }
            OutputProcessor::Template { .. } | OutputProcessor::Mute => Ok(()),
        }
    }

    /// Create/truncate a file and return the handle
    fn create_file(path: &std::path::Path, filename: &str) -> Result<File, Error> {
        let file_path = path.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::WriteGeneratedCodeFailed {
                template: file_path.clone(),
                error: format!("Failed to create directory: {e}"),
            })?;
        }
        File::create(&file_path).map_err(|e| Error::WriteGeneratedCodeFailed {
            template: file_path,
            error: e.to_string(),
        })
    }

    /// Generate output for serializable data.
    /// For file output, lazily opens the file on first call.
    #[allow(clippy::print_stdout)]
    pub fn generate<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        self.open()?;
        match self {
            OutputProcessor::Json {
                directive, file, ..
            } => {
                let content =
                    serde_json::to_string_pretty(data).map_err(|e| Error::SerializationError {
                        error: e.to_string(),
                    })?;
                Self::write_content(&content, directive, file)
            }
            OutputProcessor::Yaml {
                directive, file, ..
            } => {
                let content =
                    serde_yaml::to_string(data).map_err(|e| Error::SerializationError {
                        error: e.to_string(),
                    })?;
                Self::write_content(&content, directive, file)
            }
            OutputProcessor::Jsonl {
                directive, file, ..
            } => {
                let content =
                    serde_json::to_string(data).map_err(|e| Error::SerializationError {
                        error: e.to_string(),
                    })?;
                Self::write_line(&content, directive, file)
            }
            OutputProcessor::Template {
                engine,
                path,
                directive,
            } => engine.generate(data, path, directive),
            OutputProcessor::Mute => Ok(()),
        }
    }

    /// Write content to stdout or file
    #[allow(clippy::print_stdout)]
    fn write_content(
        content: &str,
        directive: &OutputDirective,
        file: &mut Option<File>,
    ) -> Result<(), Error> {
        match directive {
            OutputDirective::Stdout => {
                println!("{content}");
                Ok(())
            }
            OutputDirective::File => {
                let f = file.as_mut().ok_or_else(|| Error::SerializationError {
                    error: "File not opened. Call open() before generate().".to_owned(),
                })?;
                write!(f, "{content}").map_err(|e| Error::SerializationError {
                    error: e.to_string(),
                })
            }
            OutputDirective::Stderr => {
                unreachable!("OutputProcessor does not support Stderr directive")
            }
        }
    }

    /// Write a line to stdout or file (for JSONL)
    #[allow(clippy::print_stdout)]
    fn write_line(
        content: &str,
        directive: &OutputDirective,
        file: &mut Option<File>,
    ) -> Result<(), Error> {
        match directive {
            OutputDirective::Stdout => {
                println!("{content}");
                Ok(())
            }
            OutputDirective::File => {
                let f = file.as_mut().ok_or_else(|| Error::SerializationError {
                    error: "File not opened. Call open() before generate().".to_owned(),
                })?;
                writeln!(f, "{content}").map_err(|e| Error::SerializationError {
                    error: e.to_string(),
                })
            }
            OutputDirective::Stderr => {
                unreachable!("OutputProcessor does not support Stderr directive")
            }
        }
    }

    /// Returns true if file output is being used
    #[must_use]
    pub fn is_file_output(&self) -> bool {
        match self {
            OutputProcessor::Json { directive, .. }
            | OutputProcessor::Yaml { directive, .. }
            | OutputProcessor::Jsonl { directive, .. }
            | OutputProcessor::Template { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Mute => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::fs;
    use tempfile::TempDir;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    fn test_data() -> TestData {
        TestData {
            name: "test".to_owned(),
            value: 42,
        }
    }

    #[test]
    fn test_mute_format() {
        let output = OutputProcessor::new("mute", "test", None, None, None);
        assert!(matches!(output, Ok(OutputProcessor::Mute)));
    }

    #[test]
    fn test_mute_via_output_none() {
        let none_path = PathBuf::from("none");
        let output = OutputProcessor::new("json", "test", None, None, Some(&none_path));
        assert!(matches!(output, Ok(OutputProcessor::Mute)));
    }

    #[test]
    fn test_json_format_stdout() {
        let output = OutputProcessor::new("json", "test", None, None, None).unwrap();
        assert!(matches!(
            output,
            OutputProcessor::Json {
                directive: OutputDirective::Stdout,
                ..
            }
        ));
        assert!(!output.is_file_output());
    }

    #[test]
    fn test_yaml_format_stdout() {
        let output = OutputProcessor::new("yaml", "test", None, None, None).unwrap();
        assert!(matches!(
            output,
            OutputProcessor::Yaml {
                directive: OutputDirective::Stdout,
                ..
            }
        ));
    }

    #[test]
    fn test_jsonl_format_stdout() {
        let output = OutputProcessor::new("jsonl", "test", None, None, None).unwrap();
        assert!(matches!(
            output,
            OutputProcessor::Jsonl {
                directive: OutputDirective::Stdout,
                ..
            }
        ));
    }

    #[test]
    fn test_json_format_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();
        let output = OutputProcessor::new("json", "test", None, None, Some(&output_path)).unwrap();
        assert!(matches!(
            output,
            OutputProcessor::Json {
                directive: OutputDirective::File,
                ..
            }
        ));
        assert!(output.is_file_output());
    }

    #[test]
    fn test_generate_json_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();
        let mut output =
            OutputProcessor::new("json", "myprefix", None, None, Some(&output_path)).unwrap();

        output.generate(&test_data()).unwrap();

        let file_path = output_path.join("myprefix.json");
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"name\": \"test\""));
        assert!(content.contains("\"value\": 42"));
    }

    #[test]
    fn test_generate_yaml_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();
        let mut output =
            OutputProcessor::new("yaml", "myprefix", None, None, Some(&output_path)).unwrap();

        output.generate(&test_data()).unwrap();

        let file_path = output_path.join("myprefix.yaml");
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("name: test"));
        assert!(content.contains("value: 42"));
    }

    #[test]
    fn test_generate_jsonl_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();
        let mut output =
            OutputProcessor::new("jsonl", "myprefix", None, None, Some(&output_path)).unwrap();

        output.generate(&test_data()).unwrap();

        let file_path = output_path.join("myprefix.jsonl");
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        // JSONL should be compact, one line
        assert_eq!(content.trim().lines().count(), 1);
    }

    #[test]
    fn test_jsonl_multiple_lines() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();
        let mut output =
            OutputProcessor::new("jsonl", "myprefix", None, None, Some(&output_path)).unwrap();

        output.generate(&test_data()).unwrap();
        output
            .generate(&TestData {
                name: "second".to_owned(),
                value: 99,
            })
            .unwrap();

        let file_path = output_path.join("myprefix.jsonl");
        let content = fs::read_to_string(&file_path).unwrap();
        // Should have 2 lines
        assert_eq!(content.trim().lines().count(), 2);
        assert!(content.contains("\"name\":\"test\""));
        assert!(content.contains("\"name\":\"second\""));
    }

    #[test]
    fn test_mute_generate_does_nothing() {
        let mut output = OutputProcessor::Mute;
        // Should succeed without doing anything
        assert!(output.generate(&test_data()).is_ok());
        assert!(!output.is_file_output());
    }

    #[test]
    fn test_is_file_output() {
        // Mute is not file output
        assert!(!OutputProcessor::Mute.is_file_output());

        // Stdout variants are not file output
        let json_stdout = OutputProcessor::Json {
            prefix: "test".to_owned(),
            path: PathBuf::new(),
            directive: OutputDirective::Stdout,
            file: None,
        };
        assert!(!json_stdout.is_file_output());

        // File variants are file output
        let json_file = OutputProcessor::Json {
            prefix: "test".to_owned(),
            path: PathBuf::new(),
            directive: OutputDirective::File,
            file: None,
        };
        assert!(json_file.is_file_output());
    }

    #[test]
    fn test_format_case_insensitive() {
        // JSON in various cases
        assert!(matches!(
            OutputProcessor::new("JSON", "test", None, None, None),
            Ok(OutputProcessor::Json { .. })
        ));
        assert!(matches!(
            OutputProcessor::new("Json", "test", None, None, None),
            Ok(OutputProcessor::Json { .. })
        ));

        // MUTE in various cases
        assert!(matches!(
            OutputProcessor::new("MUTE", "test", None, None, None),
            Ok(OutputProcessor::Mute)
        ));
    }

    #[test]
    fn test_template_format_requires_embedded_templates() {
        // Unknown format treated as template name - should fail without embedded_templates
        let result = OutputProcessor::new("ansi", "test", None, None, None);
        assert!(result.is_err());
    }
}
