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

/// Specifies where output should be written.
#[derive(Debug, Clone)]
pub enum OutputTarget {
    /// Write to stdout.
    Stdout,
    /// Write directly to this file path.
    File(PathBuf),
    /// Create `{prefix}.{ext}` inside this directory.
    Directory(PathBuf),
    /// No output.
    Mute,
}

impl OutputTarget {
    /// Convert from the common CLI pattern: `None` = stdout, `Some("none")` = mute,
    /// `Some(path)` = directory.
    #[must_use]
    pub fn from_optional_dir(path: Option<&PathBuf>) -> Self {
        match path {
            None => OutputTarget::Stdout,
            Some(p) if p.to_str() == Some("none") => OutputTarget::Mute,
            Some(p) => OutputTarget::Directory(p.clone()),
        }
    }
}

/// Builtin serialization formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinFormat {
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

    /// Whether this format is intended to make output line by line through multiple generate calls e.g., JSONL.
    fn is_line_oriented(&self) -> bool {
        matches!(self, BuiltinFormat::Jsonl)
    }
}

/// Template output configuration
struct TemplateOutput {
    engine: TemplateEngine,
    path: PathBuf,
    directive: OutputDirective,
}

/// Internal enum for output processor variants
enum OutputKind {
    /// Builtin format e.g., JSON, YAML, JSONL
    Builtin {
        format: BuiltinFormat,
        prefix: String,
        path: PathBuf,
        directive: OutputDirective,
        file: Option<File>,
        /// When true, `path` is the exact file path (not a directory).
        direct_file: bool,
    },
    /// Template-based format
    Template(Box<TemplateOutput>),
    /// No output
    Mute,
}

/// Output processor — handles output generation with builtin formats or templates.
// This is a public struct wrapping a private enum, providing encapsulation
// while keeping the generic `generate<T: Serialize>` method.
pub struct OutputProcessor {
    kind: OutputKind,
}

impl OutputProcessor {
    /// Create an OutputProcessor from format string and configuration.
    ///
    /// * `format` - Format name: "json", "yaml", "jsonl", "mute", or a template name
    /// * `prefix` - Base filename prefix (e.g., "live_check" -> "live_check.json")
    /// * `embedded_templates` - Embedded template directory (required only for template formats)
    /// * `templates_path` - Path to override templates (required only for template formats)
    /// * `output` - Where to write output
    pub fn new(
        format: &str,
        prefix: &str,
        embedded_templates: Option<&'static Dir<'static>>,
        templates_path: Option<PathBuf>,
        output: OutputTarget,
    ) -> Result<Self, Error> {
        // Check for mute output target
        if matches!(output, OutputTarget::Mute) {
            return Ok(Self {
                kind: OutputKind::Mute,
            });
        }

        // Determine output path and directive
        let (path, directive, direct_file) = match &output {
            OutputTarget::File(p) => (p.clone(), OutputDirective::File, true),
            OutputTarget::Directory(p) => (p.clone(), OutputDirective::File, false),
            OutputTarget::Stdout => (PathBuf::from("output"), OutputDirective::Stdout, false),
            OutputTarget::Mute => unreachable!(),
        };

        let kind = match format.to_lowercase().as_str() {
            "mute" => OutputKind::Mute,
            "json" => OutputKind::Builtin {
                format: BuiltinFormat::Json,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
                direct_file,
            },
            "yaml" => OutputKind::Builtin {
                format: BuiltinFormat::Yaml,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
                direct_file,
            },
            "jsonl" => OutputKind::Builtin {
                format: BuiltinFormat::Jsonl,
                prefix: prefix.to_owned(),
                path,
                directive,
                file: None,
                direct_file,
            },
            template_name => {
                let embedded = embedded_templates.ok_or_else(|| Error::InvalidTemplateDir {
                    template_dir: PathBuf::from(template_name),
                    error: "Template format requires embedded_templates parameter".to_owned(),
                })?;
                let templates = templates_path.unwrap_or_default();

                let loader = EmbeddedFileLoader::try_new(embedded, templates, template_name)?;
                let config = WeaverConfig::try_from_loader(&loader)?;
                let engine = TemplateEngine::try_new(config, loader, Params::default())?;
                OutputKind::Template(Box::new(TemplateOutput {
                    engine,
                    path,
                    directive,
                }))
            }
        };

        Ok(Self { kind })
    }

