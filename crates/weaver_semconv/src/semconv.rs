// SPDX-License-Identifier: Apache-2.0

//! Semantic convention specification.

use crate::group::{GroupImport, GroupSpec};
use crate::json_schema::JsonSchemaValidator;
use crate::provenance::Provenance;
use crate::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use weaver_common::result::WResult;

/// A semantic convention file as defined [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
/// A semconv file is a collection of semantic convention groups (i.e. [`GroupSpec`]).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpec {
    /// A collection of semantic convention groups or [`GroupSpec`].
    pub(crate) groups: Vec<GroupSpec>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Vec<GroupImport>>,
}

/// A wrapper for a [`SemConvSpec`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecWithProvenance {
    /// The semantic convention spec.
    pub(crate) spec: SemConvSpec,
    /// The provenance of the semantic convention spec (path or URL).
    pub(crate) provenance: Provenance,
}

impl SemConvSpec {
    /// Create a new semantic convention spec from a file.
    ///
    /// # Arguments:
    ///
    /// * `path` - The path to the [`SemConvSpec`].
    ///
    /// # Returns
    ///
    /// The [`SemConvSpec`] or an [`Error`] if the semantic convention spec is invalid.
    pub fn from_file<P: AsRef<Path>>(
        path: P,
        json_schema_validator: &JsonSchemaValidator,
    ) -> WResult<SemConvSpec, Error> {
        fn from_file_or_fatal(
            path: &Path,
            provenance: &str,
            json_schema_validator: &JsonSchemaValidator,
        ) -> Result<SemConvSpec, Error> {
            use serde_yaml::Value;
            use std::io::Seek;

            // Open file
            let mut semconv_file = File::open(path).map_err(|e| Error::RegistryNotFound {
                path_or_url: provenance.to_owned(),
                error: e.to_string(),
            })?;

            // Try direct deserialization first
            match serde_yaml::from_reader::<_, SemConvSpec>(&mut semconv_file) {
                Ok(spec) => Ok(spec),
                Err(e) => {
                    // If serde fails, try to get better errors via jsonschema
                    // Rewind file for second read
                    _ = semconv_file.rewind().ok();

                    let original_error = e.to_string();
                    let value: Result<Value, _> = serde_yaml::from_reader(&mut semconv_file);
                    if let Ok(yaml_value) = value {
                        json_schema_validator.validate_yaml(yaml_value, provenance, e)?;
                    }

                    // Fallback: return original serde error
                    Err(Error::DeserializationError {
                        path_or_url: provenance.to_owned(),
                        error: original_error,
                    })
                }
            }
        }

        let provenance = path.as_ref().display().to_string();

        match from_file_or_fatal(path.as_ref(), &provenance, json_schema_validator) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate(&provenance)
            }
            Err(e) => WResult::FatalErr(e),
        }
    }

    /// Create a new semantic convention spec from a string.
    ///
    /// # Arguments:
    ///
    /// * `spec` - The semantic convention spec in string format.
    ///
    /// # Returns
    ///
    /// The [`SemConvSpec`] or an [`Error`] if the semantic convention spec is invalid.
    pub fn from_string(spec: &str) -> WResult<SemConvSpec, Error> {
        match serde_yaml::from_str::<SemConvSpec>(spec).map_err(|e| Error::DeserializationError {
            path_or_url: "NA".to_owned(),
            error: e.to_string(),
        }) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate("<str>")
            }
            Err(e) => WResult::FatalErr(e),
        }
    }

    /// Create a new semantic convention spec from a URL.
    ///
    /// # Arguments:
    ///
    /// * `semconv_url` - The URL to the semantic convention spec.
    ///
    /// # Returns
    ///
    /// The [`SemConvSpec`] or an [`Error`] if the semantic convention spec is invalid.
    pub fn from_url(semconv_url: &str) -> WResult<SemConvSpec, Error> {
        fn from_url_or_fatal(semconv_url: &str) -> Result<SemConvSpec, Error> {
            // Create a content reader from the semantic convention URL
            let reader = ureq::get(semconv_url)
                .call()
                .map_err(|e| Error::RegistryNotFound {
                    path_or_url: semconv_url.to_owned(),
                    error: e.to_string(),
                })?
                .into_reader();

            // Deserialize the telemetry schema from the content reader
            serde_yaml::from_reader(reader).map_err(|e| Error::DeserializationError {
                path_or_url: semconv_url.to_owned(),
                error: e.to_string(),
            })
        }

        match from_url_or_fatal(semconv_url) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate(semconv_url)
            }
            Err(e) => WResult::FatalErr(e),
        }
    }

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
    pub fn imports(&self) -> Option<&[GroupImport]> {
        self.imports.as_deref()
    }
}

