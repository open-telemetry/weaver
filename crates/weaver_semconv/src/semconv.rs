// SPDX-License-Identifier: Apache-2.0

//! Semantic convention specification.

use crate::group::{GroupSpec, GroupWildcard};
use crate::json_schema::JsonSchemaValidator;
use crate::provenance::Provenance;
use crate::v2::SemConvSpecV2;
use crate::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;
use weaver_common::result::WResult;

static VALIDATOR_V1: OnceLock<JsonSchemaValidator> = OnceLock::new();
static VALIDATOR_V2: OnceLock<JsonSchemaValidator> = OnceLock::new();
static VALIDATOR_UNVERSIONED: OnceLock<JsonSchemaValidator> = OnceLock::new();

/// A versioned semantic convention file.
#[derive(Serialize, Debug, Clone, JsonSchema)]
#[serde(tag = "file_format")]
pub enum Versioned {
    /// Version 1 of the semantic convention schema.
    #[serde(rename = "definition/1")]
    V1(SemConvSpecV1),
    /// Version 2 of the semantic convention schema.
    #[serde(rename = "definition/2")]
    V2(SemConvSpecV2),
}

/// A semantic convention file as defined [here](/schemas/semconv.schema.json)
/// A semconv file is a collection of semantic convention groups (i.e. [`GroupSpec`]).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpecV1 {
    /// A collection of semantic convention groups or [`GroupSpec`].
    #[serde(default)]
    pub(crate) groups: Vec<GroupSpec>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

/// Imports are used to reference groups defined in a dependent registry.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Imports {
    /// A list of metric group metric_name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<GroupWildcard>>,

    /// A list of event group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<GroupWildcard>>,

    /// A list of entity group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<GroupWildcard>>,
}

/// A wrapper for a [`Versioned`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecWithProvenance {
    /// The semantic convention spec.
    pub spec: Versioned,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: Provenance,
}

/// A wrapper for a [`SemConvSpecV1`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecV1WithProvenance {
    /// The semantic convention spec.
    pub spec: SemConvSpecV1,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: Provenance,
}

