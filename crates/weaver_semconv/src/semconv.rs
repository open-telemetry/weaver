// SPDX-License-Identifier: Apache-2.0

//! Semantic convention specification loading and version detection.

use crate::json_schema::JsonSchemaValidator;
use crate::provenance::Provenance;
pub use crate::v1::semconv::{SemConvSpecV1, SemConvSpecV1WithProvenance};
use crate::v2::SemConvSpecV2;
use crate::Error;
use schemars::JsonSchema;
use serde::Serialize;
use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;
use weaver_common::result::WResult;

static VALIDATOR_V1: OnceLock<JsonSchemaValidator> = OnceLock::new();
static VALIDATOR_V2: OnceLock<JsonSchemaValidator> = OnceLock::new();

/// A versioned semantic convention file.
#[derive(Serialize, Debug, Clone, JsonSchema)]
#[serde(tag = "file_format")]
#[allow(
    clippy::large_enum_variant,
    reason = "We plan to remove the variant in the future, and want SemconvSpecV2 (largest) to remain on stack."
)]
pub enum Versioned {
    /// Version 1 of the semantic convention schema.
    #[serde(rename = "definition/1")]
    V1(SemConvSpecV1),
    /// Version 2 of the semantic convention schema.
    #[serde(rename = "definition/2")]
    V2(SemConvSpecV2),
}

/// A wrapper for a [`Versioned`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecWithProvenance {
    /// The semantic convention spec.
    pub spec: Versioned,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: Provenance,
}

impl Versioned {
    /// Converts this versioned spec into the file_format 1 specification.
    ///
    /// name: A unique identifier to use for synthetic group ids in this semconv, if needed.
    #[must_use]
    pub fn into_v1(self, file_name: &str) -> SemConvSpecV1 {
        match self {
            Versioned::V1(v1) => v1,
            Versioned::V2(v2) => crate::convert::v2_to_v1_spec(v2, file_name),
        }
    }

    /// Validates invariants on the model.
    pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
        match self {
            Versioned::V1(v1) => v1.validate(provenance).map(Versioned::V1),
            Versioned::V2(v2) => v2.validate(provenance).map(Versioned::V2),
        }
    }
}

// This is a helper method to pull "normal" parts of a file path
// to give a relatively unique name to the attribute group registry
// when converting from V1 to V2.
fn provenance_path_to_name(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut need_dot = false;
    let p = Path::new(path);
    for component in p.components() {
        if let std::path::Component::Normal(part) = component {
            if let Some(safe_name) = Path::new(part)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .or(part.to_str())
            {
                if need_dot {
                    result.push('.');
                }
                result.push_str(safe_name);
                need_dot = true;
            }
        }
    }

    result
}

/// The detected file format of a semantic convention spec.
#[derive(Debug)]
enum FileFormat {
    /// Explicit `file_format: definition/1` (or legacy `version: 1`).
    V1,
    /// Explicit `file_format: definition/2` (or legacy `version: 2`).
    V2,
    /// No `file_format` or `version` field — treated as V1.
    Unversioned,
}

