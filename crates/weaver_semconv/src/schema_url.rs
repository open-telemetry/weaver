// SPDX-License-Identifier: Apache-2.0

//! Schema URL type for uniquely identifying semantic convention registries.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::OnceLock;

/// Represents the schema URL of a registry, which serves as a unique identifier for the registry
/// along with its version.
#[derive(Debug, Clone, JsonSchema)]
pub struct SchemaUrl {
    /// The schema URL string.
    pub url: String,
    #[serde(skip)]
    #[schemars(skip)]
    name: OnceLock<String>,
    #[serde(skip)]
    #[schemars(skip)]
    version: OnceLock<String>,
}

impl SchemaUrl {
    /// Create a new SchemaUrl from a string.
    #[must_use]
    fn new(url: String) -> Self {
        Self {
            url,
            name: OnceLock::new(),
            version: OnceLock::new(),
        }
    }

    /// Get the URL as a string.
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// Validate the schema URL format.
    pub fn validate(&self) -> Result<(), String> {
        let parsed = url::Url::parse(&self.url).map_err(|e| format!("Invalid schema URL: {e}"))?;
        let has_path = parsed
            .path_segments()
            .map(|segments| segments.filter(|s| !s.is_empty()).count() > 0)
            .unwrap_or(false);

        if !has_path {
            return Err("The schema URL must have at least one path segment.".to_owned());
        }
        Ok(())
    }

    /// Returns the registry name, derived from the schema URL.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.get_or_init(|| {
            let parsed_url = url::Url::parse(&self.url).expect("schema_url must be valid");
            let path = parsed_url.path().trim_matches('/');
            let mut segments: Vec<&str> = path.split('/').collect();
            if !segments.is_empty() {
                _ = segments.pop();
            }

            // Construct authority from host and port (replaces deprecated authority() method)
            let authority = match (parsed_url.host_str(), parsed_url.port()) {
                (Some(host), Some(port)) => format!("{}:{}", host, port),
                (Some(host), None) => host.to_owned(),
                _ => String::new(),
            };

            if segments.is_empty() {
                return authority;
            }

            format!("{}/{}", authority, segments.join("/"))
        })
    }

    /// Returns the registry version, derived from the schema URL.
    #[must_use]
    pub fn version(&self) -> &str {
        self.version.get_or_init(|| {
            let parsed_url = url::Url::parse(&self.url).expect("schema_url must be valid");
            parsed_url
                .path()
                .trim_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_owned()
        })
    }

    /// Create a SchemaUrl from name and version.
    pub fn try_from_name_version(name: &str, version: &str) -> Result<Self, String> {
        if name.trim().is_empty() || version.trim().is_empty() {
            return Err("Registry name and version cannot be empty.".to_owned());
        }
        // TODO: replace with scheme regex

        if name.starts_with("http://") || name.starts_with("https://") {
            format!("{}/{}", name.trim_end_matches('/'), version).try_into()
        } else {
            format!("https://{}/{}", name.trim_end_matches('/'), version).try_into()
        }
    }

    /// Returns a default unknown schema URL.
    #[must_use]
    pub fn new_unknown() -> Self {
        Self::new("https://unknown/unknown".to_owned())
    }
}

impl PartialEq for SchemaUrl {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Eq for SchemaUrl {}

impl std::hash::Hash for SchemaUrl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

impl std::fmt::Display for SchemaUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl<'de> Deserialize<'de> for SchemaUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let schema: SchemaUrl = s.try_into().map_err(serde::de::Error::custom)?;
        Ok(schema)
    }
}

impl Serialize for SchemaUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.url)
    }
}

impl TryFrom<&str> for SchemaUrl {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let schema_url = Self::new(value.to_owned());
        schema_url.validate()?;
        Ok(schema_url)
    }
}

