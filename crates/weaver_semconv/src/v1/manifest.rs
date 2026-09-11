// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the version 1 semantic conventions registry manifest.
//!
//! Defined here:
//! - [`DefinitionRegistryManifest`]: the definition manifest for an unpublished registry (v1)
//! - [`Dependency`]: a dependency of a semantic convention registry (v1)
//! - [`RegistryManifest`]: an enum containing the definition manifest

use std::vec;

#[allow(deprecated)]
use crate::registry_repo::LEGACY_REGISTRY_MANIFEST;
use crate::schema_url::SchemaUrl;
use crate::v1::stability::Stability;
use crate::Error;
use crate::Error::{
    DeprecatedSyntaxInRegistryManifest, InvalidRegistryManifest, LegacyRegistryManifest,
    RegistryManifestNotFound,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_common::vdir::VirtualDirectoryPath;

/// Represents the definition manifest for a semantic convention registry (version 1).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct DefinitionRegistryManifest {
    /// The schema URL for this registry.
    pub schema_url: SchemaUrl,

    /// An optional description of the registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// List of the registry's dependencies.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<Dependency>,

    /// The stability of this repository.
    #[serde(default)]
    pub stability: Stability,

    #[serde(skip)]
    pub(crate) deserialization_warnings: Vec<String>,
}

impl DefinitionRegistryManifest {
    /// Returns the registry name, which is derived from the schema URL.
    #[must_use]
    pub fn name(&self) -> &str {
        self.schema_url.name()
    }

    /// Returns the registry version, which is derived from the schema URL.
    #[must_use]
    pub fn version(&self) -> &str {
        self.schema_url.version()
    }

    /// Creates a new `DefinitionRegistryManifest` from a schema URL with default values.
    #[must_use]
    pub fn from_schema_url(schema_url: SchemaUrl) -> Self {
        Self {
            schema_url,
            description: None,
            dependencies: vec![],
            stability: Stability::Development,
            deserialization_warnings: vec![],
        }
    }
}

/// Represents a dependency of a semantic convention registry (version 1).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Dependency {
    /// The schema URL for the dependency (required).
    pub schema_url: SchemaUrl,

    /// The path to the dependency (optional).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registry_path: Option<VirtualDirectoryPath>,
}

impl Dependency {
    /// Whether the declaration pins a version. A dependency declared by `name` (v1 manifests
    /// only) does not: it names a registry without saying which version of it, so it cannot be
    /// reconciled with any other declaration of the same registry.
    #[must_use]
    pub fn is_versioned(&self) -> bool {
        self.schema_url.version() != UNKNOWN_VERSION
    }
}

const SCHEMA_URL_HELP: &str = "The schema_url uniquely identifies the dependency registry \
                               and its version, e.g. https://example.com/my-registry/1.0.0";

/// The version given to a dependency declared by `name`, which carries none.
const UNKNOWN_VERSION: &str = "unknown";

/// Parses a dependency declaration, discriminating on which identifying field it declares.
/// A v1 manifest may declare a dependency by `name`, which then gets a placeholder, unversioned schema URL minted
/// from that name.
fn parse_dependency(value: serde_yaml::Value) -> Result<Dependency, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NamedDependency {
        name: String,
        registry_path: VirtualDirectoryPath,
    }

    if value.get("schema_url").is_some() {
        return serde_yaml::from_value(value).map_err(|e| e.to_string());
    }
    let name = value.get("name").and_then(serde_yaml::Value::as_str);
    if name.is_some() {
        let named: NamedDependency = serde_yaml::from_value(value).map_err(|e| e.to_string())?;
        Ok(Dependency {
            schema_url: SchemaUrl::try_from_name_version(&named.name, UNKNOWN_VERSION)?,
            registry_path: Some(named.registry_path),
        })
    } else {
        let subject = name.map_or_else(
            || "a dependency".to_owned(),
            |name| format!("dependency '{name}'"),
        );
        Err(format!(
            "{subject} is missing the required field 'schema_url'. {SCHEMA_URL_HELP}"
        ))
    }
}

/// Raw helper for deserializing a manifest before validation.
#[derive(Deserialize)]
struct RawManifestFields {
    file_format: Option<String>,
    schema_url: Option<SchemaUrl>,
    description: Option<String>,
    #[allow(deprecated)]
    semconv_version: Option<String>,
    #[allow(deprecated)]
    schema_base_url: Option<String>,
    #[serde(default)]
    dependencies: Vec<serde_yaml::Value>,
    #[serde(default)]
    stability: Stability,
}