impl FileFormat {
    /// Detects the file format of a semantic convention spec from its YAML representation
    /// and produces warnings for deprecated or unstable formats.
    /// Returns an error if the file format is invalid.
    fn detect(
        yaml_value: &serde_yaml::Value,
        provenance: &str,
        warnings: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        use serde_yaml::Value;

        // Check for deprecated version field
        let version = yaml_value
            .get(Value::String("version".to_owned()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        if version.is_some() {
            warnings.push(Error::DeprecatedVersionField {
                provenance: provenance.to_owned(),
            });
        }

        let file_format = yaml_value
            .get(Value::String("file_format".to_owned()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let is_v2 =
            file_format == Some("definition/2".to_owned()) || version == Some("2".to_owned());
        let is_v1 =
            file_format == Some("definition/1".to_owned()) || version == Some("1".to_owned());

        if is_v2 {
            warnings.push(Error::UnstableFileFormat {
                file_format: "definition/2".to_owned(),
                provenance: provenance.to_owned(),
            });
            Ok(FileFormat::V2)
        } else if is_v1 {
            Ok(FileFormat::V1)
        } else if file_format.is_none() && version.is_none() {
            Ok(FileFormat::Unversioned)
        } else {
            Err(Error::InvalidFileFormat {
                field_key: if version.is_some() {
                    "version".to_owned()
                } else {
                    "file_format".to_owned()
                },
                field_value: version
                    .as_deref()
                    .or(file_format.as_deref())
                    .unwrap_or("unknown")
                    .to_owned(),
            })
        }
    }

    /// Returns the JSON schema validator for this file format, initializing it if necessary.
    fn validator(&self) -> &'static JsonSchemaValidator {
        match self {
            FileFormat::V1 | FileFormat::Unversioned => {
                VALIDATOR_V1.get_or_init(JsonSchemaValidator::new_for::<SemConvSpecV1>)
            }
            FileFormat::V2 => {
                VALIDATOR_V2.get_or_init(JsonSchemaValidator::new_for::<SemConvSpecV2>)
            }
        }
    }
}

/// Auxiliary function to clean the YAML mapping by removing version fields
fn clean_yaml_mapping(
    yaml_value: serde_yaml::Value,
    provenance: &str,
) -> Result<serde_yaml::Value, Error> {
    use serde_yaml::Value;

    let mut mapping = match yaml_value {
        Value::Mapping(m) => m,
        o => {
            return Err(Error::DeserializationError {
                path_or_url: provenance.to_owned(),
                error: format!("Expected a YAML mapping at the root, but found: {o:?}"),
            })
        }
    };

    _ = mapping.remove(Value::String("file_format".to_owned()));
    _ = mapping.remove(Value::String("version".to_owned()));
    Ok(Value::Mapping(mapping))
}

/// Converts a serde deserialization failure into the best available error.
fn better_error(
    value: serde_yaml::Value,
    provenance: &str,
    validator: &JsonSchemaValidator,
    e: serde_yaml::Error,
) -> Error {
    let fallback = Error::DeserializationError {
        path_or_url: provenance.to_owned(),
        error: e.to_string(),
    };
    match validator.validate_yaml(value, provenance, e) {
        Ok(()) => fallback,
        Err(better_err) => better_err,
    }
}

/// Converts a yaml value into a versioned semantic convention spec.
fn from_yaml_value(
    yaml_value: serde_yaml::Value,
    provenance: &str,
    warnings: &mut Vec<Error>,
) -> Result<Versioned, Error> {
    let format = FileFormat::detect(&yaml_value, provenance, warnings)?;
    let cleaned = clean_yaml_mapping(yaml_value, provenance)?;
    let validator = format.validator();

    match format {
        FileFormat::V2 => serde_yaml::from_value::<SemConvSpecV2>(cleaned.clone())
            .map(Versioned::V2)
            .map_err(|e| better_error(cleaned, provenance, validator, e)),
        FileFormat::V1 | FileFormat::Unversioned => {
            serde_yaml::from_value::<SemConvSpecV1>(cleaned.clone())
                .map(Versioned::V1)
                .map_err(|e| better_error(cleaned, provenance, validator, e))
        }
    }
}

impl SemConvSpecWithProvenance {
    /// Converts this semconv specification into version 1, preserving provenance.
    #[must_use]
    pub fn into_v1(self) -> SemConvSpecV1WithProvenance {
        let file_name = provenance_path_to_name(&self.provenance.path);
        log::debug!(
            "Translating v2 spec into v1 spec for {}, {}",
            file_name,
            self.provenance.path
        );
        SemConvSpecV1WithProvenance {
            spec: self.spec.into_v1(&file_name),
            provenance: self.provenance,
        }
    }

    /// Creates a semantic convention spec with provenance from a file.
    pub fn from_file<P: AsRef<Path>>(
        schema_url: crate::schema_url::SchemaUrl,
        path: P,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        Self::from_file_with_mapped_path(schema_url, path, |path| path)
    }

    /// Creates a semantic convention spec with provenance from a file with a mapped path.
    pub fn from_file_with_mapped_path<P, F>(
        schema_url: crate::schema_url::SchemaUrl,
        path: P,
        path_fixer: F,
    ) -> WResult<SemConvSpecWithProvenance, Error>
    where
        P: AsRef<Path>,
        F: Fn(String) -> String,
    {
        fn read_yaml_file(path: &Path, provenance: &str) -> Result<serde_yaml::Value, Error> {
            let semconv_file = File::open(path).map_err(|e| Error::RegistryNotFound {
                path_or_url: provenance.to_owned(),
                error: e.to_string(),
            })?;

            serde_yaml::from_reader(semconv_file).map_err(|e| Error::DeserializationError {
                path_or_url: provenance.to_owned(),
                error: e.to_string(),
            })
        }

        let path = path.as_ref().display().to_string();
        let provenance = Provenance::new(schema_url, &path_fixer(path.clone()));
        let yaml_value = match read_yaml_file(path.as_ref(), &path) {
            Ok(value) => value,
            Err(e) => return WResult::FatalErr(e),
        };
        let mut warnings = Vec::new();

        let raw_spec = match from_yaml_value(yaml_value, &path, &mut warnings) {
            Ok(semconv_spec) => semconv_spec.validate(&path),
            Err(e) => WResult::FatalErr(e),
        };
        let result = raw_spec.map(|spec| SemConvSpecWithProvenance {
            spec,
            provenance: provenance.clone(),
        });
        if warnings.is_empty() {
            result
        } else {
            match result {
                WResult::Ok(spec) => WResult::OkWithNFEs(spec, warnings),
                WResult::OkWithNFEs(spec, mut errs) => {
                    errs.extend(warnings);
                    WResult::OkWithNFEs(spec, errs)
                }
                WResult::FatalErr(err) => WResult::FatalErr(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error::{InvalidSemConvSpec, RegistryNotFound};
    use std::io::Write;
    use std::path::PathBuf;

    fn make_temp_file(spec: &str) -> tempfile::NamedTempFile {
        let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(spec.as_bytes())
            .expect("Failed to write to temp file");
        temp_file
    }

    fn semconv_from_file(spec: &str) -> WResult<SemConvSpecWithProvenance, Error> {
        let temp_file = make_temp_file(spec);
        SemConvSpecWithProvenance::from_file(
            crate::schema_url::SchemaUrl::new_unknown(),
            temp_file.path(),
        )
    }

    #[test]
    fn test_semconv_spec_from_file() {
        let path = PathBuf::from("data/database.yaml");
        let semconv_spec =
            SemConvSpecWithProvenance::from_file(crate::schema_url::SchemaUrl::new_unknown(), path)
                .into_result_failing_non_fatal()
                .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 10);

        let path = PathBuf::from("data/non-existing.yaml");
        let semconv_spec =
            SemConvSpecWithProvenance::from_file(crate::schema_url::SchemaUrl::new_unknown(), path)
                .into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(semconv_spec.unwrap_err(), RegistryNotFound { .. }));

        let path = PathBuf::from("data/invalid/invalid-semconv.yaml");
        let semconv_spec =
            SemConvSpecWithProvenance::from_file(crate::schema_url::SchemaUrl::new_unknown(), path)
                .into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(
            semconv_spec.unwrap_err(),
            InvalidSemConvSpec { .. }
        ));
    }

    #[test]
    fn test_semconv_spec_from_file_2() {
        let spec = r#"
        groups:
          - id: "group1"
            stability: "stable"
            brief: "description1"
            span_kind: "client"
            type: span
            attributes:
              - id: "attr1"
                stability: "stable"
                brief: "description1"
                type: "string"
                examples: "example1"
          - id: "group2"
            stability: "stable"
            brief: "description2"
            span_kind: "server"
            type: span
            attributes:
              - id: "attr2"
                stability: "stable"
                brief: "description2"
                type: "int"
        imports:
          metrics:
            - db.*
          events:
            - db.*
          entities:
            - host
          spans:
            - db.*
          attribute_groups:
            - db.*
        "#;

        let semconv_spec = semconv_from_file(spec)
            .into_result_failing_non_fatal()
            .unwrap()
            .spec
            .into_v1("test");
        assert_eq!(semconv_spec.groups.len(), 2);
        assert!(semconv_spec.imports.is_some());
    }

    #[test]
    fn test_versioned_semconv() {
        let v1_yaml = r#"
        file_format: definition/1
        groups:
          - id: "group1"
            brief: "description1"
            stability: "stable"
            type: attribute_group
            attributes:
              - id: "attr1"
                type: "int"
                brief: "desc"
                stability: "stable"
        "#;
        let v1 = semconv_from_file(v1_yaml)
            .into_result_failing_non_fatal()
            .unwrap();
        assert!(matches!(v1.spec, Versioned::V1(_)));

        let v2_yaml = r#"
        file_format: definition/2
        attributes:
          - key: "attr1"
            type: string
            brief: "desc"
            stability: stable
        "#;
        let (v2, _) = semconv_from_file(v2_yaml)
            .into_result_with_non_fatal()
            .unwrap();
        assert!(matches!(v2.spec, Versioned::V2(_)));
    }
}
