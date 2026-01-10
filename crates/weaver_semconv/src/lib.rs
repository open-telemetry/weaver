// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use crate::Error::CompoundError;
use miette::{Diagnostic, NamedSource, SourceSpan};
use schemars::schema::{InstanceType, Schema};
use schemars::{JsonSchema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::hash::Hasher;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};

pub mod any_value;
pub mod attribute;
pub mod deprecated;
pub mod group;
pub mod json_schema;
pub mod manifest;
pub mod provenance;
pub mod registry;
pub mod registry_repo;
pub mod semconv;
pub mod stability;
pub mod stats;
pub mod v2;

/// An error that can occur while loading a semantic convention registry.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// The semantic convention registry path pattern is invalid.
    #[error("The semantic convention registry path pattern is invalid (path_pattern: {path_pattern:?}). {error}")]
    InvalidRegistryPathPattern {
        /// The path pattern pointing to the semantic convention registry.
        path_pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention registry is not found.
    #[error(
        "The semantic convention registry is not found (path_or_url: {path_or_url:?}). {error}"
    )]
    RegistryNotFound {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// A generic error related to a semantic convention spec.
    #[error(
        "The following error occurred during the processing of semantic convention file: {error}"
    )]
    SemConvSpecError {
        /// The error that occurred.
        error: String,
    },

    /// A deserialization error occurred while processing a semantic convention group.
    #[error("{error}")]
    #[diagnostic(severity(Error))]
    DeserializationError {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention spec is invalid.
    ///
    /// Note: We use a boxed error type here to keep the main `Error` type under the 128 bytes
    /// limit and avoid the `result_large_err` clippy lint.
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidSemConvSpec(#[from] Box<InvalidSemConvSpecError>),

    /// The provided xpath is invalid.
    #[error("Invalid XPath `{xpath}` detected while validating semantic convention spec.\nError: {error}")]
    InvalidXPath {
        /// The invalid XPath expression.
        xpath: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition.
    #[error("Invalid group '{group_id}' detected while resolving '{path_or_url:?}'. {error}")]
    InvalidGroup {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains a group with duplicate attribute references.
    #[error("Duplicate attribute refs for '{attribute_ref}' found on group '{group_id}' detected while resolving '{path_or_url:?}'.")]
    #[diagnostic(severity(Warning))]
    InvalidGroupDuplicateAttributeRef {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// That path or URL of the semantic convention asset.
        group_id: String,
        /// The attribute being referenced twice.
        attribute_ref: String,
    },

    /// The semantic convention spec contains an invalid group stability definition.
    #[error("Invalid stability on group '{group_id}' detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupStability {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition. Missing extends or attributes
    #[error("Invalid group '{group_id}', missing extends or attributes, detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupMissingExtendsOrAttributes {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition. Group missing type.
    #[error("Invalid group '{group_id}', missing type, detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupMissingType {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention spec contains an invalid group definition. Span missing span_kind.
    #[error("Invalid Span group '{group_id}', missing span_kind, detected while resolving '{path_or_url:?}'. {error}")]
    #[diagnostic(severity(Warning))]
    InvalidSpanMissingSpanKind {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention asset contains an invalid attribute definition.
    #[error("Invalid attribute definition detected while resolving '{path_or_url:?}' (group_id='{group_id}', attribute_id='{attribute_id}'). {error}")]
    InvalidAttribute {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The semantic convention asset contains an invalid attribute definition.
    #[error("Invalid attribute definition detected while resolving '{path_or_url:?}' (group_id='{group_id}', attribute_id='{attribute_id}'). {error}")]
    #[diagnostic(severity(Warning))]
    InvalidAttributeWarning {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This error occurs when a semantic convention asset contains an invalid example.
    /// This is treated as a critical error in the current context.
    #[error("The attribute `{attribute_id}` in the group `{group_id}` contains an invalid example. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Error))]
    InvalidExampleError {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This warning indicates that a semantic convention asset contains an invalid example.
    /// It is treated as a non-critical warning unless the `--future` flag is enabled.
    /// With the `--future` flag, this warning is elevated to an error.
    #[error("The attribute `{attribute_id}` in the group `{group_id}` contains an example that will be considered invalid in the future. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Warning))]
    InvalidExampleWarning {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the attribute.
        attribute_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This warning indicates usage of `prefix` on a group.
    /// With the `--future` flag, this warning is elevated to an error.
    #[error("The group `{group_id}` defines a prefix. These are no longer used.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Warning))]
    InvalidGroupUsesPrefix {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
    },

    /// The semantic convention asset contains an invalid metric definition.
    #[error("Invalid metric definition in {path_or_url:?}.\ngroup_id=`{group_id}`. {error}")]
    InvalidMetric {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the metric.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This indicates that any_value is invalid.
    #[error("The value `{value_id}` in the group `{group_id}` is invalid. {error}\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Warning))]
    InvalidAnyValue {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the any_value
        value_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This indicates that a semantic convention asset contains an invalid example.
    #[error("The value `{value_id}` in the group `{group_id}` contains an invalid example. {error}.\nProvenance: {path_or_url:?}")]
    #[diagnostic(severity(Error))]
    InvalidAnyValueExampleError {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The id of the any_value
        value_id: String,
        /// The reason of the error.
        error: String,
    },

    /// This error is raised when a registry manifest is not found.
    #[error("The registry manifest at {path:?} is not found.")]
    #[diagnostic(severity(Error))]
    RegistryManifestNotFound {
        /// The path to the registry manifest file.
        path: PathBuf,
    },

    /// This error is raised when a registry manifest is invalid.
    #[error("The registry manifest at {path:?} is invalid. {error}")]
    #[diagnostic(severity(Error))]
    InvalidRegistryManifest {
        /// The path to the registry manifest file.
        path: PathBuf,
        /// The error that occurred.
        error: String,
    },

    /// A virtual directory error.
    #[error(transparent)]
    VirtualDirectoryError(#[from] weaver_common::Error),

    /// An invalid registry archive.
    #[error("The registry archive `{archive}` is invalid: {error}")]
    InvalidRegistryArchive {
        /// The registry archive path
        archive: String,
        /// The error message
        error: String,
    },

    /// This indicates the file version used is not yet stable.
    #[error("Version `{version}` schema file format is not yet stable: {provenance}")]
    #[diagnostic(severity(Warning))]
    UnstableFileVersion {
        /// The version specified.
        version: String,
        /// The source using that version.
        provenance: String,
    },

    /// This indicates that deprecated property is invalid
    #[error(
        "The `deprecated` property in `{id}` is invalid. {error}\nProvenance: {path_or_url:?}"
    )]
    #[diagnostic(severity(Warning))]
    UnstructuredDeprecatedProperty {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        id: String,
        /// The reason of the error.
        error: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),
}

/// The semantic convention spec is invalid.
///
/// Boxed detailed error struct for `InvalidSemConvSpec` variant.
/// This is used to keep main error type `Error` under the limit of 128 bytes.
/// See: [result_large_err](https://rust-lang.github.io/rust-clippy/master/index.html#result_large_err)
///
/// Note: The JSON schema governing the syntax of semantic conventions can be generated
/// using the `weaver registry json-schema -j semconv-group` command.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Serialize, Diagnostic)]
#[error("{error}")]
#[diagnostic(severity(Error), code(invalid_semconv_group))]
pub struct InvalidSemConvSpecError {
    /// The YAML content of the semantic convention spec (if available).
    #[source_code]
    #[serde(skip_serializing)]
    pub src: NamedSource<Cow<'static, str>>,

    /// The span of the error in the semantic convention spec.
    #[label("somewhere in this block")]
    pub err_span: Option<SourceSpan>,

    /// The error that occurred.
    pub error: String,

    /// Optional advice to help the user understand the error.
    #[help]
    pub advice: Option<String>,
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

/// Create a newtype wrapper for serde_yaml::value::Value in order to implement
/// JsonSchema for it.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(transparent)]
pub struct YamlValue(pub serde_yaml::value::Value);

impl JsonSchema for YamlValue {
    fn schema_name() -> String {
        "YamlValue".to_owned()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        // Create a schema that accepts any type
        let schema = schemars::schema::SchemaObject {
            instance_type: Some(
                vec![
                    InstanceType::Null,
                    InstanceType::Boolean,
                    InstanceType::Object,
                    InstanceType::Array,
                    InstanceType::Number,
                    InstanceType::String,
                ]
                .into(),
            ),
            ..Default::default()
        };

        Schema::Object(schema)
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for YamlValue {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
            utoipa::openapi::schema::ObjectBuilder::new()
                .description(Some("A YAML value that can be any valid YAML type"))
                .build(),
        ))
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for YamlValue {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("YamlValue")
    }
}

/// Implement Hash for YamlValue.
/// Keys are sorted for consistent hashing in the case of mappings/objects.
impl std::hash::Hash for YamlValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Convert the YAML value to a string representation for hashing
        // This is a simplification that works for most cases
        match &self.0 {
            serde_yaml::Value::Null => {
                0_u8.hash(state);
                "null".hash(state);
            }
            serde_yaml::Value::Bool(b) => {
                1_u8.hash(state);
                b.hash(state);
            }
            serde_yaml::Value::Number(n) => {
                2_u8.hash(state);
                // Convert number to string for hashing as Number itself doesn't implement Hash
                n.to_string().hash(state);
            }
            serde_yaml::Value::String(s) => {
                3_u8.hash(state);
                s.hash(state);
            }
            serde_yaml::Value::Sequence(seq) => {
                4_u8.hash(state);
                // Hash each element's string representation
                for item in seq {
                    YamlValue(item.clone()).hash(state);
                }
            }
            serde_yaml::Value::Mapping(map) => {
                5_u8.hash(state);
                // Sort keys for consistent hashing
                let mut keys: Vec<_> = map.keys().cloned().collect();

                // Custom sort function that doesn't rely on to_string()
                keys.sort_by(|a, b| {
                    // Compare keys based on their variant first
                    let type_order = |v: &serde_yaml::Value| -> u8 {
                        match v {
                            serde_yaml::Value::Null => 0,
                            serde_yaml::Value::Bool(_) => 1,
                            serde_yaml::Value::Number(_) => 2,
                            serde_yaml::Value::String(_) => 3,
                            serde_yaml::Value::Sequence(_) => 4,
                            serde_yaml::Value::Mapping(_) => 5,
                            serde_yaml::Value::Tagged(_) => 6,
                        }
                    };

                    let a_order = type_order(a);
                    let b_order = type_order(b);

                    if a_order != b_order {
                        return a_order.cmp(&b_order);
                    }

                    // If same type, do a specialized comparison
                    match (a, b) {
                        (serde_yaml::Value::Null, serde_yaml::Value::Null) => {
                            std::cmp::Ordering::Equal
                        }
                        (serde_yaml::Value::Bool(a_val), serde_yaml::Value::Bool(b_val)) => {
                            a_val.cmp(b_val)
                        }
                        (serde_yaml::Value::Number(a_val), serde_yaml::Value::Number(b_val)) => {
                            // Compare as strings since we can't directly compare numbers
                            a_val.to_string().cmp(&b_val.to_string())
                        }
                        (serde_yaml::Value::String(a_val), serde_yaml::Value::String(b_val)) => {
                            a_val.cmp(b_val)
                        }
                        // For complex types, we'll use a hash-based comparison
                        // This isn't ideal for sorting but ensures consistency
                        _ => {
                            // Create a hasher and hash both values
                            let mut a_hasher = std::collections::hash_map::DefaultHasher::new();
                            let mut b_hasher = std::collections::hash_map::DefaultHasher::new();

                            YamlValue(a.clone()).hash(&mut a_hasher);
                            YamlValue(b.clone()).hash(&mut b_hasher);

                            a_hasher.finish().cmp(&b_hasher.finish())
                        }
                    }
                });

                // Hash each key-value pair
                for key in keys {
                    YamlValue(key.clone()).hash(state);
                    if let Some(value) = map.get(&key) {
                        YamlValue(value.clone()).hash(state);
                    }
                }
            }
            serde_yaml::Value::Tagged(tag) => {
                6_u8.hash(state);
                tag.tag.hash(state);
                YamlValue(tag.value.clone()).hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::SemConvRegistry;
    use std::vec;
    use weaver_common::diagnostic::DiagnosticMessages;

    /// Load multiple semantic convention files in the semantic convention registry.
    /// No error should be emitted.
    #[test]
    fn test_valid_semconv_registry() {
        let result = SemConvRegistry::try_from_path_pattern("main", "data/*.yaml")
            .into_result_failing_non_fatal();
        assert!(result.is_ok(), "{:#?}", result.err().unwrap());
    }

    #[test]
    fn test_invalid_semconv_registry() {
        let yaml_files = vec!["data/invalid/*.yaml"];
        for yaml in yaml_files {
            let result = SemConvRegistry::try_from_path_pattern("main", yaml)
                .into_result_failing_non_fatal();
            assert!(result.is_err(), "{:#?}", result.ok().unwrap());
            if let Err(err) = result {
                let output = format!("{err}");
                let diag_msgs: DiagnosticMessages = err.into();
                assert_eq!(diag_msgs.len(), 1, "Unexpected diagnostics: {diag_msgs:#?}");
                assert!(!output.is_empty());
            }
        }
    }
}
