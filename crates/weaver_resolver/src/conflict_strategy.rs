// SPDX-License-Identifier: Apache-2.0

//! Dependency version conflict resolution strategies.

use crate::Error;
use weaver_semconv::schema_url::SchemaUrl;

/// Trait defining a strategy for resolving version conflicts between two dependencies.
pub(crate) trait DependencyVersionConflictStrategy {
    /// Resolves a version conflict between two dependency schema URLs (`url1` and `url2`).
    ///
    /// Returns the chosen `SchemaUrl` or an `Error` if the conflict cannot be resolved under this strategy.
    fn resolve_conflict(&self, url1: &SchemaUrl, url2: &SchemaUrl) -> Result<SchemaUrl, Error>;
}

/// The `use_latest_major_version` dependency version conflict resolution strategy.
///
/// Under this strategy:
/// - Two dependency versions (`url1` and `url2`) are compatible if and only if they share the exact
///   same registry name (`url1.name() == url2.name()`) and the exact same major version (`v1.major == v2.major`).
/// - When two compatible versions conflict, the `SchemaUrl` with the higher semantic version (`>`) is returned.
/// - If two versions with different registry names or different major versions conflict, an error (`AmbiguousReference`
///   or `DuplicateDependency`) is returned.
pub(crate) struct UseLatestMajorVersion;

impl DependencyVersionConflictStrategy for UseLatestMajorVersion {
    fn resolve_conflict(&self, url1: &SchemaUrl, url2: &SchemaUrl) -> Result<SchemaUrl, Error> {
        if url1.name() != url2.name() {
            return Err(Error::AmbiguousReference {
                r#ref: format!("registry mismatch: {} vs {}", url1.name(), url2.name()),
                schema_url1: url1.to_string(),
                schema_url2: url2.to_string(),
            });
        }
        let v1 = url1.semver()?;
        let v2 = url2.semver()?;
        if v1.major == v2.major {
            if v1 > v2 {
                Ok(url1.clone())
            } else {
                Ok(url2.clone())
            }
        } else {
            Err(Error::DuplicateDependency {
                name: url1.name().to_owned(),
                version1: url1.version().to_owned(),
                version2: url2.version().to_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_conflict_same_major() {
        let strategy = UseLatestMajorVersion;
        let u1 = SchemaUrl::try_from("http://example.com/schema/1.0.0").unwrap();
        let u2 = SchemaUrl::try_from("http://example.com/schema/1.2.0").unwrap();

        assert_eq!(strategy.resolve_conflict(&u1, &u2).unwrap(), u2);
        assert_eq!(strategy.resolve_conflict(&u2, &u1).unwrap(), u2);
    }

    #[test]
    fn test_resolve_conflict_diff_major_errors() {
        let strategy = UseLatestMajorVersion;
        let u1 = SchemaUrl::try_from("http://example.com/schema/1.0.0").unwrap();
        let u3 = SchemaUrl::try_from("http://example.com/schema/2.0.0").unwrap();

        assert!(matches!(
            strategy.resolve_conflict(&u1, &u3),
            Err(Error::DuplicateDependency { .. })
        ));
    }

    #[test]
    fn test_resolve_conflict_diff_registry_errors() {
        let strategy = UseLatestMajorVersion;
        let u1 = SchemaUrl::try_from("http://example.com/schema/1.0.0").unwrap();
        let other = SchemaUrl::try_from("http://other.com/schema/1.2.0").unwrap();

        assert!(matches!(
            strategy.resolve_conflict(&u1, &other),
            Err(Error::AmbiguousReference { .. })
        ));
    }
}
