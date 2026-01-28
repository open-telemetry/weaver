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

/// Builtin serialization formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFormat {
    /// JSON - pretty-printed
    Json,
    /// YAML
    Yaml,
    /// JSONL - compact JSON, one object per line
    Jsonl,
}

impl BuiltinFormat {
    /// File extension for this format
    fn extension(&self) -> &'static str {
        match self {
            BuiltinFormat::Json => "json",
            BuiltinFormat::Yaml => "yaml",
            BuiltinFormat::Jsonl => "jsonl",
        }
    }

    /// Serialize data to string
    fn serialize<T: Serialize>(&self, data: &T) -> Result<String, Error> {
        match self {
            BuiltinFormat::Json => {
                serde_json::to_string_pretty(data).map_err(|e| Error::SerializationError {
                    error: e.to_string(),
                })
            }
            BuiltinFormat::Yaml => {
                serde_yaml::to_string(data).map_err(|e| Error::SerializationError {
                    error: e.to_string(),
                })
            }
            BuiltinFormat::Jsonl => {
                serde_json::to_string(data).map_err(|e| Error::SerializationError {
                    error: e.to_string(),
                })
            }
        }
    }

    /// Whether this format appends a newline after each generate call
    fn is_line_oriented(&self) -> bool {
        matches!(self, BuiltinFormat::Jsonl)
    }
}

