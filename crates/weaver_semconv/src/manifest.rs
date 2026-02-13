// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! This struct is used to specify the registry, including its name, version,
//! description, and few other details.
//!
//! In the future, this struct may be extended to include additional information
//! such as the registry's owner, maintainers, and dependencies.

use crate::stability::Stability;
use crate::Error;
use crate::Error::{InvalidRegistryManifest, RegistryManifestNotFound};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
    pub schema_url: Option<String>,

    /// An optional description of the registry.
    ///
    /// This field can be used to provide additional context or information about the registry's
    /// purpose and contents.
    /// The format of the description is markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The version of the registry which will be used to define the semconv package version.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[deprecated(
        note = "The `version` field is deprecated. The registry version should be specified in the `schema_url` field, which is required and serves as a unique identifier for the registry."
    )]
    pub semconv_version: Option<String>,

    /// The base URL where the registry's schema files are hosted.
    #[serde(skip_serializing_if = "Option::is_none", default)]
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
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Dependency {
    /// The name of the dependency.
    pub name: String,
    /// The path to the dependency.
    ///
    /// This can be either:
    /// - A manifest of a published registry
    /// - A directory containing the raw definition.
    pub registry_path: VirtualDirectoryPath,
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
            manifest.schema_url = Some(format!(
                "{}/{}",
                manifest.schema_base_url.clone().unwrap_or_default(),
                manifest.semconv_version.clone().unwrap_or_default()
            ));
        }

        Ok(manifest)
    }

    fn validate(&self, path: PathBuf) -> Result<(), Error> {
        let mut errors = vec![];

        let schema_url_empty = self.schema_url.as_ref().map_or(true, |url| url.is_empty());
        let schema_base_url_empty = self.schema_base_url.as_ref().map_or(true, |url| url.is_empty());
        let semconv_version_empty = self.semconv_version.as_ref().map_or(true, |v| v.is_empty());

        if schema_url_empty {
            if schema_base_url_empty || semconv_version_empty {
                errors.push(InvalidRegistryManifest {
                    path: path.clone(),
                    error: "The registry schema URL is required.".to_owned(),
                });
            } else {
                // schema_base_url should be a valid absolute URL, otherwise push an error to the list.
                if let Err(e) = url::Url::parse(self.schema_base_url.as_ref().unwrap()) {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: format!("Invalid schema base URL: {}", e),
                    });
                }
            }
        } else {
            // validate the resolved schema URL: it must be a valid absolute URI with at least one path segment
            match url::Url::parse(self.schema_url.as_ref().unwrap()) {
                Ok(parsed_url) => {
                    if parsed_url.path_segments().map(|c| c.count()).unwrap_or(0) == 0 {
                        errors.push(InvalidRegistryManifest {
                            path: path.clone(),
                            error: "The registry schema URL must have at least one path segment.".to_owned(),
                        });
                    }
                }
                Err(e) => {
                    errors.push(InvalidRegistryManifest {
                        path: path.clone(),
                        error: format!("Invalid schema URL: {}", e),
                    });
                }
            }
        }

        handle_errors(errors)?;
        Ok(())
    }

    /// Returns the registry name, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry name would be `opentelemetry.io/schemas/sub-component`
    pub fn name(&self) -> String {
        let schema_url = self.schema_url.as_ref().expect("schema_url was validated");
        let parsed_url = url::Url::parse(schema_url).expect("schema_url was validated");
        let authority = parsed_url.host_str().unwrap_or_default();
        let path = parsed_url.path().trim_matches('/');
        let mut segments: Vec<&str> = path.split('/').collect();
        if !segments.is_empty() {
            _ = segments.pop();
        }
        format!("{}/{}", authority, segments.join("/"))
    }

    /// Returns the registry version, which is derived from the schema URL.
    /// For example, if the schema URL is `https://opentelemetry.io/schemas/sub-component/1.0.0`,
    /// the registry version would be `1.0.0`
    pub fn version(&self) -> String {
        let schema_url = self.schema_url.as_ref().expect("schema_url was validated");
        let parsed_url = url::Url::parse(schema_url).expect("schema_url was validated");
        parsed_url
            .path()
            .trim_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string()
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
        assert_eq!(config.name(), "vendor_acme");
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
                error: "The registry name is required.".to_owned(),
            },
            InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry version is required.".to_owned(),
            },
            InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry schema base URL is required.".to_owned(),
            },
        ]);

        if let Err(observed_errs) = result {
            assert_eq!(observed_errs, expected_errs);
        } else {
            panic!("Expected an error, but got a result.");
        }
    }
}
