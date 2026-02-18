// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! This struct is used to specify the registry, including its name, version,
//! description, and few other details.
//!
//! In the future, this struct may be extended to include additional information
//! such as the registry's owner, maintainers, and dependencies.

use std::vec;

use crate::registry_repo::LEGACY_REGISTRY_MANIFEST;
use crate::schema_url::SchemaUrl;
use crate::stability::Stability;
use crate::Error;
use crate::Error::{
    DeprecatedSyntaxInRegistryManifest, InvalidRegistryManifest, LegacyRegistryManifest,
    RegistryManifestNotFound,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use weaver_common::vdir::VirtualDirectoryPath;

/// Represents the information of a semantic convention registry manifest.
///
/// This information defines the registry's name, version, description, and schema
/// base url.
#[derive(Serialize, Debug, Clone, JsonSchema)]
pub struct RegistryManifest {
    /// The file format for this registry.
    ///
    /// No value is assumed to be `manifest/2.0.0`
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file_format: Option<String>,

    /// The schema URL for this registry.
    /// This URL is populated before registry is published and is used as
    /// a unique identifier of the registry. It MUST follow OTel schema URL format, which is:
    /// `http[s]://server[:port]/path/<version>`.
    /// See <https://github.com/open-telemetry/opentelemetry-specification/blob/v1.53.0/specification/schemas/README.md#schema-url> for more details.
    pub schema_url: SchemaUrl,

    /// An optional description of the registry.
    ///
    /// This field can be used to provide additional context or information about the registry's
    /// purpose and contents.
    /// The format of the description is markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// List of the registry's dependencies.
    /// Note: In the current phase, we only support zero or one dependency.
    /// See this GH issue for more details: <https://github.com/open-telemetry/weaver/issues/604>
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<Dependency>,

    /// The stability of this repository.
    #[serde(default)]
    pub stability: Stability,

    /// The location of the resolved telemetry schema, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_schema_uri: Option<String>,

    #[serde(skip)]
    deserialization_warnings: Vec<String>,
}

/// Represents a dependency of a semantic convention registry.
#[derive(Serialize, Debug, Clone, JsonSchema)]
pub struct Dependency {
    /// The schema URL for the dependency (required).
    /// It must follow OTel schema URL format, which is: `http[s]://server[:port]/path/<version>`.
    /// This is not necessarily the URL registry can be accessed at, but it provides
    /// a unique identifier for the dependency registry and its version.
    ///
    /// When registry is not published yet, this field should be populated with a placeholder URL,
    /// but it must follow the URL format and include a version segment.
    /// The actual registry files can be provided in `registry_path` field.
    pub schema_url: SchemaUrl,

    /// The path to the dependency (optional).
    /// This can be either:
    /// - A manifest of a published registry
    /// - A directory containing the raw definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_path: Option<VirtualDirectoryPath>,
}

impl<'de> Deserialize<'de> for Dependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DependencyHelper {
            name: Option<String>,
            schema_url: Option<SchemaUrl>,
            registry_path: Option<VirtualDirectoryPath>,
        }

        let helper = DependencyHelper::deserialize(deserializer)?;

        let schema_url = match (helper.schema_url, helper.name) {
            (Some(url), _) => url,
            (None, Some(name)) => SchemaUrl::try_from_name_version(&name, "unknown")
                .map_err(serde::de::Error::custom)?,
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "Either 'schema_url' or 'name' must be provided for a dependency",
                ))
            }
        };

        Ok(Dependency {
            schema_url,
            registry_path: helper.registry_path,
        })
    }
}

