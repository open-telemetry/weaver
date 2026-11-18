// SPDX-License-Identifier: Apache-2.0

//! JSON Schema validator for semantic convention files.

use crate::Error::{CompoundError, InvalidSemConvSpec, InvalidXPath};
use crate::{Error, InvalidSemConvSpecError};
use itertools::Itertools;
use jsonschema::error::{TypeKind, ValidationErrorKind};
use jsonschema::{JsonType, JsonTypeSet, ValidationError, Validator};
use miette::{NamedSource, SourceSpan};
use saphyr::{LoadableYamlNode, MarkedYaml};
use schemars::{schema_for, JsonSchema};
use serde_json::Value;
use std::borrow::Cow;
use std::fs;

/// Parsed simple XPath components.
#[derive(Debug, Clone)]
enum PathComponent {
    Key(String),
    Index(usize),
}

/// JSON schema validator for semantic conventions.
pub struct JsonSchemaValidator {
    /// The JSON schema for the semantic convention specification.
    json_schema: Value,

    /// JSON schema validator instance.
    validator: Validator,
}

impl JsonSchemaValidator {
    /// Creates a new JSON schema validator for type `T`.
    #[must_use]
    pub fn new_for<T: JsonSchema>() -> Self {
        let root_schema = schema_for!(T);
        let json_schema =
            serde_json::to_value(&root_schema).expect("Failed to convert schema to JSON value");
        let validator =
            Validator::new(&json_schema).expect("Failed to create JSON schema validator");
        JsonSchemaValidator {
            json_schema,
            validator,
        }
    }

    /// Validates `json` against the schema, reading source spans from the file at `provenance`.
    pub fn validate_yaml(&self, json: &Value, provenance: &str) -> Result<(), Error> {
        let yaml_doc = YamlDoc::try_from_file(provenance)?;
        self.validate_yaml_doc(json, yaml_doc, provenance)
    }

    /// Validates `json` against the schema, resolving source spans against `raw_yaml`.
    pub(crate) fn validate_yaml_str(
        &self,
        json: &Value,
        raw_yaml: &str,
        provenance: &str,
    ) -> Result<(), Error> {
        let yaml_doc = YamlDoc::try_from_string(raw_yaml, provenance)?;
        self.validate_yaml_doc(json, yaml_doc, provenance)
    }

