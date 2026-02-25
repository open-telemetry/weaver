// SPDX-License-Identifier: Apache-2.0

//! General-purpose output processor supporting builtin formats and templates.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use include_dir::Dir;
use serde::Serialize;

use crate::config::{Params, WeaverConfig};
use crate::error::Error;
use crate::file_loader::{EmbeddedFileLoader, FileLoader};
use crate::{OutputDirective, TemplateEngine};

/// Specifies where output should be written.
#[derive(Debug, Clone)]
pub enum OutputTarget {
    /// Write to stdout.
    Stdout,
    /// Write to stderr.
    Stderr,
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

    /// Convert from the common CLI pattern for single-file output:
    /// `None` = stdout, `Some(path)` = write directly to that file.
    #[must_use]
    pub fn from_optional_file(path: Option<&PathBuf>) -> Self {
        match path {
            None => OutputTarget::Stdout,
            Some(p) => OutputTarget::File(p.clone()),
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
    /// Try to parse a format name (already lowercased) into a builtin format.
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "json" => Some(BuiltinFormat::Json),
            "yaml" => Some(BuiltinFormat::Yaml),
            "jsonl" => Some(BuiltinFormat::Jsonl),
            _ => None,
        }
    }

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
    target: OutputTarget,
}

/// Internal enum for output processor variants
enum OutputKind {
    /// Builtin format e.g., JSON, YAML, JSONL
    Builtin {
        format: BuiltinFormat,
        prefix: String,
        target: OutputTarget,
        file: Option<File>,
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

        let format_lower = format.to_lowercase();
        let has_templates = embedded_templates.is_some() || templates_path.is_some();

        // When templates are available, check for a matching template directory
        // first. This allows users to override builtin formats (e.g. "yaml")
        // with custom templates.
        let kind = if format_lower == "mute" {
            OutputKind::Mute
        } else if has_templates
            && Self::has_template_dir(format, embedded_templates, templates_path.as_ref())
        {
            Self::load_template_kind(format, embedded_templates, templates_path, output)?
        } else if let Some(builtin) = BuiltinFormat::from_name(&format_lower) {
            OutputKind::Builtin {
                format: builtin,
                prefix: prefix.to_owned(),
                target: output,
                file: None,
            }
        } else {
            // Not a builtin and no template directory found — attempt template
            // load anyway to produce a descriptive error message.
            Self::load_template_kind(format, embedded_templates, templates_path, output)?
        };

        Ok(Self { kind })
    }

    /// Check whether a template directory exists for the given format name.
    /// Probes the local filesystem path first, then the embedded directory.
    fn has_template_dir(
        format: &str,
        embedded_templates: Option<&'static Dir<'static>>,
        templates_path: Option<&PathBuf>,
    ) -> bool {
        if let Some(path) = templates_path {
            if path.join(format).exists() {
                return true;
            }
        }
        if let Some(embedded) = embedded_templates {
            if embedded.get_dir(format).is_some() {
                return true;
            }
        }
        false
    }

    /// Load a template-based OutputKind for the given format name.
    fn load_template_kind(
        format: &str,
        embedded_templates: Option<&'static Dir<'static>>,
        templates_path: Option<PathBuf>,
        output: OutputTarget,
    ) -> Result<OutputKind, Error> {
        let embedded = embedded_templates.ok_or_else(|| Error::InvalidTemplateDir {
            template_dir: PathBuf::from(format),
            error: "Template format requires embedded_templates parameter".to_owned(),
        })?;
        let templates = templates_path.unwrap_or_default();
        let loader = EmbeddedFileLoader::try_new(embedded, templates, format)?;
        let config = WeaverConfig::try_from_loader(&loader)?;
        let engine = TemplateEngine::try_new(config, loader, Params::default())?;
        Ok(OutputKind::Template(Box::new(TemplateOutput {
            engine,
            target: output,
        })))
    }

    /// Create an OutputProcessor from an explicit template configuration.
    ///
    /// Use this when you already have a `WeaverConfig`, a `FileLoader`, and `Params`
    /// (e.g. the `registry generate` and `registry update-markdown` commands).
    ///
    /// * `config` - Weaver configuration (loaded from `weaver.yaml`).
    /// * `loader` - File loader for templates.
    /// * `params` - CLI/template parameters.
    /// * `output` - Where to write output.
    pub fn from_template_config(
        config: WeaverConfig,
        loader: impl FileLoader + Send + Sync + 'static,
        params: Params,
        output: OutputTarget,
    ) -> Result<Self, Error> {
        if matches!(output, OutputTarget::Mute) {
            return Ok(Self {
                kind: OutputKind::Mute,
            });
        }
        let engine = TemplateEngine::try_new(config, loader, params)?;
        Ok(Self {
            kind: OutputKind::Template(Box::new(TemplateOutput {
                engine,
                target: output,
            })),
        })
    }

