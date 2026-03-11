// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! Two manifest types are defined here:
//! - [`DefinitionRegistryManifest`]: the definition manifest for an unpublished registry
//! - [`PublicationRegistryManifest`]: the publication manifest produced by `weaver registry package`
//!   (strict, always includes `resolved_schema_uri`).
//! - [`RegistryManifest`]: an enum discriminated by `file_format` that can be either

use std::vec;


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

/// The file format version of the publication manifest.
pub const PUBLICATION_MANIFEST_FILE_FORMAT: &str = "manifest/2.0.0";

/// Represents the definition manifest for a semantic convention registry.
///
/// This is used when developing a registry before it is published.
/// See [`PublicationRegistryManifest`] for the stricter publication form produced
/// by `weaver registry package`.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct DefinitionRegistryManifest {
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

    #[serde(skip)]
    deserialization_warnings: Vec<String>,
}

impl DefinitionRegistryManifest {
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

/// Raw helper for deserializing a manifest before validation.
/// All fields are optional so we can decide on the variant first, then validate.
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
    dependencies: Vec<Dependency>,
    #[serde(default)]
    stability: Stability,
    resolved_schema_uri: Option<String>,
}

impl RawManifestFields {
    /// Convert to [`RegistryManifest`], reporting errors relative to `path`.
    fn into_manifest(self, path: &std::path::Path) -> Result<RegistryManifest, Error> {
        if self.file_format.as_deref() == Some(PUBLICATION_MANIFEST_FILE_FORMAT) {
            let schema_url = self
                .schema_url
                .ok_or_else(|| Error::InvalidPublicationManifest {
                    path: path.to_path_buf(),
                    details: "missing required field 'schema_url'".into(),
                })?;
            let resolved_schema_uri =
                self.resolved_schema_uri
                    .ok_or_else(|| Error::InvalidPublicationManifest {
                        path: path.to_path_buf(),
                        details: "missing required field 'resolved_schema_uri'".into(),
                    })?;
            Ok(RegistryManifest::Publication(PublicationRegistryManifest {
                file_format: PUBLICATION_MANIFEST_FILE_FORMAT.to_owned(),
                schema_url,
                description: self.description,
                dependencies: self.dependencies,
                stability: self.stability,
                resolved_schema_uri,
            }))
        } else {
            let mut warnings = vec![];
            if let Some(ref fmt) = self.file_format {
                return Err(InvalidRegistryManifest {
                    path: path.to_path_buf(),
                    error: format!(
                        "Unknown file_format '{fmt}'. Expected '{PUBLICATION_MANIFEST_FILE_FORMAT}' or no file_format for a definition manifest."
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
            Ok(RegistryManifest::Definition(DefinitionRegistryManifest {
                schema_url,
                description: self.description,
                dependencies: self.dependencies,
                stability: self.stability,
                deserialization_warnings: warnings,
            }))
        }
    }
}

/// A registry manifest that can be either a definition or a publication manifest.
///
/// The `file_format` field is the discriminator:
/// - `"manifest/2.0.0"` → [`PublicationRegistryManifest`]
/// - absent → [`DefinitionRegistryManifest`]
#[derive(Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum RegistryManifest {
    /// A definition manifest (used when developing a registry).
    Definition(DefinitionRegistryManifest),
    /// A publication manifest (produced by `weaver registry package`).
    Publication(PublicationRegistryManifest),
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
        let raw: RawManifestFields =
            serde_yaml::from_reader(reader).map_err(|e| InvalidRegistryManifest {
                path: manifest_path_buf.clone(),
                error: e.to_string(),
            })?;
        let manifest = raw.into_manifest(&manifest_path_buf)?;

        // Check if this is a legacy manifest file
        let is_legacy = if let Some(file_name) = manifest_path_buf.file_name() {
            file_name == "registry.yaml"
        } else {
            false
        };

        if is_legacy {
            nfes.push(LegacyRegistryManifest {
                path: manifest_path_buf.clone(),
            });
        }

        if let RegistryManifest::Definition(ref def) = manifest {
            nfes.extend(def.deserialization_warnings.iter().map(|w| {
                DeprecatedSyntaxInRegistryManifest {
                    path: manifest_path_buf.clone(),
                    error: w.clone(),
                }
            }));
        }

        Ok(manifest)
    }

    /// Returns the schema URL of the registry.
    #[must_use]
    pub fn schema_url(&self) -> &SchemaUrl {
        match self {
            RegistryManifest::Definition(m) => &m.schema_url,
            RegistryManifest::Publication(m) => &m.schema_url,
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
            RegistryManifest::Publication(m) => &m.dependencies,
        }
    }
}

/// Represents the publication manifest for a packaged semantic convention registry.
///
/// This is produced by `weaver registry package` and describes the contents of
/// a self-contained registry artifact, including its resolved schema location.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PublicationRegistryManifest {
    /// The file format version of this publication manifest.
    /// Always `"manifest/2.0.0"`in this version.
    #[schemars(extend("const" = "manifest/2.0.0"))]
    pub file_format: String,

    /// The schema URL for this registry.
    /// Uniquely identifies the registry and its version.
    pub schema_url: SchemaUrl,

    /// An optional description of the registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// List of the registry's dependencies.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<Dependency>,

    /// The stability of this registry.
    #[serde(default)]
    pub stability: Stability,

    /// URI pointing to the resolved telemetry schema included in this package.
    pub resolved_schema_uri: String,
}

impl PublicationRegistryManifest {
    /// Creates a `PublicationRegistryManifest` from a `DefinitionRegistryManifest` and a
    /// `resolved_schema_uri` pointing to where the resolved schema will be published.
    pub fn try_from_registry_manifest(
        registry_manifest: &DefinitionRegistryManifest,
        resolved_schema_uri: String,
    ) -> Self {
        Self {
            file_format: PUBLICATION_MANIFEST_FILE_FORMAT.to_owned(),
            schema_url: registry_manifest.schema_url.clone(),
            description: registry_manifest.description.clone(),
            dependencies: registry_manifest.dependencies.clone(),
            stability: registry_manifest.stability.clone(),
            resolved_schema_uri,
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

    #[test]
    fn test_legacy_manifest_file_warning() {
        // Test that loading from a legacy manifest filename (registry_manifest.yaml) produces a warning
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
        // Test that using deprecated properties (semconv_version and schema_base_url) produces a warning
        let mut warnings = vec![];
        let result = RegistryManifest::try_from_file(
            "tests/test_data/valid_semconv_registry_manifest.yaml",
            &mut warnings,
        );

        assert!(result.is_ok());
        let manifest = result.unwrap();
        // The manifest should still work and extract the correct values
        assert_eq!(manifest.name(), "acme.com/schemas");
        assert_eq!(manifest.version(), "0.1.0");

        // But it should produce a deprecation warning
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, DeprecatedSyntaxInRegistryManifest { .. })),
            "Expected a DeprecatedSyntaxInRegistryManifest warning, got: {warnings:?}"
        );
    }