impl SemConvSpecV1 {
    fn validate(self, provenance: &str) -> WResult<Self, Error> {
        let mut errors: Vec<Error> = vec![];

        for group in &self.groups {
            match group.validate(provenance) {
                WResult::Ok(_) => {}
                WResult::OkWithNFEs(_, errs) => errors.extend(errs),
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        WResult::with_non_fatal_errors(self, errors)
    }

    /// Returns the list of groups in the semantic convention spec.
    #[must_use]
    pub fn groups(&self) -> &[GroupSpec] {
        &self.groups
    }

    /// Returns the list of imports in the semantic convention spec.
    #[must_use]
    pub fn imports(&self) -> Option<&Imports> {
        self.imports.as_ref()
    }
}

impl Versioned {
    /// Converts this versioned spec into the file_format 1 specification.
    ///
    /// name: A unique identifier to use for synthetic group ids in this semconv, if needed.
    #[must_use]
    pub fn into_v1(self, file_name: &str) -> SemConvSpecV1 {
        match self {
            Versioned::V1(v1) => v1,
            Versioned::V2(v2) => v2.into_v1_specification(file_name),
        }
    }

    /// Validates invariants on the model.
    pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
        match self {
            Versioned::V1(v1) => v1.validate(provenance).map(Versioned::V1),
            // TODO - what validation is needed on V2?
            Versioned::V2(v2) => WResult::Ok(Versioned::V2(v2)),
        }
    }
}

// This is a helper method to pull "normal" parts of a file path
// to give a relatively unique name to the attribute group registry
// when converting from V1 to V2.
fn provenance_path_to_name(path: &str) -> String {
    // At least allocate the full path.
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
enum FileFormat {
    /// Explicit `file_format: definition/1` (or legacy `version: 1`).
    V1,
    /// Explicit `file_format: definition/2` (or legacy `version: 2`).
    V2,
    /// No `file_format` or `version` field — treated as V1.
    Unversioned,
}

impl FileFormat {
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

    fn validator(&self) -> &'static JsonSchemaValidator {
        match self {
            FileFormat::V1 => {
                VALIDATOR_V1.get_or_init(JsonSchemaValidator::new_for::<SemConvSpecV1>)
            }
            FileFormat::V2 => {
                VALIDATOR_V2.get_or_init(JsonSchemaValidator::new_for::<SemConvSpecV2>)
            }
            FileFormat::Unversioned => {
                VALIDATOR_UNVERSIONED.get_or_init(JsonSchemaValidator::new_unversioned)
            }
        }
    }
}

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

/// Converts a serde deserialization failure into the best available error:
/// the JSON schema validator produces a more targeted message when it can,
/// otherwise falls back to the original serde error.
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
        // TODO - better name
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
    // pub fn into_v1(self) -> SemConvSpecV1
    /// Creates a semantic convention spec with provenance from a file.
    ///
    /// # Arguments:
    ///
    /// * `path` - The path to the semantic convention spec.
    ///
    /// # Returns
    ///
    /// The semantic convention with provenance or an error if the semantic
    /// convention spec is invalid.
    pub fn from_file<P: AsRef<Path>>(
        registry_id: &str,
        path: P,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        Self::from_file_with_mapped_path(registry_id, path, |path| path)
    }
    /// Creates a semantic convention spec with provenance from a file.
    ///
    /// # Arguments:
    ///
    /// * `path` - The path to the semantic convention spec.
    ///
    /// # Returns
    ///
    /// The semantic convention with provenance or an error if the semantic
    /// convention spec is invalid.
    pub fn from_file_with_mapped_path<P, F>(
        registry_id: &str,
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
        let provenance = Provenance::new(registry_id, &path_fixer(path.clone()));
        let yaml_value = match read_yaml_file(path.as_ref(), &path) {
            Ok(value) => value,
            Err(e) => return WResult::FatalErr(e),
        };
        let mut warnings = Vec::new();

        let raw_spec = match from_yaml_value(yaml_value, &path, &mut warnings) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate(&path)
            }
            Err(e) => WResult::FatalErr(e),
        };
        let result = raw_spec.map(|spec| SemConvSpecWithProvenance {
            spec,
            provenance: provenance.clone(),
        });
        if warnings.is_empty() {
            result
        } else {
            // Add warnings.
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
    use weaver_common::diagnostic::DiagnosticMessages;

    use super::*;
    use crate::{
        v2::{attribute::AttributeDef, CommonFields},
        Error::{
            CompoundError, InvalidAttribute, InvalidAttributeWarning, InvalidExampleWarning,
            InvalidGroupMissingType, InvalidGroupStability, InvalidSemConvSpec,
            InvalidSpanMissingSpanKind, RegistryNotFound,
        },
    };
    use std::{collections::BTreeMap, io::Write, path::PathBuf};

    fn make_temp_file(spec: &str) -> tempfile::NamedTempFile {
        let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(spec.as_bytes())
            .expect("Failed to write to temp file");
        temp_file
    }

    fn semconv_from_file(spec: &str) -> WResult<SemConvSpecWithProvenance, Error> {
        let temp_file = make_temp_file(spec);
        SemConvSpecWithProvenance::from_file("test", temp_file.path())
    }

    #[test]
    fn test_semconv_spec_from_file() {
        // Existing file
        let path = PathBuf::from("data/database.yaml");

        let semconv_spec = SemConvSpecWithProvenance::from_file("test", path)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 10);

        // Non-existing file
        let path = PathBuf::from("data/non-existing.yaml");
        let semconv_spec =
            SemConvSpecWithProvenance::from_file("test", path).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(semconv_spec.unwrap_err(), RegistryNotFound { .. }));

        // Invalid file structure
        let path = PathBuf::from("data/invalid/invalid-semconv.yaml");
        let semconv_spec =
            SemConvSpecWithProvenance::from_file("test", path).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(
            semconv_spec.unwrap_err(),
            InvalidSemConvSpec { .. }
        ));
    }

    #[test]
    fn test_semconv_spec_from_file_2() {
        // Valid spec
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
        "#;

        let semconv_spec = semconv_from_file(spec)
            .into_result_failing_non_fatal()
            .unwrap()
            .spec
            .into_v1("test");
        assert_eq!(semconv_spec.groups.len(), 2);
        assert!(semconv_spec.imports.is_some());
        assert_eq!(
            semconv_spec
                .imports
                .as_ref()
                .unwrap()
                .metrics
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            semconv_spec
                .imports
                .as_ref()
                .unwrap()
                .events
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            semconv_spec
                .imports
                .as_ref()
                .unwrap()
                .entities
                .as_ref()
                .unwrap()
                .len(),
            1
        );

        // Invalid yaml
        let spec = r#"
        groups:
          -
          -
        "#;
        let semconv_spec = semconv_from_file(spec).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        let err = semconv_spec.unwrap_err();
        assert!(matches!(err, CompoundError(_)), "Actual error: {:?}", err);

        // Invalid spec
        let spec = r#"
        groups:
          - id: "group1"
            brief: "description1"
            type: span
            attributes:
              - id: "attr1"
                stability: "stable"
                type: "string"
          - id: "group2"
            stability: "stable"
            brief: "description2"
            span_kind: "server"
            type: span
            attributes:
              - id: "attr2"
                type: "int"
          - id: "group3"
            stability: "stable"
            brief: "description3"
            attributes:
              - id: "attr3"
                type: "double"
                stability: stable
                brief: "Brief3"
        "#;
        let temp_file = make_temp_file(spec);
        let semconv_spec = SemConvSpecWithProvenance::from_file("test", temp_file.path())
            .into_result_failing_non_fatal();
        if let Err(CompoundError(errors)) = semconv_spec {
            assert_eq!(errors.len(), 7);
            assert_eq!(
                errors,
                vec![
                    InvalidGroupStability {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group1".to_owned(),
                        error: "This group does not contain a stability field.".to_owned(),
                    },
                    InvalidSpanMissingSpanKind {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group1".to_owned(),
                        error: "This group is a Span but the span_kind is not set.".to_owned(),
                    },
                    InvalidAttribute {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group1".to_owned(),
                        attribute_id: "attr1".to_owned(),
                        error:
                            "This attribute is not deprecated and does not contain a brief field."
                                .to_owned(),
                    },
                    InvalidExampleWarning {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group1".to_owned(),
                        attribute_id: "attr1".to_owned(),
                        error: "This attribute is a string but it does not contain any examples."
                            .to_owned(),
                    },
                    InvalidAttribute {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group2".to_owned(),
                        attribute_id: "attr2".to_owned(),
                        error:
                            "This attribute is not deprecated and does not contain a brief field."
                                .to_owned(),
                    },
                    InvalidAttributeWarning {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group2".to_owned(),
                        attribute_id: "attr2".to_owned(),
                        error: "Missing stability field.".to_owned(),
                    },
                    InvalidGroupMissingType {
                        path_or_url: temp_file.path().display().to_string(),
                        group_id: "group3".to_owned(),
                        error: "This group does not contain a type field.".to_owned(),
                    },
                ]
            );
        } else {
            panic!("Expected a compound error");
        }
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_file() {
        let path = PathBuf::from("data/database.yaml");
        let semconv_spec = SemConvSpecWithProvenance::from_file("main", &path)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 10);
        assert_eq!(semconv_spec.provenance.path, path.display().to_string());
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_file_2() {
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
        "#;

        let semconv_spec = semconv_from_file(spec)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 2);
    }

    fn parse_versioned(spec: &str) -> Versioned {
        let temp_file = make_temp_file(spec);
        SemConvSpecWithProvenance::from_file("test", temp_file.path())
            .ignore(|e| matches!(e, Error::UnstableFileFormat { .. }))
            .into_result_failing_non_fatal()
            .unwrap()
            .spec
    }

    #[test]
    fn test_versioned_semconv() {
        let sample = Versioned::V2(SemConvSpecV2 {
            attributes: vec![AttributeDef {
                key: "test.key".to_owned(),
                r#type: crate::attribute::AttributeType::PrimitiveOrArray(
                    crate::attribute::PrimitiveOrArrayTypeSpec::Int,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test attribute".to_owned(),
                    note: "".to_owned(),
                    stability: crate::stability::Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
            }],
            entities: vec![],
            events: vec![],
            metrics: vec![],
            spans: vec![],
            imports: None,
            attribute_groups: vec![],
        });
        let sample_yaml = serde_yaml::to_string(&sample).expect("Failed to serialize");
        assert_eq!(
            r#"file_format: definition/2
attributes:
- key: test.key
  type: int
  brief: test attribute
  stability: stable
"#,
            sample_yaml
        );

        let spec = parse_versioned(
            r#" groups:
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
                examples: "example1""#,
        );
        // unversioned is treated as v1
        assert!(matches!(spec, Versioned::V1 { .. }));
        let v1 = parse_versioned(r#"file_format: 'definition/1'"#);
        assert!(matches!(v1, Versioned::V1 { .. }));
        let v2 = parse_versioned("file_format: 'definition/2'");
        assert!(matches!(v2, Versioned::V2 { .. }));
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_file_v2() {
        let spec = r#"
        file_format: 'definition/2'
        attributes:
        - key: "attr1"
          stability: "stable"
          brief: "description1"
          type: "string"
          examples: "example1"
        spans:
        - type: "group2"
          stability: "stable"
          brief: "description2"
          kind: "server"
          name:
           note: "{myspan}"
          attributes:
            - ref: "attr1"
        imports:
          metrics:
            - foo/*
        "#;

        let semconv_spec = semconv_from_file(spec)
            .ignore(|e| matches!(e, Error::UnstableFileFormat { .. }))
            .into_result_failing_non_fatal()
            .unwrap();

        let spec_v1 = semconv_spec.clone().into_v1().spec;
        assert_eq!(spec_v1.groups.len(), 2);
        let mut group_ids: Vec<&str> = spec_v1.groups.iter().map(|g| g.id.as_str()).collect();
        group_ids.sort();
        assert_eq!(
            format!(
                "registry.{}",
                provenance_path_to_name(&semconv_spec.provenance.path)
            ),
            group_ids[0]
        );
        assert_eq!("span.group2", group_ids[1]);
    }

    #[test]
    fn test_error_message_bad_format() {
        let spec = r#"
        file_format: 'definition/24'
        attributes:
        - key: "attr1"
          stability: "stable"
          brief: "description1"
          type: "string"
          examples: "example1"
        "#;

        let result = semconv_from_file(spec);
        assert!(result.is_fatal());
        let mut diag_msgs = DiagnosticMessages::empty();
        let error_message = result
            .capture_non_fatal_errors(&mut diag_msgs)
            .err()
            .unwrap()
            .to_string();
        assert!(error_message.contains("Invalid file format: `file_format: definition/24`. Expected 'file_format: definition/1' or 'file_format: definition/2'"), "Actual error message: {}", error_message);
    }

    #[test]
    fn test_error_message_invalid_v1() {
        let spec = r#"
        file_format: 'definition/1'
        attributes:
        - key: "attr1"
        "#;

        let result = semconv_from_file(spec);
        assert!(result.is_fatal());
        let mut diag_msgs = DiagnosticMessages::empty();
        let error_message = result
            .capture_non_fatal_errors(&mut diag_msgs)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error_message.contains("Object contains unexpected properties: attributes. These properties are not defined in the schema."),
            "Actual error message: {}",
            error_message
        );
    }

    #[test]
    fn test_error_message_invalid_unversioned() {
        let spec = r#"
        attributes:
        - key: "attr1"
        "#;

        let result = semconv_from_file(spec);
        assert!(result.is_fatal());
        let mut diag_msgs = DiagnosticMessages::empty();
        let error_message = result
            .capture_non_fatal_errors(&mut diag_msgs)
            .err()
            .unwrap()
            .to_string();

        assert!(
            error_message.contains("Object contains unexpected properties: attributes. These properties are not defined in the schema."),
            "Actual error message: {}",
            error_message
        );
    }

    #[test]
    fn test_error_message_invalid_format_2() {
        let spec = r#"
        file_format: 'definition/2'
        groups:
          - id: group
        "#;

        let result = semconv_from_file(spec);
        assert!(result.is_fatal());
        let mut diag_msgs = DiagnosticMessages::empty();
        let error_message = result
            .capture_non_fatal_errors(&mut diag_msgs)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error_message.contains("Object contains unexpected properties: groups. These properties are not defined in the schema."),
            "Actual error message: {}",
            error_message
        );
    }
}