    /// Validates `json` against the schema and attaches source spans from `yaml_doc`.
    fn validate_yaml_doc(
        &self,
        json: &Value,
        yaml_doc: YamlDoc,
        provenance: &str,
    ) -> Result<(), Error> {
        let mut errors = vec![];

        for error in self.validator.iter_errors(json) {
            let description = self
                .build_description_from_xpath(error.schema_path().as_str())
                .ok();
            let error_msg = self.build_error_message(&error, "".to_owned());
            errors.push(InvalidSemConvSpec(Box::new(InvalidSemConvSpecError {
                src: NamedSource::new(provenance, yaml_doc.raw_yaml()),
                err_span: yaml_doc.source_span(error.instance_path().as_str()).ok(),
                error: error_msg,
                advice: description,
            })));
        }

        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors
                .into_iter()
                .next()
                .expect("At least one error should be present"))
        } else {
            Err(CompoundError(errors))
        }
    }

    /// Formats sub-errors for each schema branch of a failed `anyOf` or `oneOf` validation.
    fn build_variant_error_message(
        &self,
        context: &[Vec<ValidationError<'_>>],
        indent: String,
    ) -> String {
        context
            .iter()
            .enumerate()
            .map(|(idx, errors)| {
                format!(
                    "{indent}(Variant {}):\n{}",
                    idx + 1,
                    errors
                        .iter()
                        .map(|e| format!(
                            "{indent}- {}",
                            self.build_error_message(e, format!("{indent}  "))
                        ))
                        .join("\n")
                )
            })
            .join("\n")
    }

    /// Formats a human-readable message for a single JSON Schema validation error.
    fn build_error_message(&self, error: &ValidationError<'_>, indent: String) -> String {
        match &error.kind() {
            ValidationErrorKind::AdditionalItems { limit } =>
                format!("Array has more items ({}) than allowed (maximum: {limit}).", error.instance()),

            ValidationErrorKind::AdditionalProperties { unexpected } =>
                format!(
                    "Object contains unexpected properties: {}. These properties are not defined in the schema.",
                    unexpected.join(", ")
                ),

            ValidationErrorKind::AnyOf { context } =>
                format!("The following YAML snippet does not match any of the allowed schemas.\n{}", self.build_variant_error_message(context, indent)),

            ValidationErrorKind::BacktrackLimitExceeded { error: e } =>
                format!("Regex match failed: backtrack limit exceeded ({e})"),

            ValidationErrorKind::Constant { expected_value } =>
                format!("Value {} does not match the required constant: {expected_value}.", error.instance()),

            ValidationErrorKind::Contains =>
                format!("Array {} does not contain any items matching the required schema.", error.instance()),

            ValidationErrorKind::ContentEncoding { content_encoding } =>
                format!("Value {} does not conform to the required contentEncoding: '{content_encoding}'.", error.instance()),

            ValidationErrorKind::ContentMediaType { content_media_type } =>
                format!("Value {} does not conform to the required contentMediaType: '{content_media_type}'.", error.instance()),

            ValidationErrorKind::Custom { message, keyword: _ } =>
                format!("Custom validation error: {message}"),

            ValidationErrorKind::Enum { options } =>
                format!("Value {} is not among the allowed options: {options}.", error.instance()),

            ValidationErrorKind::ExclusiveMaximum { limit } =>
                format!("Value {} must be less than {limit}.", error.instance()),

            ValidationErrorKind::ExclusiveMinimum { limit } =>
                format!("Value {} must be greater than {limit}.", error.instance()),

            ValidationErrorKind::FalseSchema =>
                format!("Schema is set to 'false'; all values are invalid (found {}).", error.instance()),

            ValidationErrorKind::Format { format } =>
                format!("Value {} does not match the required format: '{format}'.", error.instance()),

            ValidationErrorKind::FromUtf8 { error: e } =>
                format!("Base64 or UTF-8 decoding failed: {e}"),

            ValidationErrorKind::MaxItems { limit } =>
                format!("Array {} has too many items: {} (maximum allowed is {limit}).", error.instance(), error.instance()),

            ValidationErrorKind::Maximum { limit } =>
                format!("Value {} must be less than or equal to {limit}.", error.instance()),

            ValidationErrorKind::MaxLength { limit } =>
                format!(
                    "String {} is too long: exceeds the maximum length of {limit} character{}.",
                    error.instance(),
                    if *limit == 1 { "" } else { "s" }
                ),

            ValidationErrorKind::MaxProperties { limit } =>
                format!("Object {} has too many properties: exceeds the maximum of {limit} property(ies).", error.instance()),

            ValidationErrorKind::MinItems { limit } =>
                format!("Array {} has too few items: minimum required is {limit}.", error.instance()),

            ValidationErrorKind::Minimum { limit } =>
                format!("Value {} must be greater than or equal to {limit}.", error.instance()),

            ValidationErrorKind::MinLength { limit } =>
                format!(
                    "String {} is too short: must be at least {limit} character{}.",
                    error.instance(),
                    if *limit == 1 { "" } else { "s" }
                ),

            ValidationErrorKind::MinProperties { limit } =>
                format!("Object {} has too few properties: minimum required is {limit} property(ies).", error.instance()),

            ValidationErrorKind::MultipleOf { multiple_of } =>
                format!("Value {} is not a multiple of {multiple_of}.", error.instance()),

            ValidationErrorKind::Not { schema } =>
                format!("Value {} matches a schema that is explicitly forbidden (not). Schema: {schema}", error.instance()),

            ValidationErrorKind::OneOfMultipleValid { context } =>
                format!("Value {} matches more than one schema in a 'oneOf' group; it must match exactly one.\n{}", error.instance(), self.build_variant_error_message(context, indent)),

            ValidationErrorKind::OneOfNotValid { context } =>
                format!("Value {} does not match any schema in a 'oneOf' group; it must match exactly one.\n{}", error.instance(), self.build_variant_error_message(context, indent)),

            ValidationErrorKind::Pattern { pattern } =>
                format!("String {} does not match the required pattern: '{pattern}'.", error.instance()),

            ValidationErrorKind::PropertyNames { error: e } =>
                format!("Invalid object property name(s): {e}"),

            ValidationErrorKind::Required { property } =>
                format!("Missing required property: {property}."),

            ValidationErrorKind::Type { kind: TypeKind::Single(type_) } =>
                format!("Value {} is not of the required type: '{type_}'.", error.instance()),

            ValidationErrorKind::Type { kind: TypeKind::Multiple(types) } =>
                format!("Value {} is not of any of the required types. {}.", error.instance(), Self::describe_types(types)),

            ValidationErrorKind::UnevaluatedItems { unexpected } =>
                format!("Array contains unevaluated items that are not allowed: {}.", unexpected.join(", ")),

            ValidationErrorKind::UnevaluatedProperties { unexpected } =>
                format!("Object contains unevaluated properties that are not allowed: {}.", unexpected.join(", ")),

            ValidationErrorKind::UniqueItems =>
                format!("Array {} has duplicate (non-unique) items.", error.instance()),

            ValidationErrorKind::Referencing(e) =>
                format!("Error resolving $ref: {e}"),

            ValidationErrorKind::RegexEngineFailure { message } =>
                format!("Regex engine failure: {message}"),
        }
    }

    /// Returns the description corresponding to the JSON schema XPath.
    fn build_description_from_xpath(&self, xpath: &str) -> Result<String, Error> {
        let path_components = parse_xpath(xpath)?;
        let mut current = &self.json_schema;
        let mut description = "No description available".to_owned();

        for component in path_components {
            if let Some(desc) = current.get("description") {
                if let Some(desc) = desc.as_str() {
                    description = desc.to_owned();
                }
            }

            match component {
                PathComponent::Key(key) => {
                    if let Some(v) = current.get(&key) {
                        if key == "$ref" {
                            current = self.get_ref_definition(v.as_str().unwrap_or(""))?;
                        } else {
                            current = v;
                        }
                    } else {
                        return Err(InvalidXPath {
                            xpath: xpath.to_owned(),
                            error: format!("Key '{key}' not found in JSON schema"),
                        });
                    }
                }
                PathComponent::Index(index) => {
                    if let Some(v) = current.get(index) {
                        current = v;
                    } else {
                        return Err(InvalidXPath {
                            xpath: xpath.to_owned(),
                            error: format!("Index {index} out of bounds in JSON schema"),
                        });
                    }
                }
            }
        }

        Ok(description)
    }

    /// Retrieves the definition for a `$ref` path in the JSON schema.
    fn get_ref_definition(&self, ref_path: &str) -> Result<&Value, Error> {
        let parts: Vec<&str> = ref_path.split('/').collect();
        if parts.len() < 2 || parts[0] != "#" {
            return Err(InvalidXPath {
                xpath: ref_path.to_owned(),
                error: "Invalid $ref path".to_owned(),
            });
        }

        let mut current = &self.json_schema;
        for part in &parts[1..] {
            if let Some(value) = current.get(part) {
                current = value;
            } else {
                return Err(InvalidXPath {
                    xpath: ref_path.to_owned(),
                    error: format!("No such reference: {part}"),
                });
            }
        }
        Ok(current)
    }

    /// Formats the allowed JSON types in `types` as a human-readable list.
    fn describe_types(types: &JsonTypeSet) -> String {
        let mut type_names: Vec<String> = Vec::new();
        let mut has_null = false;

        for json_type in types.iter() {
            if json_type == JsonType::Null {
                has_null = true;
            } else {
                type_names.push(json_type.to_string());
            }
        }

        if has_null {
            type_names.push("null (i.e. optional)".to_owned());
        }

        if !type_names.is_empty() {
            format!("Supported types: {}", type_names.join(", "))
        } else {
            String::new()
        }
    }
}