impl SemConvSpecWithProvenance {
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
        validator: &JsonSchemaValidator,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        let path = path.as_ref().display().to_string();
        let provenance = Provenance::new(registry_id, &path);
        SemConvSpec::from_file(path, validator)
            .map(|spec| SemConvSpecWithProvenance { spec, provenance })
    }

    /// Creates a semantic convention spec with provenance from a string.
    ///
    /// # Arguments:
    ///
    /// * `provenance` - The provenance of the semantic convention spec.
    /// * `spec` - The semantic convention spec.
    ///
    /// # Returns
    ///
    /// The semantic convention with provenance or an error if the semantic
    /// convention spec is invalid.
    pub fn from_string(
        provenance: Provenance,
        spec: &str,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        SemConvSpec::from_string(spec).map(|spec| SemConvSpecWithProvenance { spec, provenance })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error::{
        DeserializationError, InvalidAttribute, InvalidAttributeWarning, InvalidExampleWarning,
        InvalidGroupMissingType, InvalidGroupStability, InvalidSemConvSpec,
        InvalidSpanMissingSpanKind, RegistryNotFound,
    };
    use std::path::PathBuf;
    use weaver_common::test::ServeStaticFiles;

    #[test]
    fn test_semconv_spec_from_file() {
        let validator = JsonSchemaValidator::new();
        // Existing file
        let path = PathBuf::from("data/database.yaml");
        let semconv_spec = SemConvSpec::from_file(path, &validator)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.groups.len(), 10);

        // Non-existing file
        let path = PathBuf::from("data/non-existing.yaml");
        let semconv_spec = SemConvSpec::from_file(path, &validator).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(semconv_spec.unwrap_err(), RegistryNotFound { .. }));

        // Invalid file structure
        let path = PathBuf::from("data/invalid-semconv.yaml");
        let semconv_spec = SemConvSpec::from_file(path, &validator).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(
            semconv_spec.unwrap_err(),
            InvalidSemConvSpec { .. }
        ));
    }

    #[test]
    fn test_semconv_spec_from_string() {
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
          - metric_ref: "metric_id"
          - event_ref: "event_id"
          - entity_ref: "entity_id"
        "#;
        let semconv_spec = SemConvSpec::from_string(spec)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.groups.len(), 2);
        assert_eq!(semconv_spec.imports.as_ref().unwrap().len(), 3);

        // Invalid yaml
        let spec = r#"
        groups:
          -
          -
        "#;
        let semconv_spec = SemConvSpec::from_string(spec).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(
            semconv_spec.unwrap_err(),
            DeserializationError { .. }
        ));

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
        let semconv_spec = SemConvSpec::from_string(spec).into_result_failing_non_fatal();
        if let Err(Error::CompoundError(errors)) = semconv_spec {
            assert_eq!(errors.len(), 7);
            assert_eq!(
                errors,
                vec![
                    InvalidGroupStability {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group1".to_owned(),
                        error: "This group does not contain a stability field.".to_owned(),
                    },
                    InvalidSpanMissingSpanKind {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group1".to_owned(),
                        error: "This group is a Span but the span_kind is not set.".to_owned(),
                    },
                    InvalidAttribute {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group1".to_owned(),
                        attribute_id: "attr1".to_owned(),
                        error:
                            "This attribute is not deprecated and does not contain a brief field."
                                .to_owned(),
                    },
                    InvalidExampleWarning {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group1".to_owned(),
                        attribute_id: "attr1".to_owned(),
                        error: "This attribute is a string but it does not contain any examples."
                            .to_owned(),
                    },
                    InvalidAttribute {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group2".to_owned(),
                        attribute_id: "attr2".to_owned(),
                        error:
                            "This attribute is not deprecated and does not contain a brief field."
                                .to_owned(),
                    },
                    InvalidAttributeWarning {
                        path_or_url: "<str>".to_owned(),
                        group_id: "group2".to_owned(),
                        attribute_id: "attr2".to_owned(),
                        error: "Missing stability field.".to_owned(),
                    },
                    InvalidGroupMissingType {
                        path_or_url: "<str>".to_owned(),
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
    fn test_semconv_spec_from_url() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();
        // Existing URL. The URL is a raw file from the semantic conventions repository.
        // This file is expected to be available.
        let semconv_url = server.relative_path_to_url("url/common.yaml");
        let semconv_spec = SemConvSpec::from_url(&semconv_url)
            .into_result_failing_non_fatal()
            .unwrap();
        assert!(!semconv_spec.groups.is_empty());

        // Invalid semconv file
        let semconv_url = server.relative_path_to_url("README.md");
        let semconv_spec = SemConvSpec::from_url(&semconv_url).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(
            semconv_spec.unwrap_err(),
            DeserializationError { .. }
        ));

        // Non-existing URL
        let semconv_url = server.relative_path_to_url("unknown-semconv.yaml");
        let semconv_spec = SemConvSpec::from_url(&semconv_url).into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(semconv_spec.unwrap_err(), RegistryNotFound { .. }));
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_file() {
        let validator = JsonSchemaValidator::new();
        let path = PathBuf::from("data/database.yaml");
        let semconv_spec = SemConvSpecWithProvenance::from_file("main", &path, &validator)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.groups.len(), 10);
        assert_eq!(semconv_spec.provenance.path, path.display().to_string());
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_string() {
        let provenance = Provenance::new("main", "<str>");
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

        let semconv_spec = SemConvSpecWithProvenance::from_string(provenance.clone(), spec)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.groups.len(), 2);
        assert_eq!(semconv_spec.provenance, provenance);
    }
}