impl RawManifestFields {
    fn into_manifest(self, path: &std::path::Path) -> Result<RegistryManifest, Error> {
        let mut warnings = vec![];
        if let Some(ref fmt) = self.file_format {
            return Err(InvalidRegistryManifest {
                path: path.to_path_buf(),
                error: format!(
                    "Unknown file_format '{fmt}'. Expected no file_format for a definition manifest."
                ),
            });
        }
        let schema_url = if let Some(url) = self.schema_url {
            url
        } else {
            let base_url =
                self.schema_base_url.as_ref().ok_or_else(|| InvalidRegistryManifest {
                    path: path.to_path_buf(),
                    error: "Either 'schema_url' or both 'schema_base_url' and 'semconv_version' must be provided".into(),
                })?;
            let version =
                self.semconv_version.as_ref().ok_or_else(|| InvalidRegistryManifest {
                    path: path.to_path_buf(),
                    error: "Either 'schema_url' or both 'schema_base_url' and 'semconv_version' must be provided".into(),
                })?;
            warnings.push(
                "The 'semconv_version' and 'schema_base_url' fields are deprecated in favor of 'schema_url'."
                    .to_owned(),
            );
            SchemaUrl::try_from_name_version(base_url, version).map_err(|e| {
                InvalidRegistryManifest {
                    path: path.to_path_buf(),
                    error: e,
                }
            })?
        };
        let dependencies = self
            .dependencies
            .into_iter()
            .map(parse_dependency)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| InvalidRegistryManifest {
                path: path.to_path_buf(),
                error,
            })?;
        Ok(RegistryManifest::Definition(DefinitionRegistryManifest {
            schema_url,
            description: self.description,
            dependencies,
            stability: self.stability,
            deserialization_warnings: warnings,
        }))
    }
}

/// A registry manifest for version 1 semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum RegistryManifest {
    /// A definition manifest (used when developing a registry).
    Definition(DefinitionRegistryManifest),
}

impl RegistryManifest {
    /// Attempts to load a registry manifest from a file.
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
        let raw: RawManifestFields =
            serde_yaml::from_reader(reader).map_err(|e| InvalidRegistryManifest {
                path: manifest_path_buf.clone(),
                error: e.to_string(),
            })?;
        let manifest = raw.into_manifest(&manifest_path_buf)?;

        // Check if this is a legacy manifest file
        let is_legacy = if let Some(file_name) = manifest_path_buf.file_name() {
            #[allow(deprecated)]
            let legacy = file_name == LEGACY_REGISTRY_MANIFEST;
            legacy
        } else {
            false
        };

        if is_legacy {
            nfes.push(LegacyRegistryManifest {
                path: manifest_path_buf.clone(),
            });
        }

        let deserialization_warnings = match &manifest {
            RegistryManifest::Definition(def) => def.deserialization_warnings.as_slice(),
        };
        nfes.extend(
            deserialization_warnings
                .iter()
                .map(|w| DeprecatedSyntaxInRegistryManifest {
                    path: manifest_path_buf.clone(),
                    error: w.clone(),
                }),
        );

