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
use weaver_common::result::WResult;

/// A semantic convention file as defined [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
/// A semconv file either follows version 1 or 2.  Default is version 1.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum SemConvSpec {
    /// Semantic convention specification that includes a version tag.
    WithVersion(Versioned),
    /// Semantic convention specification that does NOT include a version tag.
    NoVersion(SemConvSpecV1),
}

/// A versioned semantic convention file.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "version")]
#[allow(unused_qualifications)]
pub enum Versioned {
    /// Version 1 of the semantic convention schema.
    #[serde(rename = "1")]
    V1(SemConvSpecV1),
    /// Version 2 of the semantic convention schema.
    #[serde(rename = "2")]
    V2(SemConvSpecV2),
}

// Note: We automatically create the Schemars code and provide `allow(unused_qualifications)` to work around schemars limitations.
// You can use `cargo expand -p weaver_semconv` to find this code and generate it in the future.
const _: () = {
    #[automatically_derived]
    #[allow(unused_braces, unused_qualifications)]
    impl schemars::JsonSchema for Versioned {
        fn schema_name() -> std::string::String {
            "Versioned".to_owned()
        }
        fn schema_id() -> std::borrow::Cow<'static, str> {
            std::borrow::Cow::Borrowed("weaver_semconv::semconv::Versioned")
        }
        fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
            schemars::_private::metadata::add_description(
                schemars::schema::Schema::Object(schemars::schema::SchemaObject {
                    subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                        one_of: Some(<[_]>::into_vec(Box::new([
                            schemars::_private::metadata::add_description(
                                schemars::_private::new_internally_tagged_enum(
                                    "version", "1", false,
                                ),
                                "Version 1 of the semantic convention schema.",
                            )
                            .flatten(
                                <SemConvSpecV1 as schemars::JsonSchema>::json_schema(generator),
                            ),
                            schemars::_private::metadata::add_description(
                                schemars::_private::new_internally_tagged_enum(
                                    "version", "2", false,
                                ),
                                "Version 2 of the semantic convention schema.",
                            )
                            .flatten(
                                <SemConvSpecV2 as schemars::JsonSchema>::json_schema(generator),
                            ),
                        ]))),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
                "A versioned semantic convention file.",
            )
        }
    }
};

/// A semantic convention file as defined [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
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

/// A wrapper for a [`SemConvSpec`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecWithProvenance {
    /// The semantic convention spec.
    pub spec: SemConvSpec,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: Provenance,
}

/// A wrapper for a [`SemConvSpec`] with its provenance.
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

impl SemConvSpec {
    /// Converts this SemconvSpec into the version 1 specification.
    ///
    /// name: A unique identifier to use for synthetic group ids in this semconv, if needed.
    #[must_use]
    pub fn into_v1(self, name: &str) -> SemConvSpecV1 {
        match self {
            SemConvSpec::NoVersion(v1) => v1,
            SemConvSpec::WithVersion(Versioned::V1(v1)) => v1,
            SemConvSpec::WithVersion(Versioned::V2(v2)) => v2.into_v1_specification(name),
        }
    }

