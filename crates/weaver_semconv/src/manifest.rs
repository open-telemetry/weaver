// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the semantic conventions registry manifest.
//!
//! Two manifest types are defined here:
//! - [`DefinitionRegistryManifest`]: the definition manifest for an unpublished registry
//! - [`PublicationRegistryManifest`]: the publication manifest produced by `weaver registry package`
//!   (strict, always includes `resolved_registry_uri`).
//! - [`RegistryManifest`]: an enum discriminated by `file_format` that can be either

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
use serde::{Deserialize, Serialize};
use weaver_common::vdir::VirtualDirectoryPath;

/// The file format version of the publication manifest.
pub const PUBLICATION_MANIFEST_FILE_FORMAT: &str = "manifest/2.0";

/// Represents the definition manifest for a semantic convention registry.
///
/// This is used when developing a registry before it is published.
/// See [`PublicationRegistryManifest`] for the stricter publication form produced
/// by `weaver registry package`.
#[derive(Serialize, Debug, Clone, JsonSchema)]
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
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registry_path: Option<VirtualDirectoryPath>,
}

impl Dependency {
    /// Whether the declaration pins a version. A dependency declared by `name` (v1 manifests
    /// only) does not: it names a registry without saying which version of it, so it cannot be
    /// reconciled with any other declaration of the same registry.
    ///
    /// Detected by the version segment, so a `schema_url` an author wrote as `.../unknown`
    /// counts as unversioned too. That is the intent: it says no more about which version is
    /// wanted than the minted placeholder does. Any other segment is treated as a version,
    /// valid semver or not.
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
///
/// `strict` is set for v2 manifests, which require `schema_url`. A v1 manifest may instead
/// declare a dependency by `name`, which then gets a placeholder, unversioned schema URL minted
/// from that name.
fn parse_dependency(value: serde_yaml::Value, strict: bool) -> Result<Dependency, String> {
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
    if strict || name.is_none() {
        let subject = name.map_or_else(
            || "a dependency".to_owned(),
            |name| format!("dependency '{name}'"),
        );
        return Err(format!(
            "{subject} is missing the required field 'schema_url'. {SCHEMA_URL_HELP}"
        ));
    }

    let named: NamedDependency = serde_yaml::from_value(value).map_err(|e| e.to_string())?;
    Ok(Dependency {
        schema_url: SchemaUrl::try_from_name_version(&named.name, UNKNOWN_VERSION)?,
        registry_path: Some(named.registry_path),
    })
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
    /// Parsed once the manifest version is known: only v1 manifests may declare a dependency
    /// by `name`.
    #[serde(default)]
    dependencies: Vec<serde_yaml::Value>,
    #[serde(default)]
    stability: Stability,
    resolved_registry_uri: Option<String>,
    /// Deprecated alias for `resolved_registry_uri`.
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
            let dependencies = self
                .dependencies
                .into_iter()
                .map(|value| parse_dependency(value, true))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|details| Error::InvalidPublicationManifest {
                    path: path.to_path_buf(),
                    details,
                })?;
            let mut warnings = vec![];
            let resolved_registry_uri = match (self.resolved_registry_uri, self.resolved_schema_uri)
            {
                (Some(v), _) => v,
                (None, Some(v)) => {
                    warnings.push(
                        "The 'resolved_schema_uri' field is deprecated in favor of 'resolved_registry_uri'."
                            .to_owned(),
                    );
                    v
                }
                (None, None) => {
                    return Err(Error::InvalidPublicationManifest {
                        path: path.to_path_buf(),
                        details: "missing required field 'resolved_registry_uri'".into(),
                    });
                }
            };
            Ok(RegistryManifest::Publication(PublicationRegistryManifest {
                file_format: PUBLICATION_MANIFEST_FILE_FORMAT.to_owned(),
                schema_url,
                description: self.description,
                dependencies,
                stability: self.stability,
                resolved_registry_uri,
                deserialization_warnings: warnings,
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
            // Only v1 manifests -- those identified by `schema_base_url` + `semconv_version`
            // rather than `schema_url` -- may declare dependencies by `name`.
            let is_v1 = self.schema_url.is_none();
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
                .map(|value| parse_dependency(value, !is_v1))
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
}

/// A registry manifest that can be either a definition or a publication manifest.
///
/// The `file_format` field is the discriminator:
/// - `"manifest/2.0"` → [`PublicationRegistryManifest`]
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
            file_name == LEGACY_REGISTRY_MANIFEST
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
            RegistryManifest::Publication(pubm) => pubm.deserialization_warnings.as_slice(),
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
/// a self-contained registry artifact, including the URI of the resolved
/// registry artifact (`resolved.yaml`).
#[derive(Serialize, Debug, Clone, JsonSchema)]
pub struct PublicationRegistryManifest {
    /// The file format version of this publication manifest.
    /// Always `"manifest/2.0"`in this version.
    #[schemars(extend("const" = "manifest/2.0"))]
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

    /// URI pointing to the resolved registry artifact included in this package.
    #[serde(alias = "resolved_schema_uri")]
    pub resolved_registry_uri: String,

    #[serde(skip)]
    deserialization_warnings: Vec<String>,
}

impl PublicationRegistryManifest {
    /// Creates a `PublicationRegistryManifest` from a `DefinitionRegistryManifest` and a
    /// `resolved_registry_uri` pointing to where the resolved registry will be published.
    ///
    /// Dependencies are reduced to their `schema_url`: `registry_path` points at the author's
    /// machine and means nothing to a consumer of the published registry, who locates the
    /// dependency by its schema URL. That URL must pin a version, which rules out dependencies
    /// declared by `name`.
    pub fn try_from_registry_manifest(
        registry_manifest: &DefinitionRegistryManifest,
        resolved_registry_uri: String,
    ) -> Result<Self, Error> {
        let dependencies = registry_manifest
            .dependencies
            .iter()
            .map(|dependency| {
                if !dependency.is_versioned() {
                    return Err(Error::UnversionedDependencyInPublication {
                        schema_url: dependency.schema_url.to_string(),
                        registry_path: dependency.registry_path.as_ref().map(ToString::to_string),
                    });
                }
                Ok(Dependency {
                    schema_url: dependency.schema_url.clone(),
                    registry_path: None,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            file_format: PUBLICATION_MANIFEST_FILE_FORMAT.to_owned(),
            schema_url: registry_manifest.schema_url.clone(),
            description: registry_manifest.description.clone(),
            dependencies,
            stability: registry_manifest.stability.clone(),
            resolved_registry_uri,
            deserialization_warnings: vec![],
        })
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
    /// Parses as a v1 manifest would: both dependency shapes are accepted.
    fn dep_from_yaml_lenient(yaml: &str) -> Result<Dependency, String> {
        parse_dependency(serde_yaml::from_str(yaml).expect("invalid YAML"), false)
    }

    /// Parses as a v2 manifest would: only the `schema_url` shape is accepted.
    fn dep_from_yaml_strict(yaml: &str) -> Result<Dependency, String> {
        parse_dependency(serde_yaml::from_str(yaml).expect("invalid YAML"), true)
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

    /// A version segment that is not semver is still a version the author supplied, so it must
    /// not be mistaken for the placeholder minted from a `name`.
    #[test]
    fn test_non_semver_schema_url_is_versioned() {
        let dep = dep_from_yaml_lenient(r#"schema_url: "https://example.com/dep/1.0""#)
            .expect("Failed to deserialize");
        assert!(dep.schema_url.semver().is_err());
        assert!(dep.is_versioned());
    }

    #[test]
    fn test_v1_dependency_name_without_registry_path_is_rejected() {
        // A dependency declared by `name` has no `schema_url` to locate the files with,
        // so `registry_path` is required.
        let err = dep_from_yaml_lenient(r#"name: "acme-registry""#)
            .expect_err("a dependency with no identity and no path cannot be located");
        assert!(
            err.contains("registry_path"),
            "error should report the missing 'registry_path'; got: {err}"
        );
    }

    #[test]
    fn test_v2_dependency_name_only_is_rejected() {
        let err = dep_from_yaml_strict(
            r#"
name: "acme-registry"
registry_path: "./registry"
"#,
        )
        .expect_err("a v2 manifest must reject a dependency declared by name");
        assert!(
            err.contains("schema_url") && err.contains("acme-registry"),
            "error should name the dependency missing 'schema_url'; got: {err}"
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
        for result in [
            dep_from_yaml_lenient(r#"registry_path: "./registry""#),
            dep_from_yaml_strict(r#"registry_path: "./registry""#),
        ] {
            let err = result.expect_err("a dependency with no identity must be rejected");
            assert!(err.contains("schema_url"), "got: {err}");
        }
    }

    #[test]
    fn test_dependency_serialize() {
        let yaml = serde_yaml::to_string(&dep("https://opentelemetry.io/schemas/1.0.0", None))
            .expect("Failed to serialize");
        assert!(yaml.contains("schema_url"));
        assert!(yaml.contains("https://opentelemetry.io/schemas/1.0.0"));
        // `registry_path` is skipped when None.
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

    #[test]
    fn test_publication_manifest_parsed_as_publication_variant() {
        let manifest = manifest_from_yaml(
            r#"
file_format: "manifest/2.0"
schema_url: "https://example.com/schemas/1.0.0"
resolved_registry_uri: "https://example.com/resolved/1.0.0/resolved.yaml"
"#,
            &mut vec![],
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

    fn manifest_from_yaml(yaml: &str, nfes: &mut Vec<Error>) -> Result<RegistryManifest, Error> {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();
        RegistryManifest::try_from_file(tmp.path(), nfes)
    }

    #[test]
    fn test_from_registry_manifest() {
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Definition(definition) = manifest else {
            panic!("Expected a Definition manifest");
        };

        let resolved_registry_uri = "https://example.com/resolved/1.0.0/resolved.yaml".to_owned();
        let publication = PublicationRegistryManifest::try_from_registry_manifest(
            &definition,
            resolved_registry_uri.clone(),
        )
        .expect("Failed to build the publication manifest");

        assert_eq!(publication.file_format, PUBLICATION_MANIFEST_FILE_FORMAT);
        assert_eq!(
            publication.schema_url.as_str(),
            "https://example.com/schemas/1.0.0"
        );
        assert_eq!(publication.description.as_deref(), Some("A test registry"));
        assert_eq!(publication.stability, Stability::Stable);
        assert!(publication.dependencies.is_empty());
        assert_eq!(publication.resolved_registry_uri, resolved_registry_uri);
    }

    /// The local `registry_path` of a dependency is not published: consumers locate the
    /// dependency by its schema URL, and the path only exists on the author's machine.
    #[test]
    fn test_from_registry_manifest_drops_dependency_registry_path() {
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
dependencies:
  - schema_url: "https://example.com/dep/2.0.0"
    registry_path: "/home/author/dep/registry"
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Definition(definition) = manifest else {
            panic!("Expected a Definition manifest");
        };

        let publication = PublicationRegistryManifest::try_from_registry_manifest(
            &definition,
            "https://example.com/resolved/1.0.0/resolved.yaml".to_owned(),
        )
        .expect("Failed to build the publication manifest");

        let [dependency] = publication.dependencies.as_slice() else {
            panic!("expected exactly one dependency");
        };
        assert_eq!(
            dependency.schema_url.as_str(),
            "https://example.com/dep/2.0.0"
        );
        assert!(dependency.registry_path.is_none());

        // The result must round-trip through the publication manifest reader.
        let yaml = serde_yaml::to_string(&publication).expect("Failed to serialize");
        let reparsed =
            manifest_from_yaml(&yaml, &mut vec![]).expect("publication manifest is not readable");
        assert!(matches!(reparsed, RegistryManifest::Publication(_)));
    }

    /// A dependency declared with the v1 `name` syntax carries no version, so it cannot be
    /// published; packaging must fail rather than emit a manifest the reader rejects.
    #[test]
    fn test_from_registry_manifest_rejects_v1_dependency() {
        let manifest = manifest_from_yaml(
            r#"
schema_base_url: "https://example.com/schemas"
semconv_version: "1.0.0"
dependencies:
  - name: "acme-registry"
    registry_path: "/home/author/dep/registry"
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Definition(definition) = manifest else {
            panic!("Expected a Definition manifest");
        };

        let result = PublicationRegistryManifest::try_from_registry_manifest(
            &definition,
            "https://example.com/resolved/1.0.0/resolved.yaml".to_owned(),
        );
        assert!(matches!(
            result,
            Err(Error::UnversionedDependencyInPublication {
                schema_url,
                registry_path,
            }) if schema_url == "https://acme-registry/unknown"
                && registry_path.as_deref() == Some("/home/author/dep/registry")
        ));
    }

    #[test]
    fn test_publication_manifest_parsed_as_publication_variant() {
        // A manifest with file_format "manifest/2.0" and resolved_registry_uri
        // is parsed as the Publication variant.
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
file_format: "manifest/2.0"
resolved_registry_uri: "https://example.com/resolved/1.0.0/resolved.yaml"
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");

        assert!(
            matches!(manifest, RegistryManifest::Publication(_)),
            "expected Publication variant, got {manifest:?}"
        );
    }

    /// A publication YAML using the deprecated `resolved_schema_uri` field name
    /// must still deserialize correctly into the renamed `resolved_registry_uri`
    /// field, and surface a deprecation warning via `nfes`.
    #[test]
    fn test_publication_manifest_accepts_deprecated_resolved_schema_uri() {
        let mut nfes = vec![];
        let manifest = manifest_from_yaml(
            r#"
file_format: "manifest/2.0"
schema_url: "https://example.com/schemas/1.0.0"
resolved_schema_uri: "https://example.com/resolved/1.0.0/resolved.yaml"
"#,
            &mut nfes,
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Publication(pubm) = manifest else {
            panic!("expected Publication variant");
        };
        assert_eq!(
            pubm.resolved_registry_uri,
            "https://example.com/resolved/1.0.0/resolved.yaml"
        );
        assert!(
            nfes.iter()
                .any(|w| matches!(w, DeprecatedSyntaxInRegistryManifest { .. })),
            "expected a DeprecatedSyntaxInRegistryManifest warning, got: {nfes:?}"
        );
    }

    /// When both the deprecated `resolved_schema_uri` and the new
    /// `resolved_registry_uri` are provided, the new name wins (no warning).
    #[test]
    fn test_publication_manifest_new_name_wins_over_deprecated() {
        let mut nfes = vec![];
        let manifest = manifest_from_yaml(
            r#"
file_format: "manifest/2.0"
schema_url: "https://example.com/schemas/1.0.0"
resolved_registry_uri: "https://example.com/resolved/new.yaml"
resolved_schema_uri: "https://example.com/resolved/old.yaml"
"#,
            &mut nfes,
        )
        .expect("Failed to load RegistryManifest");

        let RegistryManifest::Publication(pubm) = manifest else {
            panic!("expected Publication variant");
        };
        assert_eq!(
            pubm.resolved_registry_uri,
            "https://example.com/resolved/new.yaml"
        );
    }

    /// A publication manifest that omits both the new and deprecated names
    /// is rejected with `InvalidPublicationManifest`.
    #[test]
    fn test_publication_manifest_missing_resolved_registry_uri_is_error() {
        let result = manifest_from_yaml(
            r#"
file_format: "manifest/2.0"
schema_url: "https://example.com/schemas/1.0.0"
"#,
            &mut vec![],
        );
        assert!(matches!(
            result,
            Err(Error::InvalidPublicationManifest { details, .. })
                if details.contains("resolved_registry_uri")
        ));
    }
}