/// Represents a position in the source file.
#[derive(Debug, Clone, PartialEq, Default)]
struct Position {
    /// Line number (1-based).
    line: usize,
    /// Column number (1-based).
    column: usize,
    char_pos: usize,
}

/// Represents a block's boundaries in the source.
#[derive(Debug, Clone, Default)]
struct BlockPosition {
    start: Position,
    end: Position,
}

/// Parsed YAML document with character-to-byte offset mapping for span lookup.
struct YamlDoc {
    raw_yaml: Cow<'static, str>,
    marked_yaml: MarkedYaml<'static>,
    /// The byte offsets of each character in the raw YAML string.
    /// For the n-th character, `char_offsets[n]` is its starting byte position.
    char_offsets: Vec<usize>,
}

impl YamlDoc {
    /// Loads and parses a YAML document from a file path.
    fn try_from_file(path_or_url: &str) -> Result<Self, Error> {
        let raw_yaml =
            fs::read_to_string(path_or_url).map_err(|e| Error::DeserializationError {
                path_or_url: path_or_url.to_owned(),
                error: e.to_string(),
            })?;

        Self::try_from_string(&raw_yaml, path_or_url)
    }

    /// Parses a single YAML document from `yaml` and indexes its character byte offsets.
    fn try_from_string(yaml: &str, path_or_url: &str) -> Result<Self, Error> {
        let raw_yaml: Cow<'static, str> = Cow::Owned(yaml.to_owned());

        let yaml_docs =
            MarkedYaml::load_from_str(yaml).map_err(|e| Error::DeserializationError {
                path_or_url: path_or_url.to_owned(),
                error: e.to_string(),
            })?;
        if yaml_docs.len() != 1 {
            return Err(Error::DeserializationError {
                path_or_url: path_or_url.to_owned(),
                error: "Expected exactly one YAML document".to_owned(),
            });
        }

        let mut char_offsets: Vec<usize> = yaml.char_indices().map(|(i, _)| i).collect();
        // Spans may reference the position after the last character.
        char_offsets.push(yaml.len());

        Ok(YamlDoc {
            raw_yaml,
            marked_yaml: yaml_docs
                .into_iter()
                .next()
                .expect("There should be exactly one YAML document"),
            char_offsets,
        })
    }