impl<'de> Deserialize<'de> for RegistryManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RegistryManifestHelper {
            file_format: Option<String>,
            schema_url: Option<SchemaUrl>,
            description: Option<String>,
            #[allow(deprecated)]
            semconv_version: Option<String>,
            #[allow(deprecated)]
            schema_base_url: Option<String>,
            #[serde(default)]
            dependencies: Vec<Dependency>,
            #[serde(default)]
            stability: Stability,
            resolved_schema_uri: Option<String>,
        }

        let helper = RegistryManifestHelper::deserialize(deserializer)?;
        let mut warnings = vec![];

        let schema_url = if let Some(url) = helper.schema_url {
            url
        } else {
            // Fall back to deprecated fields
            let base_url = helper.schema_base_url.as_ref().ok_or_else(|| {
               serde::de::Error::custom(
                   "Either 'schema_url' or both 'schema_base_url' and 'semconv_version' must be provided",
               )
            })?;

            let version = helper.semconv_version.as_ref().ok_or_else(|| {
                serde::de::Error::custom(
                    "Either 'schema_url' or both 'schema_base_url' and 'semconv_version' must be provided",
                )
            })?;

            warnings.push("The 'semconv_version' and 'schema_base_url' fields are deprecated in favor of 'schema_url'.".to_owned());
            SchemaUrl::try_from_name_version(base_url, version).map_err(serde::de::Error::custom)?
        };

        Ok(RegistryManifest {
            file_format: helper.file_format,
            schema_url,
            description: helper.description,
            dependencies: helper.dependencies,
            stability: helper.stability,
            resolved_schema_uri: helper.resolved_schema_uri,
            deserialization_warnings: warnings,
        })
    }
}

impl RegistryManifest {
    /// Attempts to load a registry manifest from a file.
    ///
    /// The expected file format is YAML.
    pub fn try_from_file<P: AsRef<std::path::Path>>(
        path: P,
        nfes: &mut Vec<Error>,
    ) -> Result<Self, Error> {
        let manifest_path_buf = path.as_ref().to_path_buf();

        if !manifest_path_buf.exists() {
            return Err(RegistryManifestNotFound {
                path: manifest_path_buf.clone(),
            });
        }

        let file = std::fs::File::open(path).map_err(|e| InvalidRegistryManifest {
            path: manifest_path_buf.clone(),
            error: e.to_string(),
        })?;
        let reader = std::io::BufReader::new(file);
        let manifest: RegistryManifest =
            serde_yaml::from_reader(reader).map_err(|e| InvalidRegistryManifest {
                path: manifest_path_buf.clone(),
                error: e.to_string(),
            })?;

        // Check if this is a legacy manifest file
        let is_legacy = if let Some(file_name) = manifest_path_buf.file_name() {
            file_name == LEGACY_REGISTRY_MANIFEST
        } else {
            false
        };

        if is_legacy {
            nfes.push(LegacyRegistryManifest {
                path: manifest_path_buf.clone(),
            });
        }

        nfes.extend(manifest.deserialization_warnings.iter().map(|w| {
            DeprecatedSyntaxInRegistryManifest {
                path: manifest_path_buf.clone(),
                error: w.clone(),
            }
        }));
        Ok(manifest)
    }

    /// Returns the registry name, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry name would be `opentelemetry.io/schemas/sub-component`
    #[must_use]
    pub fn name(&self) -> &str {
        self.schema_url.name()
    }

    /// Returns the registry version, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry version would be `1.0.0`
    #[must_use]
    pub fn version(&self) -> &str {
        self.schema_url.version()
    }