    /// Validates invariants on the model.
    pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
        match self {
            SemConvSpec::NoVersion(v1) | SemConvSpec::WithVersion(Versioned::V1(v1)) => v1
                .validate(provenance)
                .map(|v1| SemConvSpec::WithVersion(Versioned::V1(v1))),
            // TODO - what validation is needed on V2?
            SemConvSpec::WithVersion(Versioned::V2(v2)) => {
                WResult::Ok(SemConvSpec::WithVersion(Versioned::V2(v2)))
            }
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

impl SemConvSpecWithProvenance {
    /// True if this specification contains V2 version.
    fn is_v2(&self) -> bool {
        matches!(
            self,
            SemConvSpecWithProvenance {
                spec: SemConvSpec::WithVersion(Versioned::V2(..)),
                ..
            }
        )
    }

    /// Converts this semconv specification into version 1, preserving provenance.
    #[must_use]
    pub fn into_v1(self) -> SemConvSpecV1WithProvenance {
        // TODO - better name
        let name = provenance_path_to_name(&self.provenance.path);
        SemConvSpecV1WithProvenance {
            spec: self.spec.into_v1(&name),
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
        validator: &JsonSchemaValidator,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        Self::from_file_with_mapped_path(registry_id, path, validator, |path| path)
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
        validator: &JsonSchemaValidator,
        path_fixer: F,
    ) -> WResult<SemConvSpecWithProvenance, Error>
    where
        P: AsRef<Path>,
        F: Fn(String) -> String,
    {
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
        let path = path.as_ref().display().to_string();
        let provenance = Provenance::new(registry_id, &path_fixer(path.clone()));
        let raw_spec = match from_file_or_fatal(path.as_ref(), &path, validator) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate(&path)
            }
            Err(e) => WResult::FatalErr(e),
        };
        let result = raw_spec.map(|spec| SemConvSpecWithProvenance { spec, provenance });
        // Check for unstable versions and add warnings.
        match result {
            WResult::Ok(spec) => {
                if spec.is_v2() {
                    let nfe = Error::UnstableFileVersion {
                        version: "2".to_owned(),
                        provenance: spec.provenance.path.clone(),
                    };
                    WResult::with_non_fatal_errors(spec, vec![nfe])
                } else {
                    WResult::Ok(spec)
                }
            }
            WResult::OkWithNFEs(spec, errs) => {
                if spec.is_v2() {
                    let mut nfes = errs;
                    nfes.push(Error::UnstableFileVersion {
                        version: "2".to_owned(),
                        provenance: spec.provenance.path.clone(),
                    });
                    WResult::OkWithNFEs(spec, nfes)
                } else {
                    WResult::OkWithNFEs(spec, errs)
                }
            }
            WResult::FatalErr(err) => WResult::FatalErr(err),
        }
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
    pub(crate) fn from_string(
        provenance: Provenance,
        spec: &str,
    ) -> WResult<SemConvSpecWithProvenance, Error> {
        let raw_spec = match serde_yaml::from_str::<SemConvSpec>(spec).map_err(|e| {
            Error::DeserializationError {
                path_or_url: "NA".to_owned(),
                error: e.to_string(),
            }
        }) {
            Ok(semconv_spec) => {
                // Important note: the resolution process expects this step of validation to be done for
                // each semantic convention spec.
                semconv_spec.validate(&provenance.path)
            }
            Err(e) => WResult::FatalErr(e),
        };
        raw_spec.map(|spec| SemConvSpecWithProvenance { spec, provenance })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        v2::{attribute::AttributeDef, CommonFields},
        Error::{
            DeserializationError, InvalidAttribute, InvalidAttributeWarning, InvalidExampleWarning,
            InvalidGroupMissingType, InvalidGroupStability, InvalidSemConvSpec,
            InvalidSpanMissingSpanKind, RegistryNotFound,
        },
    };
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn test_semconv_spec_from_file() {
        let validator = JsonSchemaValidator::new();
        // Existing file
        let path = PathBuf::from("data/database.yaml");

        let semconv_spec = SemConvSpecWithProvenance::from_file("test", path, &validator)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 10);

        // Non-existing file
        let path = PathBuf::from("data/non-existing.yaml");
        let semconv_spec = SemConvSpecWithProvenance::from_file("test", path, &validator)
            .into_result_failing_non_fatal();
        assert!(semconv_spec.is_err());
        assert!(matches!(semconv_spec.unwrap_err(), RegistryNotFound { .. }));

        // Invalid file structure
        let path = PathBuf::from("data/invalid/invalid-semconv.yaml");
        let semconv_spec = SemConvSpecWithProvenance::from_file("test", path, &validator)
            .into_result_failing_non_fatal();
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
          metrics:
            - db.*
          events:
            - db.*
          entities:
            - host
        "#;

        let semconv_spec =
            SemConvSpecWithProvenance::from_string(Provenance::new("registry", "test"), spec)
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
        let semconv_spec =
            SemConvSpecWithProvenance::from_string(Provenance::new("registry", "test"), spec)
                .into_result_failing_non_fatal();
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
        let semconv_spec =
            SemConvSpecWithProvenance::from_string(Provenance::new("registry", "<str>"), spec)
                .into_result_failing_non_fatal();
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
    fn test_semconv_spec_with_provenance_from_file() {
        let validator = JsonSchemaValidator::new();
        let path = PathBuf::from("data/database.yaml");
        let semconv_spec = SemConvSpecWithProvenance::from_file("main", &path, &validator)
            .into_result_failing_non_fatal()
            .unwrap();
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 10);
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
        assert_eq!(semconv_spec.spec.into_v1("test").groups.len(), 2);
        assert_eq!(semconv_spec.provenance, provenance);
    }

    fn parse_versioned(spec: &str) -> SemConvSpec {
        serde_yaml::from_str(spec).expect("Failed to parse SemConvSpec.")
    }

    #[test]
    fn test_versioned_semconv() {
        let sample = SemConvSpec::WithVersion(Versioned::V2(SemConvSpecV2 {
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
        }));
        let sample_yaml = serde_yaml::to_string(&sample).expect("Failed to serialize");
        assert_eq!(
            r#"version: '2'
attributes:
- key: test.key
  type: int
  brief: test attribute
  stability: stable
"#,
            sample_yaml
        );

        let raw = parse_versioned(
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
        assert!(matches!(raw, SemConvSpec::NoVersion(_)));
        let v1 = parse_versioned(r#"version: '1'"#);
        assert!(matches!(v1, SemConvSpec::WithVersion(Versioned::V1 { .. })));
        let v2 = parse_versioned("version: '2'");
        assert!(matches!(v2, SemConvSpec::WithVersion(Versioned::V2 { .. })));
    }

    #[test]
    fn test_semconv_spec_with_provenance_from_string_v2() {
        // let provenance = Provenance::new("main", "my_string");
        let spec = r#"
        version: '2'
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
          name: "{myspan}"
          attributes:
            - ref: "attr1"
        imports:
          metrics:
            - foo/*
        "#;

        let semconv_spec = SemConvSpecWithProvenance::from_string(
            Provenance {
                registry_id: "test".into(),
                path: "test".to_owned(),
            },
            spec,
        )
        .into_result_failing_non_fatal()
        .unwrap()
        .into_v1()
        .spec;
        assert_eq!(semconv_spec.groups.len(), 2);
        let mut group_ids: Vec<&str> = semconv_spec.groups.iter().map(|g| g.id.as_str()).collect();
        group_ids.sort();
        assert_eq!(vec!["registry.test", "span.group2"], group_ids);
    }
}