    /// Resolves a JSON Pointer-style XPath string to start and end positions in the YAML document.
    fn xpath_to_block_position(&self, xpath: &str) -> Result<BlockPosition, Error> {
        let path_components = parse_xpath(xpath)?;
        let mut current = &self.marked_yaml;

        for component in &path_components {
            match component {
                PathComponent::Key(key) => {
                    if let Some(value) = current.data.as_mapping_get(key) {
                        current = value;
                    } else {
                        return Err(InvalidXPath {
                            xpath: xpath.to_owned(),
                            error: format!("Key '{key}' not found in YAML document"),
                        });
                    }
                }
                PathComponent::Index(index) => {
                    if let Some(value) = current.data.as_sequence_get(*index) {
                        current = value;
                    } else {
                        return Err(InvalidXPath {
                            xpath: xpath.to_owned(),
                            error: format!("Index {index} out of bounds in YAML document"),
                        });
                    }
                }
            }
        }

        Ok(BlockPosition {
            start: Position {
                line: current.span.start.line(),
                column: current.span.start.col(),
                char_pos: current.span.start.index(),
            },
            end: Position {
                line: current.span.end.line(),
                column: current.span.end.col(),
                char_pos: current.span.end.index(),
            },
        })
    }

    /// Resolves an XPath string to a byte [`SourceSpan`] within the raw YAML source.
    fn source_span(&self, xpath: &str) -> Result<SourceSpan, Error> {
        let block_pos = self.xpath_to_block_position(xpath)?;
        Ok((
            self.char_offsets[block_pos.start.char_pos],
            self.char_offsets[block_pos.end.char_pos] - self.char_offsets[block_pos.start.char_pos],
        )
            .into())
    }

    /// Returns the raw YAML source string.
    fn raw_yaml(&self) -> Cow<'static, str> {
        self.raw_yaml.clone()
    }
}

/// Parses a `/`-delimited XPath of keys and array indices (e.g. `/groups/0/attributes/3`).
fn parse_xpath(xpath: &str) -> Result<Vec<PathComponent>, Error> {
    if !xpath.starts_with('/') {
        return Err(InvalidXPath {
            xpath: xpath.to_owned(),
            error: "XPath must start with '/'".to_owned(),
        });
    }

    Ok(xpath[1..]
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|component| {
            if let Ok(index) = component.parse::<usize>() {
                PathComponent::Index(index)
            } else {
                PathComponent::Key(component.to_owned())
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xpath_to_block_position() {
        let yaml = r##"groups:
  - name: long /
          name
    attributes:
      - attr1
      - attr2
      - attr3
  # Second group
  - name: group2
"##;
        let yaml = YamlDoc::try_from_string(yaml, "test.yaml").unwrap();

        let block_pos = yaml.xpath_to_block_position("/").unwrap();
        assert_eq!(block_pos.start.line, 1);
        assert_eq!(block_pos.end.line, 10);

        let block_pos = yaml.xpath_to_block_position("/groups").unwrap();
        assert_eq!(block_pos.start.line, 2);
        assert_eq!(block_pos.end.line, 10);

        let block_pos = yaml.xpath_to_block_position("/groups/0").unwrap();
        assert_eq!(block_pos.start.line, 2);
        assert_eq!(block_pos.end.line, 9);

        let block_pos = yaml.xpath_to_block_position("/groups/0/name").unwrap();
        assert_eq!(block_pos.start.line, 2);
        assert_eq!(block_pos.end.line, 3);

        let block_pos = yaml.xpath_to_block_position("/groups/1/name").unwrap();
        assert_eq!(block_pos.start.line, 9);
        assert_eq!(block_pos.end.line, 9);

        let block_pos = yaml
            .xpath_to_block_position("/groups/0/attributes")
            .unwrap();
        assert_eq!(block_pos.start.line, 5);
        assert_eq!(block_pos.end.line, 9);

        let block_pos = yaml
            .xpath_to_block_position("/groups/0/attributes/0")
            .unwrap();
        assert_eq!(block_pos.start.line, 5);
        assert_eq!(block_pos.end.line, 5);

        let block_pos = yaml
            .xpath_to_block_position("/groups/0/attributes/1")
            .unwrap();
        assert_eq!(block_pos.start.line, 6);
        assert_eq!(block_pos.end.line, 6);

        // Invalid XPath should return an error
        assert!(yaml
            .xpath_to_block_position("/groups/2/attributes/1")
            .is_err());
    }
}