    /// Generate output for serializable data.
    pub fn generate<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        match &mut self.kind {
            OutputKind::Builtin {
                format,
                prefix,
                path,
                directive,
                file,
                direct_file,
            } => {
                // Open file if needed
                if *directive == OutputDirective::File && file.is_none() {
                    if *direct_file {
                        *file = Some(create_direct_file(path)?);
                    } else {
                        let filename = format!("{}.{}", prefix, format.extension());
                        *file = Some(create_file(path, &filename)?);
                    }
                }
                let content = format.serialize(data)?;
                write_content(&content, directive, file, format.is_line_oriented())
            }
            OutputKind::Template(t) => t.engine.generate(data, &t.path, &t.directive),
            OutputKind::Mute => Ok(()),
        }
    }

    /// Serialize/render data to a String without writing to stdout/file.
    pub fn generate_to_string<T: Serialize>(&self, data: &T) -> Result<String, Error> {
        match &self.kind {
            OutputKind::Builtin { format, .. } => format.serialize(data),
            OutputKind::Template(t) => t.engine.generate_to_string(data),
            OutputKind::Mute => Ok(String::new()),
        }
    }

    /// Returns the MIME content type for the configured format.
    #[must_use]
    pub fn content_type(&self) -> &'static str {
        match &self.kind {
            OutputKind::Builtin { format, .. } => match format {
                BuiltinFormat::Json => "application/json",
                BuiltinFormat::Yaml => "application/yaml",
                BuiltinFormat::Jsonl => "application/x-ndjson",
            },
            OutputKind::Template(_) => "text/plain",
            OutputKind::Mute => "text/plain",
        }
    }

    /// Returns true if file output is being used.
    #[must_use]
    pub fn is_file_output(&self) -> bool {
        match &self.kind {
            OutputKind::Builtin { directive, .. } => *directive == OutputDirective::File,
            OutputKind::Template(t) => t.directive == OutputDirective::File,
            OutputKind::Mute => false,
        }
    }

    /// Returns true if this format is line-oriented (supports multiple generate calls,
    /// one item per line). Currently only JSONL has this behavior.
    #[must_use]
    pub fn is_line_oriented(&self) -> bool {
        matches!(
            &self.kind,
            OutputKind::Builtin {
                format: BuiltinFormat::Jsonl,
                ..
            }
        )
    }
}

/// Create/truncate a file inside a directory and return the handle.
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

/// Create/truncate a file at the exact path and return the handle.
fn create_direct_file(path: &std::path::Path) -> Result<File, Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::OutputFileError {
            path: path.to_path_buf(),
            error: format!("Failed to create directory: {e}"),
        })?;
    }
    File::create(path).map_err(|e| Error::OutputFileError {
        path: path.to_path_buf(),
        error: e.to_string(),
    })
}

