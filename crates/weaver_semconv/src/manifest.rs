// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! This struct is used to specify the registry, including its name, version,
//! description, and few other details.
//!
//! In the future, this struct may be extended to include additional information
//! such as the registry's owner, maintainers, and dependencies.

use crate::Error;
use crate::Error::{InvalidRegistryManifest, RegistryManifestNotFound};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use weaver_common::error::handle_errors;

/// Represents the information of a semantic convention registry manifest.
///
/// This information defines the registry's name, version, description, and schema
/// base url.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegistryManifest {
    /// The name of the registry. This name is used to define the package name.
    pub name: String,

    /// An optional description of the registry.
    ///
    /// This field can be used to provide additional context or information about the registry's
    /// purpose and contents.
    /// The format of the description is markdown.
    pub description: Option<String>,

    /// The version of the registry which will be used to define the semconv package version.
    pub semconv_version: String,

    /// The base URL where the registry's schema files are hosted.
    pub schema_base_url: String,
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
        let manifest: RegistryManifest =
            serde_yaml::from_reader(reader).map_err(|e| InvalidRegistryManifest {
                path: manifest_path_buf.clone(),
                error: e.to_string(),
            })?;

        manifest.validate(manifest_path_buf.clone())?;

        Ok(manifest)
    }

    fn validate(&self, path: PathBuf) -> Result<(), Error> {
        let mut errors = vec![];

        if self.name.is_empty() {
            errors.push(InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry name is required.".to_owned(),
            });
        }

        if self.semconv_version.is_empty() {
            errors.push(InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry version is required.".to_owned(),
            });
        }

        if self.schema_base_url.is_empty() {
            errors.push(InvalidRegistryManifest {
                path: path.clone(),
                error: "The registry schema base URL is required.".to_owned(),
            });
        }

        handle_errors(errors)?;

        Ok(())
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
        assert_eq!(config.name, "vendor_acme");
        assert_eq!(config.semconv_version, "0.1.0");
        assert_eq!(config.schema_base_url, "https://acme.com/schemas/");
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