    fn manifest_from_yaml(yaml: &str) -> Result<RegistryManifest, Error> {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();
        RegistryManifest::try_from_file(tmp.path(), &mut vec![])
    }

    #[test]
    fn test_unknown_file_format_is_rejected() {
        let result = manifest_from_yaml(
            r#"
file_format: "garbage/1.0.0"
schema_url: "https://example.com/schemas/1.0.0"
"#,
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
        )
        .expect("Failed to load RegistryManifest");

        assert!(
            matches!(manifest, RegistryManifest::Definition(_)),
            "expected Definition variant, got {manifest:?}"
        );
    }

    #[test]
    fn test_publication_manifest_parsed_as_publication_variant() {
        let manifest = manifest_from_yaml(
            r#"
file_format: "manifest/2.0.0"
schema_url: "https://example.com/schemas/1.0.0"
resolved_schema_uri: "https://example.com/resolved/1.0.0/resolved.yaml"
"#,
        )
        .expect("Failed to load RegistryManifest");

        assert!(
            matches!(manifest, RegistryManifest::Publication(_)),
            "expected Publication variant, got {manifest:?}"
        );
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use crate::stability::Stability;

    fn manifest_from_yaml(yaml: &str) -> Result<RegistryManifest, Error> {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();
        RegistryManifest::try_from_file(tmp.path(), &mut vec![])
    }

    #[test]
    fn test_from_registry_manifest() {
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Definition(definition) = manifest else {
            panic!("Expected a Definition manifest");
        };

        let resolved_schema_uri = "https://example.com/resolved/1.0.0/resolved.yaml".to_owned();
        let publication = PublicationRegistryManifest::try_from_registry_manifest(
            &definition,
            resolved_schema_uri.clone(),
        );

        assert_eq!(publication.file_format, PUBLICATION_MANIFEST_FILE_FORMAT);
        assert_eq!(
            publication.schema_url.as_str(),
            "https://example.com/schemas/1.0.0"
        );
        assert_eq!(publication.description.as_deref(), Some("A test registry"));
        assert_eq!(publication.stability, Stability::Stable);
        assert!(publication.dependencies.is_empty());
        assert_eq!(publication.resolved_schema_uri, resolved_schema_uri);
    }

    #[test]
    fn test_publication_manifest_parsed_as_publication_variant() {
        // A manifest with file_format "manifest/2.0.0" and resolved_schema_uri
        // is parsed as the Publication variant.
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
file_format: "manifest/2.0.0"
resolved_schema_uri: "https://example.com/resolved/1.0.0/resolved.yaml"
"#,
        )
        .expect("Failed to load RegistryManifest");

        assert!(
            matches!(manifest, RegistryManifest::Publication(_)),
            "expected Publication variant, got {manifest:?}"
        );
    }
}