/// Write content to stdout or file.
#[allow(clippy::print_stdout)]
fn write_content(
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
        let output =
            OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!output.is_file_output());
        assert!(!output.is_line_oriented());
    }

    #[test]
    fn test_mute_via_output_target() {
        let output = OutputProcessor::new("json", "test", None, None, OutputTarget::Mute).unwrap();
        assert!(!output.is_file_output());
        assert!(!output.is_line_oriented());
    }

    #[test]
    fn test_mute_via_from_optional_dir() {
        let none_path = PathBuf::from("none");
        let target = OutputTarget::from_optional_dir(Some(&none_path));
        let output = OutputProcessor::new("json", "test", None, None, target).unwrap();
        assert!(!output.is_file_output());
        assert!(!output.is_line_oriented());
    }

    #[test]
    fn test_all_builtin_formats_stdout() {
        let formats = ["json", "yaml", "jsonl"];
        for name in formats {
            let mut output = OutputProcessor::new(name, "test", None, None, OutputTarget::Stdout)
                .unwrap_or_else(|e| panic!("Failed to create {name}: {e}"));
            assert!(!output.is_file_output(), "{name}");
            output
                .generate(&test_data())
                .unwrap_or_else(|e| panic!("Failed to generate {name}: {e}"));
        }
    }

    #[test]
    fn test_json_format_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .unwrap();
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let file_path = path.join("test.json");
        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: TestData = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_json_format_to_direct_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("output.json");
        let mut output = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::File(file_path.clone()),
        )
        .unwrap();
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: TestData = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_yaml_format_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "yaml",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .unwrap();
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output.generate(&test_data()).unwrap();

        let file_path = path.join("test.yaml");
        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: TestData = serde_yaml::from_str(&content).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_jsonl_format_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "jsonl",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .unwrap();
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
        let mut output =
            OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(output.generate(&test_data()).is_ok());
        assert!(!output.is_file_output());
    }

    #[test]
    fn test_is_file_output() {
        // Mute is not file output
        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!mute.is_file_output());

        // Stdout is not file output
        let stdout =
            OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!stdout.is_file_output());

        // Directory output is file output
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let dir = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .unwrap();
        assert!(dir.is_file_output());

        // Direct file output is file output
        let file = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::File(path.join("test.json")),
        )
        .unwrap();
        assert!(file.is_file_output());
    }

    #[test]
    fn test_format_case_insensitive() {
        // JSON (uppercase) should create a valid non-line-oriented processor
        let json_upper =
            OutputProcessor::new("JSON", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!json_upper.is_line_oriented());
        assert!(!json_upper.is_file_output());

        // Json (mixed case) should also work
        let json_mixed =
            OutputProcessor::new("Json", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!json_mixed.is_line_oriented());
        assert!(!json_mixed.is_file_output());

        // MUTE (uppercase) should create a mute processor
        let mute = OutputProcessor::new("MUTE", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!mute.is_file_output());
        assert!(!mute.is_line_oriented());
    }

    #[test]
    fn test_template_format_requires_embedded_templates() {
        let result = OutputProcessor::new("ansi", "test", None, None, OutputTarget::Stdout);
        assert!(result.is_err());
    }

    #[test]
    fn test_template_format_stdout() {
        let mut output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .unwrap();
        assert!(!output.is_line_oriented());
        assert!(!output.is_file_output());
        output.generate(&test_data()).unwrap();
    }

    #[test]
    fn test_template_format_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Directory(path.clone()),
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
        let json = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!json.is_line_oriented());

        let yaml = OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!yaml.is_line_oriented());

        let jsonl =
            OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(jsonl.is_line_oriented());

        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        assert!(!mute.is_line_oriented());
    }

    #[test]
    fn test_generate_to_string_json() {
        let output =
            OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout).unwrap();
        let result = output.generate_to_string(&test_data()).unwrap();
        let parsed: TestData = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_yaml() {
        let output =
            OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout).unwrap();
        let result = output.generate_to_string(&test_data()).unwrap();
        let parsed: TestData = serde_yaml::from_str(&result).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_jsonl() {
        let output =
            OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout).unwrap();
        let result = output.generate_to_string(&test_data()).unwrap();
        let parsed: TestData = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_mute() {
        let output =
            OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        let result = output.generate_to_string(&test_data()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_generate_to_string_template() {
        let output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .unwrap();
        let result = output.generate_to_string(&test_data()).unwrap();
        assert!(result.contains("test"), "should contain name");
        assert!(result.contains("42"), "should contain value");
    }

    #[test]
    fn test_generate_to_string_template_each_array() {
        #[derive(Serialize)]
        struct Items {
            items: Vec<TestData>,
        }

        let output = OutputProcessor::new(
            "each_test",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .unwrap();

        let data = Items {
            items: vec![
                TestData {
                    name: "a".to_owned(),
                    value: 1,
                },
                TestData {
                    name: "b".to_owned(),
                    value: 2,
                },
                TestData {
                    name: "c".to_owned(),
                    value: 3,
                },
            ],
        };
        let result = output.generate_to_string(&data).unwrap();
        assert!(
            result.contains("a=1"),
            "should contain first item: {result}"
        );
        assert!(
            result.contains("b=2"),
            "should contain second item: {result}"
        );
        assert!(
            result.contains("c=3"),
            "should contain third item: {result}"
        );
    }

    #[test]
    fn test_generate_to_string_template_each_non_array() {
        // When the filter returns a non-array, each mode renders it as a single item
        #[derive(Serialize)]
        struct Items {
            items: TestData,
        }

        let output = OutputProcessor::new(
            "each_test",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .unwrap();

        let data = Items {
            items: TestData {
                name: "solo".to_owned(),
                value: 99,
            },
        };
        let result = output.generate_to_string(&data).unwrap();
        assert!(
            result.contains("solo=99"),
            "should contain the single item: {result}"
        );
    }

    #[test]
    fn test_content_type() {
        let json = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout).unwrap();
        assert_eq!(json.content_type(), "application/json");

        let yaml = OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout).unwrap();
        assert_eq!(yaml.content_type(), "application/yaml");

        let jsonl =
            OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout).unwrap();
        assert_eq!(jsonl.content_type(), "application/x-ndjson");

        let template = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .unwrap();
        assert_eq!(template.content_type(), "text/plain");

        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout).unwrap();
        assert_eq!(mute.content_type(), "text/plain");
    }

    #[test]
    fn test_from_optional_dir() {
        assert!(matches!(
            OutputTarget::from_optional_dir(None),
            OutputTarget::Stdout
        ));

        let none_path = PathBuf::from("none");
        assert!(matches!(
            OutputTarget::from_optional_dir(Some(&none_path)),
            OutputTarget::Mute
        ));

        let dir_path = PathBuf::from("/tmp/output");
        assert!(matches!(
            OutputTarget::from_optional_dir(Some(&dir_path)),
            OutputTarget::Directory(_)
        ));
    }
}