    /// Creates a new `RegistryManifest` from a schema URL with default values.
    #[must_use]
    pub fn from_schema_url(schema_url: SchemaUrl) -> Self {
        Self {
            file_format: None,
            schema_url,
            description: None,
            dependencies: vec![],
            resolved_schema_uri: None,
            stability: Stability::Development,
            deserialization_warnings: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_not_found_registry_info() {
        let result =
            RegistryManifest::try_from_file("tests/test_data/missing_registry.yaml", &mut vec![]);
        assert!(
            matches!(result, Err(RegistryManifestNotFound { path, .. }) if path.ends_with("missing_registry.yaml"))
        );
    }

    #[test]
    fn test_incomplete_registry_info() {
        let result = RegistryManifest::try_from_file(
            "tests/test_data/incomplete_semconv_registry_manifest.yaml",
            &mut vec![],
        );
        assert!(
            matches!(result, Err(InvalidRegistryManifest { path, .. }) if path.ends_with("incomplete_semconv_registry_manifest.yaml"))
        );
    }

    #[test]
    fn test_valid_registry_info() {
        let config = RegistryManifest::try_from_file(
            "tests/test_data/valid_semconv_registry_manifest.yaml",
            &mut vec![],
        )
        .expect("Failed to load the registry configuration file.");
        assert_eq!(config.name(), "acme.com/schemas");
        assert_eq!(config.version(), "0.1.0");
    }

    #[test]
    fn test_invalid_registry_info() {
        let result = RegistryManifest::try_from_file(
            "tests/test_data/invalid_semconv_registry_manifest.yaml",
            &mut vec![],
        );
        let path = PathBuf::from("tests/test_data/invalid_semconv_registry_manifest.yaml");

        let expected_errs = InvalidRegistryManifest {
            path: path.clone(),
            error: "Registry name and version cannot be empty.".to_owned(),
        };

        if let Err(observed_errs) = result {
            assert_eq!(observed_errs, expected_errs);
        } else {
            panic!("Expected an error, but got a result.");
        }
    }

    // Dependency tests
    #[test]
    fn test_dependency_deserialize_with_schema_url() {
        let yaml = r#"
schema_url: "https://opentelemetry.io/schemas/1.0.0"
"#;
        let dep: Dependency = serde_yaml::from_str(yaml).expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
        assert!(dep.registry_path.is_none());
    }

    #[test]
    fn test_dependency_deserialize_with_registry_path() {
        let yaml = r#"
schema_url: "https://opentelemetry.io/schemas/1.0.0"
registry_path: "./registry"
"#;
        let dep: Dependency = serde_yaml::from_str(yaml).expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
        assert!(dep.registry_path.is_some());
    }

    #[test]
    fn test_dependency_deserialize_with_deprecated_name() {
        let yaml = r#"
name: "acme-registry"
"#;
        let dep: Dependency = serde_yaml::from_str(yaml).expect("Failed to deserialize");
        assert_eq!(dep.schema_url.as_str(), "https://acme-registry/unknown");
    }

    #[test]
    fn test_dependency_deserialize_schema_url_takes_precedence() {
        let yaml = r#"
schema_url: "https://opentelemetry.io/schemas/1.0.0"
name: "ignored-name"
"#;
        let dep: Dependency = serde_yaml::from_str(yaml).expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
    }

    #[test]
    fn test_dependency_deserialize_missing_both_fields() {
        let yaml = r#"
registry_path: "./registry"
"#;
        let result: Result<Dependency, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err
            .to_string()
            .contains("Either 'schema_url' or 'name' must be provided"));
    }

    #[test]
    fn test_dependency_serialize() {
        let dep = Dependency {
            schema_url: "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap(),
            registry_path: None,
        };

        let yaml = serde_yaml::to_string(&dep).expect("Failed to serialize");
        // Verify schema_url is serialized
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("https://opentelemetry.io/schemas/1.0.0"));
        // Verify name is NOT serialized (skip_serializing)
        assert!(!yaml.contains("name:"));
    }

    #[test]
    fn test_dependency_serialize_with_registry_path() {
        let dep = Dependency {
            schema_url: "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap(),
            registry_path: Some(VirtualDirectoryPath::LocalFolder {
                path: "./registry".to_owned(),
            }),
        };

        let yaml = serde_yaml::to_string(&dep).expect("Failed to serialize");
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_serialize_without_optional_path() {
        let dep = Dependency {
            schema_url: "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap(),
            registry_path: None,
        };

        let yaml = serde_yaml::to_string(&dep).expect("Failed to serialize");
        // registry_path should not be serialized when None (skip_serializing_if)
        assert!(!yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_roundtrip_serialization() {
        let original = Dependency {
            schema_url: "https://example.com/schemas/1.0.0".try_into().unwrap(),
            registry_path: Some(VirtualDirectoryPath::LocalFolder {
                path: "./test/registry".to_owned(),
            }),
        };

        let yaml = serde_yaml::to_string(&original).expect("Failed to serialize");
        let deserialized: Dependency = serde_yaml::from_str(&yaml).expect("Failed to deserialize");

        assert_eq!(original.schema_url, deserialized.schema_url);
        assert!(deserialized.registry_path.is_some());
    }
}
