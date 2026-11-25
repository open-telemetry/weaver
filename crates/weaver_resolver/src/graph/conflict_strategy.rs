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

/// Resolves conflicts within the same registry and major version by selecting the highest semantic
/// version, returning [`Error::AmbiguousReference`] or [`Error::IncompatibleDependencyVersions`]
/// otherwise.
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
            Err(Error::IncompatibleDependencyVersions {
                name: url1.name().to_owned(),
                version1: url1.version().to_owned(),
                version2: url2.version().to_owned(),
            })
        }
    }
}

/// Resolves a version conflict between two competing dependency origins for `reference`,
/// selecting the higher compatible version within the same registry family or returning
/// [`Error::AmbiguousReference`].
pub(crate) fn resolve_origin_conflict(
    reference: &str,
    url1: &SchemaUrl,
    url2: &SchemaUrl,
) -> Result<SchemaUrl, Error> {
    UseLatestMajorVersion
        .resolve_conflict(url1, url2)
        .map_err(|e| match e {
            Error::IncompatibleDependencyVersions { .. } | Error::AmbiguousReference { .. } => {
                Error::AmbiguousReference {
                    r#ref: reference.to_owned(),
                    schema_url1: url1.to_string(),
                    schema_url2: url2.to_string(),
                }
            }
            other => other,
        })
}

/// Selects the single winning item from `matches` using [`resolve_origin_conflict`].
pub(crate) fn pick_winning_by_origin<T>(
    matches: impl IntoIterator<Item = T>,
    reference: &str,
    origin_of: impl Fn(&T) -> &SchemaUrl,
) -> Result<Option<T>, Error> {
    matches
        .into_iter()
        .try_fold(None, |acc: Option<T>, m| match acc {
            None => Ok(Some(m)),
            Some(existing) => {
                let winning_url =
                    resolve_origin_conflict(reference, origin_of(&m), origin_of(&existing))?;
                if winning_url == *origin_of(&existing) {
                    Ok(Some(existing))
                } else {
                    Ok(Some(m))
                }
            }
        })
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
            Err(Error::IncompatibleDependencyVersions { .. })
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
