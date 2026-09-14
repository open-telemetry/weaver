// SPDX-License-Identifier: Apache-2.0

//! Contains the definitions for the version 2 semantic conventions registry manifest.
//!
//! Two manifest types are defined here:
//! - [`DefinitionRegistryManifest`]: the definition manifest for an unpublished registry
//! - [`PublicationRegistryManifest`]: the publication manifest produced by `weaver registry package`
//!   (strict, always includes `resolved_registry_uri`).
//! - [`RegistryManifest`]: an enum discriminated by `file_format` that can be either

use std::vec;

#[allow(deprecated)]
use crate::registry_repo::LEGACY_REGISTRY_MANIFEST;
use crate::schema_url::SchemaUrl;
use crate::v2::stability::Stability;
use crate::Error;
use crate::Error::{
    DeprecatedSyntaxInRegistryManifest, InvalidRegistryManifest, LegacyRegistryManifest,
    MissingManifestFileFormat, RegistryManifestNotFound,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_common::vdir::VirtualDirectoryPath;

/// The file format version of the definition manifest.
pub const DEFINITION_MANIFEST_FILE_FORMAT: &str = "definition_manifest/2.0";

/// The file format version of the publication manifest.
pub const PUBLICATION_MANIFEST_FILE_FORMAT: &str = "manifest/2.0";

/// Represents the definition manifest for a semantic convention registry (version 2).
///
/// This is used when developing a registry before it is published.
/// See [`PublicationRegistryManifest`] for the stricter publication form produced
/// by `weaver registry package`.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct DefinitionRegistryManifest {
    /// The file format version of this definition manifest.
    /// Always `"definition_manifest/2.0"` in this version.
    #[schemars(extend("const" = "definition_manifest/2.0"))]
    pub file_format: String,

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
    pub(crate) deserialization_warnings: Vec<String>,
}

impl DefinitionRegistryManifest {
    /// Creates a new `DefinitionRegistryManifest` from a schema URL with default values.
    #[must_use]
    pub fn from_schema_url(schema_url: SchemaUrl) -> Self {
        Self {
            file_format: DEFINITION_MANIFEST_FILE_FORMAT.to_owned(),
            schema_url,
            description: None,
            dependencies: vec![],
            stability: Stability::Development,
            deserialization_warnings: vec![],
        }
    }
}

/// Represents a dependency of a semantic convention registry (version 2).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Dependency {
    /// The schema URL for the dependency (required).
    /// It must follow OTel schema URL format, which is: `http[s]://server[:port]/path/<version>`.
    /// This is not necessarily the URL a registry can be accessed at, but it provides
    /// a unique identifier for the dependency registry and its version.
    pub schema_url: SchemaUrl,

    /// The path to the dependency (optional).
    /// This can be either:
    /// - A manifest of a published registry
    /// - A directory containing the raw definition.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registry_path: Option<VirtualDirectoryPath>,
}

impl Dependency {
    /// Whether the declaration pins a version.
    #[must_use]
    pub fn is_versioned(&self) -> bool {
        self.schema_url.version() != UNKNOWN_VERSION
    }
}

const SCHEMA_URL_HELP: &str = "The schema_url uniquely identifies the dependency registry \
                               and its version, e.g. https://example.com/my-registry/1.0.0";

/// The version given to a dependency declared by `name`, which carries none.
const UNKNOWN_VERSION: &str = "unknown";

/// Parses a dependency declaration strictly requiring `schema_url`.
fn parse_dependency(value: serde_yaml::Value) -> Result<Dependency, String> {
    if value.get("schema_url").is_some() {
        return serde_yaml::from_value(value).map_err(|e| e.to_string());
    }
    let name = value.get("name").and_then(serde_yaml::Value::as_str);
    let subject = name.map_or_else(
        || "a dependency".to_owned(),
        |name| format!("dependency '{name}'"),
    );
    Err(format!(
        "{subject} is missing the required field 'schema_url'. {SCHEMA_URL_HELP}"
    ))
}

/// Raw helper for deserializing a manifest before validation.
/// All fields are optional so we can decide on the variant first, then validate.
#[derive(Deserialize)]
struct RawManifestFields {
    file_format: Option<String>,
    schema_url: Option<SchemaUrl>,
    description: Option<String>,
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
        let is_publication = self.file_format.as_deref() == Some(PUBLICATION_MANIFEST_FILE_FORMAT);
        let is_definition = self.file_format.as_deref() == Some(DEFINITION_MANIFEST_FILE_FORMAT);
        let is_empty = self.file_format.is_none();

        if is_publication {
            let schema_url = self
                .schema_url
                .ok_or_else(|| Error::InvalidPublicationManifest {
                    path: path.to_path_buf(),
                    details: "missing required field 'schema_url'".into(),
                })?;
            let dependencies = self
                .dependencies
                .into_iter()
                .map(parse_dependency)
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
        } else if is_definition || is_empty {
            let mut warnings = vec![];
            if is_empty {
                warnings.push(format!(
                    "Missing 'file_format' field. Assumed to be a V2 definition manifest ('{DEFINITION_MANIFEST_FILE_FORMAT}')."
                ));
            }
            let schema_url = self.schema_url.ok_or_else(|| InvalidRegistryManifest {
                path: path.to_path_buf(),
                error: "missing required field 'schema_url'".into(),
            })?;
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
                file_format: DEFINITION_MANIFEST_FILE_FORMAT.to_owned(),
                schema_url,
                description: self.description,
                dependencies,
                stability: self.stability,
                deserialization_warnings: warnings,
            }))
        } else {
            let fmt = self.file_format.as_deref().unwrap_or("unknown");
            Err(InvalidRegistryManifest {
                path: path.to_path_buf(),
                error: format!(
                    "Unknown file_format '{fmt}'. Expected '{DEFINITION_MANIFEST_FILE_FORMAT}' or '{PUBLICATION_MANIFEST_FILE_FORMAT}'."
                ),
            })
        }
    }
}