        Ok(manifest)
    }

    /// Returns the schema URL of the registry.
    #[must_use]
    pub fn schema_url(&self) -> &SchemaUrl {
        match self {
            RegistryManifest::Definition(m) => &m.schema_url,
        }
    }

    /// Returns the registry name, which is derived from the schema URL.
    #[must_use]
    pub fn name(&self) -> &str {
        self.schema_url().name()
    }

    /// Returns the registry version, which is derived from the schema URL.
    #[must_use]
    pub fn version(&self) -> &str {
        self.schema_url().version()
    }

    /// Returns the dependencies of the registry.
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        match self {
            RegistryManifest::Definition(m) => &m.dependencies,
        }
    }

    /// Returns the description of the registry, if present.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            RegistryManifest::Definition(m) => m.description.as_deref(),
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

    fn dep_from_yaml_lenient(yaml: &str) -> Result<Dependency, String> {
        parse_dependency(serde_yaml::from_str(yaml).expect("invalid YAML"))
    }

    fn dep(schema_url: &str, registry_path: Option<&str>) -> Dependency {
        Dependency {
            schema_url: schema_url.try_into().unwrap(),
            registry_path: registry_path.map(|path| VirtualDirectoryPath::LocalFolder {
                path: path.to_owned(),
            }),
        }
    }

    #[test]
    fn test_dependency_deserialize_with_schema_url() {
        let dep = dep_from_yaml_lenient(r#"schema_url: "https://opentelemetry.io/schemas/1.0.0""#)
            .expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
        assert!(dep.registry_path.is_none());
        assert!(dep.is_versioned());
    }

    #[test]
    fn test_dependency_deserialize_with_registry_path() {
        let dep = dep_from_yaml_lenient(
            r#"
schema_url: "https://opentelemetry.io/schemas/1.0.0"
registry_path: "./registry"
"#,
        )
        .expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
        assert!(dep.registry_path.is_some());
    }

    #[test]
    fn test_v1_dependency_accepts_name_and_registry_path() {
        let dep = dep_from_yaml_lenient(
            r#"
name: "acme-registry"
registry_path: "./registry"
"#,
        )
        .expect("v1 manifests must keep supporting dependencies declared by name");
        assert_eq!(dep.schema_url.as_str(), "https://acme-registry/unknown");
        assert!(
            !dep.is_versioned(),
            "a dependency declared without 'schema_url' carries no version"
        );
    }

    #[test]
    fn test_non_semver_schema_url_is_versioned() {
        let dep = dep_from_yaml_lenient(r#"schema_url: "https://example.com/dep/1.0""#)
            .expect("Failed to deserialize");
        assert!(dep.schema_url.semver().is_err());
        assert!(dep.is_versioned());
    }

    #[test]
    fn test_v1_dependency_name_without_registry_path_is_rejected() {
        let err = dep_from_yaml_lenient(r#"name: "acme-registry""#)
            .expect_err("a dependency with no identity and no path cannot be located");
        assert!(
            err.contains("registry_path"),
            "error should report the missing 'registry_path'; got: {err}"
        );
    }

    #[test]
    fn test_dependency_deserialize_schema_url_takes_precedence() {
        let dep = dep_from_yaml_lenient(
            r#"
schema_url: "https://opentelemetry.io/schemas/1.0.0"
name: "ignored-name"
"#,
        )
        .expect("Failed to deserialize");
        assert_eq!(
            dep.schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
    }

    #[test]
    fn test_dependency_deserialize_missing_both_fields() {
        let err = dep_from_yaml_lenient(r#"registry_path: "./registry""#)
            .expect_err("a dependency with no identity must be rejected");
        assert!(err.contains("schema_url"), "got: {err}");
    }

    #[test]
    fn test_dependency_serialize() {
        let yaml = serde_yaml::to_string(&dep("https://opentelemetry.io/schemas/1.0.0", None))
            .expect("Failed to serialize");
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("https://opentelemetry.io/schemas/1.0.0"));
        assert!(!yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_serialize_with_registry_path() {
        let yaml = serde_yaml::to_string(&dep(
            "https://opentelemetry.io/schemas/1.0.0",
            Some("./registry"),
        ))
        .expect("Failed to serialize");
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("registry_path"));
    }

    #[test]
    fn test_dependency_roundtrip_serialization() {
        let original = dep("https://example.com/schemas/1.0.0", Some("./test/registry"));
        let yaml = serde_yaml::to_string(&original).expect("Failed to serialize");
        let deserialized = dep_from_yaml_lenient(&yaml).expect("Failed to deserialize");

        assert_eq!(original.schema_url, deserialized.schema_url);
        assert!(deserialized.registry_path.is_some());
    }

    #[test]
    fn test_legacy_manifest_file_warning() {
        let mut warnings = vec![];
        let result = RegistryManifest::try_from_file(
            "tests/test_data/registry_manifest.yaml",
            &mut warnings,
        );

        assert!(result.is_ok());
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, LegacyRegistryManifest { .. })),
            "Expected a LegacyRegistryManifest warning, got: {warnings:?}"
        );
    }

    #[test]
    fn test_deprecated_properties_warning() {
        let mut warnings = vec![];
        let result = RegistryManifest::try_from_file(
            "tests/test_data/valid_semconv_registry_manifest.yaml",
            &mut warnings,
        );

        assert!(result.is_ok());
        let manifest = result.unwrap();
        assert_eq!(manifest.name(), "acme.com/schemas");
        assert_eq!(manifest.version(), "0.1.0");

        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, DeprecatedSyntaxInRegistryManifest { .. })),
            "Expected a DeprecatedSyntaxInRegistryManifest warning, got: {warnings:?}"
        );
    }

    fn manifest_from_yaml(yaml: &str, nfes: &mut Vec<Error>) -> Result<RegistryManifest, Error> {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();
        RegistryManifest::try_from_file(tmp.path(), nfes)
    }

    #[test]
    fn test_unknown_file_format_is_rejected() {
        let result = manifest_from_yaml(
            r#"
file_format: "garbage/1.0.0"
schema_url: "https://example.com/schemas/1.0.0"
"#,
            &mut vec![],
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown file_format"));
    }

    #[test]
    fn test_definition_manifest_parsed_as_definition_variant() {
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");

        assert!(
            matches!(manifest, RegistryManifest::Definition(_)),
            "expected Definition variant, got {manifest:?}"
        );
    }
}
