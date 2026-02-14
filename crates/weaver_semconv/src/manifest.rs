// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! This struct is used to specify the registry, including its name, version,
//! description, and few other details.
//!
//! In the future, this struct may be extended to include additional information
//! such as the registry's owner, maintainers, and dependencies.

use crate::schema_url::SchemaUrl;
use crate::stability::Stability;
use crate::Error;
use crate::Error::{InvalidRegistryManifest, RegistryManifestNotFound};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use weaver_common::error::handle_errors;
use weaver_common::vdir::VirtualDirectoryPath;

/// Represents the information of a semantic convention registry manifest.
///
/// This information defines the registry's name, version, description, and schema
/// base url.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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
    /// See https://github.com/open-telemetry/opentelemetry-specification/blob/v1.53.0/specification/schemas/README.md#schema-url for more details.
    pub schema_url: Option<SchemaUrl>,

    /// An optional description of the registry.
    ///
    /// This field can be used to provide additional context or information about the registry's
    /// purpose and contents.
    /// The format of the description is markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The version of the registry which will be used to define the semconv package version.
    #[serde(default, skip_serializing)]
    #[deprecated(
        note = "The `version` field is deprecated. The registry version should be specified in the `schema_url` field, which is required and serves as a unique identifier for the registry."
    )]
    pub semconv_version: Option<String>,

    /// The base URL where the registry's schema files are hosted.
    #[serde(default, skip_serializing)]
    #[deprecated(
        note = "The `schema_base_url` field is deprecated. The registry schema URL should be specified in the `schema_url` field, which is required and serves as a unique identifier for the registry."
    )]
    pub schema_base_url: Option<String>,

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

    /// This field is deprecated and should not be used.
    /// The registry name should be derived from the `schema_url` field,
    /// which serves as a unique identifier for the dependency registry
    /// and includes registry version.
    #[deprecated(
        note = "The `name` field is deprecated. The registry name should be derived from the `schema_url` field, which serves as a unique identifier for the dependency registry."
    )]
    #[serde(default, skip_serializing)] // we can read, but won't write this field
    pub name: Option<String>,
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
            (None, Some(name)) => SchemaUrl::new(format!("{}/unknown", name)),
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "Either 'schema_url' or 'name' must be provided for a dependency",
                ))
            }
        };

        Ok(Dependency {
            schema_url,
            registry_path: helper.registry_path,
            name: None,
        })
    }
}

impl RegistryManifest {
    /// Attempts to load a registry manifest from a file.
    ///
    /// The expected file format is YAML.
    pub fn try_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
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
        let mut manifest: RegistryManifest =
            serde_yaml::from_reader(reader).map_err(|e| InvalidRegistryManifest {
                path: manifest_path_buf.clone(),
                error: e.to_string(),
            })?;

        manifest.validate(manifest_path_buf.clone())?;

        // If the schema URL is not provided, populate it using deprecated schema_base_url and semconv_version
        // validation would fail if they were not provided
        if manifest.schema_url.is_none() {
            manifest.schema_url = Some(
                SchemaUrl::from_name_version(
                    &manifest.schema_base_url.clone().unwrap_or_default(),
                    &manifest.semconv_version.clone().unwrap_or_default(),
                )
                .map_err(|e| InvalidRegistryManifest {
                    path: manifest_path_buf.clone(),
                    error: e,
                })?,
            );
        }