/// A registry manifest that can be either a definition or a publication manifest (version 2).
///
/// The `file_format` field is the discriminator:
/// - `"manifest/2.0"` → [`PublicationRegistryManifest`]
/// - `"definition_manifest/2.0"` → [`DefinitionRegistryManifest`]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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
            RegistryManifest::Publication(pubm) => pubm.deserialization_warnings.as_slice(),
        };
        for w in deserialization_warnings {
            if w.starts_with("Missing 'file_format' field") {
                nfes.push(MissingManifestFileFormat {
                    path: manifest_path_buf.clone(),
                    expected_format: DEFINITION_MANIFEST_FILE_FORMAT.to_owned(),
                });
            } else {
                nfes.push(DeprecatedSyntaxInRegistryManifest {
                    path: manifest_path_buf.clone(),
                    error: w.clone(),
                });
            }
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

    /// Returns the dependencies of the registry.
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        match self {
            RegistryManifest::Definition(m) => &m.dependencies,
            RegistryManifest::Publication(m) => &m.dependencies,
        }
    }

    /// Returns the description of the registry, if present.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            RegistryManifest::Definition(m) => m.description.as_deref(),
            RegistryManifest::Publication(m) => m.description.as_deref(),
        }
    }
}

/// Represents the publication manifest for a packaged semantic convention registry.
///
/// This is produced by `weaver registry package` and describes the contents of
/// a self-contained registry artifact, including the URI of the resolved
/// registry artifact (`resolved.yaml`).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PublicationRegistryManifest {
    /// The file format version of this publication manifest.
    /// Always `"manifest/2.0"` in this version.
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
    pub(crate) deserialization_warnings: Vec<String>,
}

impl PublicationRegistryManifest {
    /// Creates a `PublicationRegistryManifest` from a `DefinitionRegistryManifest` and a
    /// `resolved_registry_uri` pointing to where the resolved registry will be published.
    ///
    /// Dependencies are reduced to their `schema_url`: `registry_path` points at the author's
    /// machine and means nothing to a consumer of the published registry, who locates the
    /// dependency by its schema URL. That URL must pin a version.
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
    use super::*;

    #[test]
    fn test_not_found_registry_info() {
        let result =
            RegistryManifest::try_from_file("tests/test_data/missing_registry.yaml", &mut vec![]);
        assert!(
            matches!(result, Err(RegistryManifestNotFound { path, .. }) if path.ends_with("missing_registry.yaml"))
        );
    }

    fn dep_from_yaml_strict(yaml: &str) -> Result<Dependency, String> {
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
        let dep = dep_from_yaml_strict(r#"schema_url: "https://opentelemetry.io/schemas/1.0.0""#)
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
        let dep = dep_from_yaml_strict(
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
    fn test_non_semver_schema_url_is_versioned() {
        let dep = dep_from_yaml_strict(r#"schema_url: "https://example.com/dep/1.0""#)
            .expect("Failed to deserialize");
        assert!(dep.schema_url.semver().is_err());
        assert!(dep.is_versioned());
    }

    #[test]
    fn test_dependency_deserialize_missing_both_fields() {
        let err = dep_from_yaml_strict(r#"registry_path: "./registry""#)
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
        let deserialized = dep_from_yaml_strict(&yaml).expect("Failed to deserialize");

        assert_eq!(original.schema_url, deserialized.schema_url);
        assert!(deserialized.registry_path.is_some());
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
file_format: "definition_manifest/2.0"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
            &mut vec![],
        )
        .expect("Failed to load RegistryManifest");
        assert!(matches!(manifest, RegistryManifest::Definition(_)));
    }

    #[test]
    fn test_definition_manifest_missing_file_format_is_accepted_with_warning() {
        let mut nfes = vec![];
        let manifest = manifest_from_yaml(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
            &mut nfes,
        )
        .expect("Failed to load RegistryManifest with missing file_format");

        assert!(matches!(manifest, RegistryManifest::Definition(_)));
        assert_eq!(
            manifest.schema_url().as_str(),
            "https://example.com/schemas/1.0.0"
        );
        assert!(
            nfes.iter()
                .any(|w| matches!(w, MissingManifestFileFormat { .. })),
            "Expected MissingManifestFileFormat warning, got: {nfes:?}"
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
        assert!(matches!(manifest, RegistryManifest::Publication(_)));
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;

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
file_format: "definition_manifest/2.0"
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

    #[test]
    fn test_from_registry_manifest_drops_dependency_registry_path() {
        let manifest = manifest_from_yaml(
            r#"
file_format: "definition_manifest/2.0"
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

        let yaml = serde_yaml::to_string(&publication).expect("Failed to serialize");
        let reparsed =
            manifest_from_yaml(&yaml, &mut vec![]).expect("publication manifest is not readable");
        assert!(matches!(reparsed, RegistryManifest::Publication(_)));
    }

    #[test]
    fn test_from_registry_manifest_rejects_unversioned_dependency() {
        let definition = DefinitionRegistryManifest {
            file_format: DEFINITION_MANIFEST_FILE_FORMAT.to_owned(),
            schema_url: "https://example.com/schemas/1.0.0".try_into().unwrap(),
            description: None,
            dependencies: vec![Dependency {
                schema_url: SchemaUrl::try_from_name_version("acme-registry", "unknown").unwrap(),
                registry_path: Some(VirtualDirectoryPath::LocalFolder {
                    path: "/home/author/dep/registry".into(),
                }),
            }],
            stability: Stability::Stable,
            deserialization_warnings: vec![],
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