    /// Generate a template snippet from serializable context and a snippet identifier.
    ///
    /// Only valid for `Template` variants. Returns an error for `Builtin` and `Mute`.
    pub fn generate_snippet<T: Serialize>(
        &self,
        context: &T,
        filter: &str,
        snippet_id: String,
    ) -> Result<String, Error> {
        match &self.kind {
            OutputKind::Template(t) => t.engine.generate_snippet(context, filter, snippet_id),
            OutputKind::Builtin { .. } | OutputKind::Mute => Err(Error::InvalidTemplateDir {
                template_dir: PathBuf::from("(not a template)"),
                error: "generate_snippet is only supported for template-based OutputProcessor"
                    .to_owned(),
            }),
        }
    }

    /// Generate output for serializable data.
    pub fn generate<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        match &mut self.kind {
            OutputKind::Builtin {
                format,
                prefix,
                target,
                file,
            } => {
                if file.is_none() {
                    match target {
                        OutputTarget::File(p) => *file = Some(open_file(p)?),
                        OutputTarget::Directory(p) => {
                            let name = format!("{}.{}", prefix, format.extension());
                            *file = Some(open_file(&p.join(name))?);
                        }
                        _ => {} // Stdout/Stderr — no file needed
                    }
                }
                let use_stderr = matches!(target, OutputTarget::Stderr);
                let content = format.serialize(data)?;
                write_content(&content, file, format.is_line_oriented(), use_stderr)
            }
            OutputKind::Template(t) => {
                let (path, directive) = match &t.target {
                    OutputTarget::Stdout => (PathBuf::from("output"), OutputDirective::Stdout),
                    OutputTarget::Stderr => (PathBuf::from("output"), OutputDirective::Stderr),
                    OutputTarget::File(p) | OutputTarget::Directory(p) => {
                        (p.clone(), OutputDirective::File)
                    }
                    OutputTarget::Mute => {
                        return Err(Error::InternalError(
                            "Template generate called with Mute target".to_owned(),
                        ));
                    }
                };
                t.engine.generate(data, &path, &directive)
            }
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
                BuiltinFormat::Jsonl => "application/jsonl",
            },
            OutputKind::Template(_) => "text/plain",
            OutputKind::Mute => "text/plain",
        }
    }

    /// Returns true if file output is being used.
    #[must_use]
    pub fn is_file_output(&self) -> bool {
        match &self.kind {
            OutputKind::Builtin { target, .. } => {
                matches!(target, OutputTarget::File(_) | OutputTarget::Directory(_))
            }
            OutputKind::Template(t) => {
                matches!(t.target, OutputTarget::File(_) | OutputTarget::Directory(_))
            }
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

/// Create parent directories and open/truncate a file at the given path.
fn open_file(path: &std::path::Path) -> Result<File, Error> {
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

/// Write content to stdout/stderr (if `file` is `None`) or to the open file.
#[allow(clippy::print_stdout, clippy::print_stderr)]
fn write_content(
    content: &str,
    file: &mut Option<File>,
    with_newline: bool,
    use_stderr: bool,
) -> Result<(), Error> {
    match file {
        None => {
            if use_stderr {
                eprintln!("{content}");
            } else {
                println!("{content}");
            }
            Ok(())
        }
        Some(f) => if with_newline {
            writeln!(f, "{content}")
        } else {
            write!(f, "{content}")
        }
        .map_err(|e| Error::SerializationError {
            error: e.to_string(),
        }),
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
        let output = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
        assert!(!output.is_file_output());
        assert!(!output.is_line_oriented());
    }

    #[test]
    fn test_mute_via_output_target() {
        let output = OutputProcessor::new("json", "test", None, None, OutputTarget::Mute)
            .expect("json with Mute target should succeed");
        assert!(!output.is_file_output());
        assert!(!output.is_line_oriented());
    }

    #[test]
    fn test_mute_via_from_optional_dir() {
        let none_path = PathBuf::from("none");
        let target = OutputTarget::from_optional_dir(Some(&none_path));
        let output = OutputProcessor::new("json", "test", None, None, target)
            .expect("json with 'none' dir target should succeed");
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
        let temp_dir = TempDir::new().expect("should create temp dir");
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .expect("json directory output should succeed");
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output
            .generate(&test_data())
            .expect("generate should succeed");

        let file_path = path.join("test.json");
        let content = fs::read_to_string(&file_path).expect("should read output file");
        let parsed: TestData = serde_json::from_str(&content).expect("should parse JSON");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_json_format_to_direct_file() {
        let temp_dir = TempDir::new().expect("should create temp dir");
        let file_path = temp_dir.path().join("output.json");
        let mut output = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::File(file_path.clone()),
        )
        .expect("json file output should succeed");
        assert!(output.is_file_output());

        output
            .generate(&test_data())
            .expect("generate should succeed");

        let content = fs::read_to_string(&file_path).expect("should read output file");
        let parsed: TestData = serde_json::from_str(&content).expect("should parse JSON");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_yaml_format_to_directory() {
        let temp_dir = TempDir::new().expect("should create temp dir");
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "yaml",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .expect("yaml directory output should succeed");
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output
            .generate(&test_data())
            .expect("generate should succeed");

        let file_path = path.join("test.yaml");
        let content = fs::read_to_string(&file_path).expect("should read output file");
        let parsed: TestData = serde_yaml::from_str(&content).expect("should parse YAML");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_jsonl_format_to_directory() {
        let temp_dir = TempDir::new().expect("should create temp dir");
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "jsonl",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .expect("jsonl directory output should succeed");
        assert!(output.is_line_oriented());
        assert!(output.is_file_output());

        let second = TestData {
            name: "second".to_owned(),
            value: 99,
        };
        output
            .generate(&test_data())
            .expect("generate first should succeed");
        output
            .generate(&second)
            .expect("generate second should succeed");

        let file_path = path.join("test.jsonl");
        let content = fs::read_to_string(&file_path).expect("should read output file");
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let parsed_first: TestData =
            serde_json::from_str(lines[0]).expect("should parse first JSON line");
        let parsed_second: TestData =
            serde_json::from_str(lines[1]).expect("should parse second JSON line");
        assert_eq!(parsed_first, test_data());
        assert_eq!(parsed_second, second);
    }

    #[test]
    fn test_mute_generate_does_nothing() {
        let mut output = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
        assert!(output.generate(&test_data()).is_ok());
        assert!(!output.is_file_output());
    }

    #[test]
    fn test_is_file_output() {
        // Mute is not file output
        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
        assert!(!mute.is_file_output());

        // Stdout is not file output
        let stdout = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout)
            .expect("json stdout should succeed");
        assert!(!stdout.is_file_output());

        // Directory output is file output
        let temp_dir = TempDir::new().expect("should create temp dir");
        let path = temp_dir.path().to_path_buf();
        let dir = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::Directory(path.clone()),
        )
        .expect("json directory output should succeed");
        assert!(dir.is_file_output());

        // Direct file output is file output
        let file = OutputProcessor::new(
            "json",
            "test",
            None,
            None,
            OutputTarget::File(path.join("test.json")),
        )
        .expect("json file output should succeed");
        assert!(file.is_file_output());
    }

    #[test]
    fn test_format_case_insensitive() {
        // JSON (uppercase) should create a valid non-line-oriented processor
        let json_upper = OutputProcessor::new("JSON", "test", None, None, OutputTarget::Stdout)
            .expect("JSON uppercase should succeed");
        assert!(!json_upper.is_line_oriented());
        assert!(!json_upper.is_file_output());

        // Json (mixed case) should also work
        let json_mixed = OutputProcessor::new("Json", "test", None, None, OutputTarget::Stdout)
            .expect("Json mixed case should succeed");
        assert!(!json_mixed.is_line_oriented());
        assert!(!json_mixed.is_file_output());

        // MUTE (uppercase) should create a mute processor
        let mute = OutputProcessor::new("MUTE", "test", None, None, OutputTarget::Stdout)
            .expect("MUTE uppercase should succeed");
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
        .expect("template format should succeed");
        assert!(!output.is_line_oriented());
        assert!(!output.is_file_output());
        output
            .generate(&test_data())
            .expect("generate should succeed");
    }

    #[test]
    fn test_template_format_to_directory() {
        let temp_dir = TempDir::new().expect("should create temp dir");
        let path = temp_dir.path().to_path_buf();
        let mut output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Directory(path.clone()),
        )
        .expect("template directory output should succeed");
        assert!(!output.is_line_oriented());
        assert!(output.is_file_output());

        output
            .generate(&test_data())
            .expect("generate should succeed");

        let file_path = path.join("output.txt");
        let content = fs::read_to_string(&file_path).expect("should read output file");
        assert!(content.contains("test"), "should contain name");
        assert!(content.contains("42"), "should contain value");
    }

    #[test]
    fn test_is_line_oriented() {
        let json = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout)
            .expect("json format should succeed");
        assert!(!json.is_line_oriented());

        let yaml = OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout)
            .expect("yaml format should succeed");
        assert!(!yaml.is_line_oriented());

        let jsonl = OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout)
            .expect("jsonl format should succeed");
        assert!(jsonl.is_line_oriented());

        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
        assert!(!mute.is_line_oriented());
    }

    #[test]
    fn test_generate_to_string_json() {
        let output = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout)
            .expect("json format should succeed");
        let result = output
            .generate_to_string(&test_data())
            .expect("generate_to_string should succeed");
        let parsed: TestData = serde_json::from_str(&result).expect("should parse JSON");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_yaml() {
        let output = OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout)
            .expect("yaml format should succeed");
        let result = output
            .generate_to_string(&test_data())
            .expect("generate_to_string should succeed");
        let parsed: TestData = serde_yaml::from_str(&result).expect("should parse YAML");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_jsonl() {
        let output = OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout)
            .expect("jsonl format should succeed");
        let result = output
            .generate_to_string(&test_data())
            .expect("generate_to_string should succeed");
        let parsed: TestData = serde_json::from_str(&result).expect("should parse JSON");
        assert_eq!(parsed, test_data());
    }

    #[test]
    fn test_generate_to_string_mute() {
        let output = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
        let result = output
            .generate_to_string(&test_data())
            .expect("generate_to_string should succeed");
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
        .expect("template format should succeed");
        let result = output
            .generate_to_string(&test_data())
            .expect("generate_to_string should succeed");
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
        .expect("each_test template should succeed");

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
        let result = output
            .generate_to_string(&data)
            .expect("generate_to_string should succeed");
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
        .expect("each_test template should succeed");

        let data = Items {
            items: TestData {
                name: "solo".to_owned(),
                value: 99,
            },
        };
        let result = output
            .generate_to_string(&data)
            .expect("generate_to_string should succeed");
        assert!(
            result.contains("solo=99"),
            "should contain the single item: {result}"
        );
    }

    #[test]
    fn test_content_type() {
        let json = OutputProcessor::new("json", "test", None, None, OutputTarget::Stdout)
            .expect("json format should succeed");
        assert_eq!(json.content_type(), "application/json");

        let yaml = OutputProcessor::new("yaml", "test", None, None, OutputTarget::Stdout)
            .expect("yaml format should succeed");
        assert_eq!(yaml.content_type(), "application/yaml");

        let jsonl = OutputProcessor::new("jsonl", "test", None, None, OutputTarget::Stdout)
            .expect("jsonl format should succeed");
        assert_eq!(jsonl.content_type(), "application/jsonl");

        let template = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stdout,
        )
        .expect("template format should succeed");
        assert_eq!(template.content_type(), "text/plain");

        let mute = OutputProcessor::new("mute", "test", None, None, OutputTarget::Stdout)
            .expect("mute format should succeed");
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

    #[test]
    fn test_from_optional_file() {
        assert!(matches!(
            OutputTarget::from_optional_file(None),
            OutputTarget::Stdout
        ));

        let file_path = PathBuf::from("/tmp/output.json");
        assert!(matches!(
            OutputTarget::from_optional_file(Some(&file_path)),
            OutputTarget::File(_)
        ));
    }

    #[test]
    fn test_all_builtin_formats_stderr() {
        let formats = ["json", "yaml", "jsonl"];
        for name in formats {
            let mut output = OutputProcessor::new(name, "test", None, None, OutputTarget::Stderr)
                .unwrap_or_else(|e| panic!("Failed to create {name}: {e}"));
            assert!(!output.is_file_output(), "{name}");
            output
                .generate(&test_data())
                .unwrap_or_else(|e| panic!("Failed to generate {name}: {e}"));
        }
    }

    #[test]
    fn test_template_format_stderr() {
        let mut output = OutputProcessor::new(
            "simple",
            "test",
            Some(&EMBEDDED_TEMPLATES),
            None,
            OutputTarget::Stderr,
        )
        .expect("template stderr output should succeed");
        assert!(!output.is_line_oriented());
        assert!(!output.is_file_output());
        output
            .generate(&test_data())
            .expect("generate should succeed");
    }
}