        Ok(manifest)
    }

    fn validate(&self, path: PathBuf) -> Result<(), Error> {
        let mut errors = vec![];

        if self.schema_url.is_none() {
            if self.schema_base_url.is_none() || self.semconv_version.is_none() {
                errors.push(InvalidRegistryManifest {
                    path: path.clone(),
                    error: "The registry schema URL is required.".to_owned(),
                });
            } else {
                if self
                    .schema_base_url
                    .as_ref()
                    .map_or(true, |url| url.is_empty())
                {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: "The registry schema base URL is required.".to_owned(),
                    });
                } else if let Err(e) =
                    url::Url::parse(&self.schema_base_url.clone().unwrap_or_default())
                {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: format!("Invalid schema base URL: {}", e),
                    });
                }

                if self
                    .semconv_version
                    .as_ref()
                    .map_or(true, |version| version.is_empty())
                {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: "The registry version is required.".to_owned(),
                    });
                }
            }
        } else {
            // validate the resolved schema URL: it must be a valid absolute URI with at least one path segment
            if let Some(url) = self.schema_url.as_ref() {
                url.validate().unwrap_or_else(|e| {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: format!("Invalid schema URL: {}", e),
                    });
                });
            }
        }

        handle_errors(errors)?;
        Ok(())
    }

    /// Returns the registry name, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry name would be `opentelemetry.io/schemas/sub-component`
    #[must_use]
    pub fn name(&self) -> String {
        self.schema_url
            .as_ref()
            .map(|url| url.name().to_owned())
            .unwrap_or_default()
    }

    /// Returns the registry version, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry version would be `1.0.0`
    #[must_use]
    pub fn version(&self) -> String {
        self.schema_url
            .as_ref()
            .map(|url| url.version().to_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error::CompoundError;

    #[test]
    fn test_not_found_registry_info() {
        let result = RegistryManifest::try_from_file("tests/test_data/missing_registry.yaml");
        assert!(
            matches!(result, Err(RegistryManifestNotFound { path, .. }) if path.ends_with("missing_registry.yaml"))
        );
    }

    #[test]
    fn test_incomplete_registry_info() {
        let result = RegistryManifest::try_from_file(
            "tests/test_data/incomplete_semconv_registry_manifest.yaml",
        );
        assert!(
            matches!(result, Err(InvalidRegistryManifest { path, .. }) if path.ends_with("incomplete_semconv_registry_manifest.yaml"))
        );
    }

    #[test]
    fn test_valid_registry_info() {
        let config =
            RegistryManifest::try_from_file("tests/test_data/valid_semconv_registry_manifest.yaml")
                .expect("Failed to load the registry configuration file.");
        assert_eq!(config.name(), "acme.com/schemas");
        assert_eq!(config.version(), "0.1.0");
    }

    #[test]
    fn test_invalid_registry_info() {
        let result = RegistryManifest::try_from_file(
            "tests/test_data/invalid_semconv_registry_manifest.yaml",
        );
        let path = PathBuf::from("tests/test_data/invalid_semconv_registry_manifest.yaml");

        let expected_errs = CompoundError(vec![
            InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry schema base URL is required.".to_owned(),
            },
            InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry version is required.".to_owned(),
            },
        ]);

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
        assert_eq!(dep.schema_url.as_str(), "acme-registry/unknown");
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
            schema_url: SchemaUrl::new("https://opentelemetry.io/schemas/1.0.0".to_owned()),
            registry_path: None,
            name: None,
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
            schema_url: SchemaUrl::new("https://opentelemetry.io/schemas/1.0.0".to_owned()),
            registry_path: Some(VirtualDirectoryPath::LocalFolder {
                path: "./registry".to_owned(),
            }),
            name: None,
        };

        let yaml = serde_yaml::to_string(&dep).expect("Failed to serialize");
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_serialize_without_optional_path() {
        let dep = Dependency {
            schema_url: SchemaUrl::new("https://opentelemetry.io/schemas/1.0.0".to_owned()),
            registry_path: None,
            name: None,
        };

        let yaml = serde_yaml::to_string(&dep).expect("Failed to serialize");
        // registry_path should not be serialized when None (skip_serializing_if)
        assert!(!yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_roundtrip_serialization() {
        let original = Dependency {
            schema_url: SchemaUrl::new("https://example.com/schemas/1.0.0".to_owned()),
            registry_path: Some(VirtualDirectoryPath::LocalFolder {
                path: "./test/registry".to_owned(),
            }),
            name: None,
        };

        let yaml = serde_yaml::to_string(&original).expect("Failed to serialize");
        let deserialized: Dependency = serde_yaml::from_str(&yaml).expect("Failed to deserialize");

        assert_eq!(original.schema_url, deserialized.schema_url);
        assert!(deserialized.registry_path.is_some());
    }
}