impl TryFrom<String> for SchemaUrl {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let schema_url = Self::new(value);
        schema_url.validate()?;
        Ok(schema_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_as_str() {
        let url = "https://opentelemetry.io/schemas/1.0.0";
        let schema_url: SchemaUrl = url.try_into().unwrap();
        assert_eq!(schema_url.as_str(), url);
    }

    #[test]
    fn test_validate_invalid_url_syntax() {
        let result: Result<SchemaUrl, _> = "not a valid url".try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_url_without_path() {
        let result = TryInto::<SchemaUrl>::try_into("https://opentelemetry.io");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least one path segment"));
    }

    #[test]
    fn test_try_new_valid_url() {
        let result = TryInto::<SchemaUrl>::try_into("https://opentelemetry.io/schemas/1.0.0");
        assert!(result.is_ok());
        let schema_url = result.unwrap();
        assert_eq!(
            schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
    }

    #[test]
    fn test_name_extraction_simple() {
        let schema_url: SchemaUrl =
            TryInto::<SchemaUrl>::try_into("https://opentelemetry.io/schemas/1.0.0").unwrap();
        assert_eq!(schema_url.name(), "opentelemetry.io/schemas");
    }

    #[test]
    fn test_name_extraction_nested_path() {
        let schema_url: SchemaUrl =
            TryInto::<SchemaUrl>::try_into("https://opentelemetry.io/schemas/sub-component/1.0.0")
                .unwrap();
        assert_eq!(schema_url.name(), "opentelemetry.io/schemas/sub-component");
    }

    #[test]
    fn test_name_extraction_single_segment() {
        let schema_url: SchemaUrl = "https://opentelemetry.io/1.0.0".try_into().unwrap();
        assert_eq!(schema_url.name(), "opentelemetry.io");
    }

    #[test]
    fn test_name_extraction_with_port() {
        let schema_url: SchemaUrl = "https://example.com:8080/schemas/1.0.0".try_into().unwrap();
        assert_eq!(schema_url.name(), "example.com:8080/schemas");
    }

    #[test]
    fn test_version_extraction_simple() {
        let schema_url: SchemaUrl = "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap();
        assert_eq!(schema_url.version(), "1.0.0");
    }

    #[test]
    fn test_version_extraction_semantic_version() {
        let schema_url: SchemaUrl = "https://example.com/schemas/1.2.3".try_into().unwrap();
        assert_eq!(schema_url.version(), "1.2.3");
    }

    #[test]
    fn test_version_extraction_single_segment() {
        let schema_url: SchemaUrl = "https://example.com/v1".try_into().unwrap();
        assert_eq!(schema_url.version(), "v1");
    }

    #[test]
    fn test_try_from_name_version_with_https() {
        let result = SchemaUrl::try_from_name_version("https://opentelemetry.io/schemas", "1.0.0");
        assert!(result.is_ok());
        let schema_url = result.unwrap();
        assert_eq!(
            schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
    }

    #[test]
    fn test_try_from_name_version_without_scheme() {
        let result = SchemaUrl::try_from_name_version("opentelemetry.io/schemas", "1.0.0");
        assert!(result.is_ok());
        let schema_url = result.unwrap();
        assert_eq!(
            schema_url.as_str(),
            "https://opentelemetry.io/schemas/1.0.0"
        );
    }

    #[test]
    fn test_try_from_name_version_with_http() {
        let result = SchemaUrl::try_from_name_version("http://example.com/schemas", "1.0.0");
        assert!(result.is_ok());
        let schema_url = result.unwrap();
        assert_eq!(schema_url.as_str(), "http://example.com/schemas/1.0.0");
    }

    #[test]
    fn test_try_from_name_version_with_trailing_slash() {
        let result = SchemaUrl::try_from_name_version("https://example.com/schemas/", "1.0.0");
        assert!(result.is_ok());
        let schema_url = result.unwrap();
        assert_eq!(schema_url.as_str(), "https://example.com/schemas/1.0.0");
    }

    #[test]
    fn test_equality() {
        let url1: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();
        let url2: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();
        let url3: SchemaUrl = "https://example.com/schemas/2.0.0".try_into().unwrap();

        assert_eq!(url1, url2);
        assert_ne!(url1, url3);
    }

    #[test]
    fn test_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let url1: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();
        let url2: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();

        let mut hasher1 = DefaultHasher::new();
        url1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        url2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_display() {
        let schema_url: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();
        assert_eq!(
            format!("{}", schema_url),
            "https://example.com/schemas/1.0.0"
        );
    }

    #[test]
    fn test_serialize() {
        let schema_url: SchemaUrl = "https://example.com/schemas/1.0.0".try_into().unwrap();
        let json = serde_json::to_string(&schema_url).unwrap();
        assert_eq!(json, "\"https://example.com/schemas/1.0.0\"");
    }

    #[test]
    fn test_deserialize() {
        let json = "\"https://example.com/schemas/1.0.0\"";
        let schema_url: SchemaUrl = serde_json::from_str(json).unwrap();
        assert_eq!(schema_url.as_str(), "https://example.com/schemas/1.0.0");
    }

    #[test]
    fn test_deserialize_invalid_url() {
        let json = "\"not a valid url\"";
        let result: Result<SchemaUrl, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid schema URL"));
    }

    #[test]
    fn test_deserialize_url_without_path() {
        let json = "\"https://example.com\"";
        let result: Result<SchemaUrl, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("at least one path segment"));
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original: SchemaUrl = "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SchemaUrl = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_name_caching() {
        let schema_url: SchemaUrl = "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap();

        // Call name() twice and verify they return the same reference
        let name1 = schema_url.name();
        let name2 = schema_url.name();

        assert_eq!(name1, name2);
        assert_eq!(name1, "opentelemetry.io/schemas");

        // Verify we're getting the same pointer (cached value)
        assert_eq!(name1.as_ptr(), name2.as_ptr());
    }

    #[test]
    fn test_version_caching() {
        let schema_url: SchemaUrl = "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap();

        // Call version() twice and verify they return the same reference
        let version1 = schema_url.version();
        let version2 = schema_url.version();

        assert_eq!(version1, version2);
        assert_eq!(version1, "1.0.0");

        // Verify we're getting the same pointer (cached value)
        assert_eq!(version1.as_ptr(), version2.as_ptr());
    }

    #[test]
    fn test_clone_preserves_url_but_resets_cache() {
        let original: SchemaUrl = "https://opentelemetry.io/schemas/1.0.0".try_into().unwrap();

        // Access name to populate cache
        let _ = original.name();

        // Clone should have the same URL but empty cache
        let cloned = original.clone();
        assert_eq!(original.as_str(), cloned.as_str());
        assert_eq!(original.name(), cloned.name());
    }
}