/// Output processor - handles output generation with builtin formats or templates
#[allow(clippy::large_enum_variant)]
pub enum OutputProcessor {
    /// Builtin format (JSON, YAML, JSONL)
    Builtin {
        /// The serialization format
        format: BuiltinFormat,
        /// Filename prefix (e.g., "live_check" -> "live_check.json")
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

        match format.to_lowercase().as_str() {
            "mute" => Ok(OutputProcessor::Mute),
            "json" => Ok(OutputProcessor::Builtin {
                format: BuiltinFormat::Json,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
            }),
            "yaml" => Ok(OutputProcessor::Builtin {
                format: BuiltinFormat::Yaml,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
            }),
            "jsonl" => Ok(OutputProcessor::Builtin {
                format: BuiltinFormat::Jsonl,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
            }),
            template_name => {
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
    fn open(&mut self) -> Result<(), Error> {
        if let OutputProcessor::Builtin {
            format,
            prefix,
            path,
            directive,
            file,
        } = self
        {
            if *directive == OutputDirective::File && file.is_none() {
                let filename = format!("{}.{}", prefix, format.extension());
                *file = Some(Self::create_file(path, &filename)?);
            }
        }
        Ok(())
    }

    /// Create/truncate a file and return the handle
    fn create_file(path: &std::path::Path, filename: &str) -> Result<File, Error> {
        let file_path = path.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OutputFileError {
                path: file_path.clone(),
                error: format!("Failed to create directory: {e}"),
            })?;
        }
        File::create(&file_path).map_err(|e| Error::OutputFileError {
            path: file_path,
            error: e.to_string(),
        })
    }

    /// Generate output for serializable data.
    #[allow(clippy::print_stdout)]
    pub fn generate<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        self.open()?;
        match self {
            OutputProcessor::Builtin {
                format,
                directive,
                file,
                ..
            } => {
                let content = format.serialize(data)?;
                Self::write(&content, directive, file, format.is_line_oriented())
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
    fn write(
        content: &str,
        directive: &OutputDirective,
        file: &mut Option<File>,
        with_newline: bool,
    ) -> Result<(), Error> {
        match directive {
            OutputDirective::Stdout => {
                println!("{content}");
                Ok(())
            }
            OutputDirective::File => {
                let f = file.as_mut().ok_or_else(|| Error::SerializationError {
                    error: "File not opened".to_owned(),
                })?;
                if with_newline {
                    writeln!(f, "{content}")
                } else {
                    write!(f, "{content}")
                }
                .map_err(|e| Error::SerializationError {
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
            OutputProcessor::Builtin { directive, .. }
            | OutputProcessor::Template { directive, .. } => *directive == OutputDirective::File,
            OutputProcessor::Mute => false,
        }
    }

    /// Returns true if this format is line-oriented (supports multiple generate calls,
    /// one item per line). Currently only JSONL has this behavior.
    #[must_use]
    pub fn is_line_oriented(&self) -> bool {
        matches!(
            self,
            OutputProcessor::Builtin {
                format: BuiltinFormat::Jsonl,
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use include_dir::{include_dir, Dir};
    use serde::{Deserialize, Serialize};
    use std::fs;
    use tempfile::TempDir;

    static EMBEDDED_TEMPLATES: Dir<'_> = include_dir!("crates/weaver_forge/templates");

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
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
    fn test_all_builtin_formats_stdout() {
        let formats = ["json", "yaml", "jsonl"];
        for name in formats {
            let mut output = OutputProcessor::new(name, "test", None, None, None)
                .unwrap_or_else(|e| panic!("Failed to create {name}: {e}"));
            assert!(!output.is_file_output(), "{name}");
            output
                .generate(&test_data())
                .unwrap_or_else(|e| panic!("Failed to generate {name}: {e}"));
        }
    }

    #[test]
    fn test_json_format_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new("json", "test", None, None, Some(&path)).unwrap();
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let file_path = path.join("test.json");
        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: TestData = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_yaml_format_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new("yaml", "test", None, None, Some(&path)).unwrap();
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let file_path = path.join("test.yaml");
        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: TestData = serde_yaml::from_str(&content).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_jsonl_format_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new("jsonl", "test", None, None, Some(&path)).unwrap();
        assert!(output.is_line_oriented());
        assert!(output.is_file_output());

        let second = TestData {
            name: "second".to_owned(),
            value: 99,
        };
        output.generate(&test_data()).unwrap();
        output.generate(&second).unwrap();

        let file_path = path.join("test.jsonl");
        let content = fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let parsed_first: TestData = serde_json::from_str(lines[0]).unwrap();
        let parsed_second: TestData = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(parsed_first, test_data());
        assert_eq!(parsed_second, second);
    }

    #[test]
    fn test_mute_generate_does_nothing() {
        let mut output = OutputProcessor::Mute;
        assert!(output.generate(&test_data()).is_ok());
        assert!(!output.is_file_output());
    }

    #[test]
    fn test_is_file_output() {
        assert!(!OutputProcessor::Mute.is_file_output());

        let builtin_stdout = OutputProcessor::Builtin {
            format: BuiltinFormat::Json,
            prefix: "test".to_owned(),
            path: PathBuf::new(),
            directive: OutputDirective::Stdout,
            file: None,
        };
        assert!(!builtin_stdout.is_file_output());

        let builtin_file = OutputProcessor::Builtin {
            format: BuiltinFormat::Json,
            prefix: "test".to_owned(),
            path: PathBuf::new(),
            directive: OutputDirective::File,
            file: None,
        };
        assert!(builtin_file.is_file_output());
    }

    #[test]
    fn test_format_case_insensitive() {
        assert!(matches!(
            OutputProcessor::new("JSON", "test", None, None, None),
            Ok(OutputProcessor::Builtin {
                format: BuiltinFormat::Json,
                ..
            })
        ));
        assert!(matches!(
            OutputProcessor::new("Json", "test", None, None, None),
            Ok(OutputProcessor::Builtin {
                format: BuiltinFormat::Json,
                ..
            })
        ));
        assert!(matches!(
            OutputProcessor::new("MUTE", "test", None, None, None),
            Ok(OutputProcessor::Mute)
        ));
    }

    #[test]
    fn test_template_format_requires_embedded_templates() {
        let result = OutputProcessor::new("ansi", "test", None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_template_format_stdout() {
        let mut output =
            OutputProcessor::new("simple", "test", Some(&EMBEDDED_TEMPLATES), None, None).unwrap();
        assert!(!output.is_line_oriented());
        assert!(!output.is_file_output());
        output.generate(&test_data()).unwrap();
    }

    #[test]
    fn test_template_format_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            Some(&path),
        )
        .unwrap();
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let file_path = path.join("output.txt");
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("test"), "should contain name");
        assert!(content.contains("42"), "should contain value");
    }

    #[test]
    fn test_is_line_oriented() {
        let json = OutputProcessor::new("json", "test", None, None, None).unwrap();
        assert!(!json.is_line_oriented());

        let yaml = OutputProcessor::new("yaml", "test", None, None, None).unwrap();
        assert!(!yaml.is_line_oriented());

        let jsonl = OutputProcessor::new("jsonl", "test", None, None, None).unwrap();
        assert!(jsonl.is_line_oriented());

        assert!(!OutputProcessor::Mute.is_line_oriented());
    }
}
