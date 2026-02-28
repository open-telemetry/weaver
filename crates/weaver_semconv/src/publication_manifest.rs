// SPDX-License-Identifier: Apache-2.0

//! Publication manifest for a packaged semantic convention registry.
//!
//! This is produced by `weaver registry package` and describes the contents of
//! a self-contained registry artifact, including its resolved schema location.

use crate::manifest::{Dependency, RegistryManifest};
use crate::schema_url::SchemaUrl;
use crate::stability::Stability;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The file format version of the publication manifest.
/// TODO: do we want it to be 2.0.0 or manifest/2.0.0 ?
pub const PUBLICATION_MANIFEST_FILE_FORMAT: &str = "2.0.0";

/// Represents the publication manifest for a packaged semantic convention registry.
///
/// This is produced by `weaver registry package` and describes the contents of
/// a self-contained registry artifact, including its resolved schema location.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PublicationRegistryManifest {
    /// The file format version of this publication manifest.
    /// Always `"2.0.0"`.
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
    /// Creates a `PublicationRegistryManifest` from a `RegistryManifest` and a
    /// `resolved_schema_uri` pointing to where the resolved schema will be published.
    #[must_use]
    pub fn from_registry_manifest(
        registry_manifest: &RegistryManifest,
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
    use super::*;
    use crate::stability::Stability;

    #[test]
    fn test_from_registry_manifest() {
        let manifest: RegistryManifest = serde_yaml::from_str(
            r#"
schema_url: "https://example.com/schemas/1.0.0"
description: "A test registry"
stability: stable
"#,
        )
        .expect("Failed to deserialize RegistryManifest");

        let resolved_schema_uri =
            "https://example.com/resolved/1.0.0/resolved.yaml".to_owned();
        let publication = PublicationRegistryManifest::from_registry_manifest(
            &manifest,
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
}
