// SPDX-License-Identifier: Apache-2.0

//! File-format version identifiers of the form `"prefix/MAJOR.MINOR"`,
//! shared by manifest and resolved-schema files.

use crate::Error;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::str::FromStr;

/// A file-format identifier of the form `"prefix/MAJOR.MINOR"`, e.g. `"manifest/2.0"`.
///
/// Serializes and deserializes as the string form (e.g. `"resolved/2.0"`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FileFormat {
    /// The format prefix (e.g. `"manifest"`, `"resolved"`).
    pub prefix: Cow<'static, str>,
    /// The major version.
    pub major: u32,
    /// The minor version.
    pub minor: Option<u32>,
}

impl FileFormat {
    /// Construct a [`FileFormat`] from its parts.
    #[must_use]
    pub const fn new(prefix: &'static str, major: u32, minor: u32) -> Self {
        Self {
            prefix: Cow::Borrowed(prefix),
            major,
            minor: Some(minor),
        }
    }

    /// Construct a [`FileFormat`] declared without a minor (e.g. `"definition/2"`).
    #[must_use]
    pub const fn without_minor(prefix: &'static str, major: u32) -> Self {
        Self {
            prefix: Cow::Borrowed(prefix),
            major,
            minor: None,
        }
    }

    /// Validate `found` against this expected version, logging a warning for newer-minor
    /// versions and returning a structured [`Error`] for incompatible prefix or major.
    ///
    /// Both inputs are already parsed [`FileFormat`]s — typically `self` is the build's
    /// expected constant and `found` comes from deserializing the file's `file_format` field.
    pub fn validate(&self, found: &FileFormat, path: &std::path::Path) -> Result<(), Error> {
        if found.prefix != self.prefix {
            return Err(Error::UnrecognizedFileFormat {
                path: path.to_path_buf(),
                found: found.to_string(),
                expected: self.clone(),
            });
        }
        if found.major != self.major {
            return Err(Error::IncompatibleFileFormatMajorVersion {
                path: path.to_path_buf(),
                found: found.clone(),
                expected: self.clone(),
            });
        }
        let is_newer_minor = match (self.minor, found.minor) {
            (Some(s), Some(f)) => f > s,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if is_newer_minor {
            log::warn!(
                "File format '{found}' in {path:?} is newer than the supported format '{}'. \
                 Some fields may be ignored — update weaver to a newer version to consume them.",
                self,
            );
        }
        Ok(())
    }

    /// Returns `true` when `other` is a known minor version of `self` (the expected minor
    /// or older, with matching prefix and major).
    ///
    /// Callers use this after [`Self::validate`] succeeds to decide whether to reject
    /// unknown fields (known minor: yes; newer minor: tolerate).
    #[must_use]
    pub fn is_known_minor(&self, other: &FileFormat) -> bool {
        if self.prefix != other.prefix || self.major != other.major {
            return false;
        }
        match (self.minor, other.minor) {
            (Some(s), Some(o)) => o <= s,
            (None, None) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.minor {
            Some(minor) => write!(f, "{}/{}.{}", self.prefix, self.major, minor),
            None => write!(f, "{}/{}", self.prefix, self.major),
        }
    }
}

impl std::fmt::Debug for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl FromStr for FileFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix, major, minor) = parse_file_format_version(s).ok_or_else(|| {
            format!("invalid file_format '{s}': expected 'prefix/MAJOR' or 'prefix/MAJOR.MINOR'")
        })?;
        Ok(Self {
            prefix: Cow::Owned(prefix.to_owned()),
            major,
            minor,
        })
    }
}

impl Serialize for FileFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FileFormat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(de::Error::custom)
    }
}

impl JsonSchema for FileFormat {
    fn schema_name() -> Cow<'static, str> {
        "FileFormat".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::FileFormat").into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": r"^[A-Za-z][A-Za-z0-9_-]*/\d+(\.\d+)?$",
            "description": "A file-format identifier of the form 'prefix/MAJOR' or 'prefix/MAJOR.MINOR'.",
        })
    }
}

/// Prefixes that may be written without a `.MINOR`. Every other prefix must include a minor,
/// otherwise serde would silently accept malformed input like `manifest/2`.
const NO_MINOR_PREFIXES: &[&str] = &["definition"];

/// Parses `"prefix/MAJOR"` or `"prefix/MAJOR.MINOR"`. The minor-less form is only accepted
/// for 'definition'.
#[must_use]
pub fn parse_file_format_version(s: &str) -> Option<(&str, u32, Option<u32>)> {
    let (prefix, ver) = s.split_once('/')?;
    if let Some((major_s, minor_s)) = ver.split_once('.') {
        Some((prefix, major_s.parse().ok()?, Some(minor_s.parse().ok()?)))
    } else if NO_MINOR_PREFIXES.contains(&prefix) {
        Some((prefix, ver.parse().ok()?, None))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        let v: FileFormat = "resolved/2.5".parse().expect("parse");
        assert_eq!(v.prefix, "resolved");
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, Some(5));
        assert_eq!(v.to_string(), "resolved/2.5");
    }

    #[test]
    fn from_str_rejects_garbage() {
        assert!("garbage".parse::<FileFormat>().is_err());
        assert!("resolved/2".parse::<FileFormat>().is_err());
        assert!("resolved/x.y".parse::<FileFormat>().is_err());
        assert!("resolved/x".parse::<FileFormat>().is_err());
    }

    #[test]
    fn parse_no_minor_uses_none() {
        let v: FileFormat = "definition/2".parse().expect("parse");
        assert_eq!(v.prefix, "definition");
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, None);
        assert_eq!(v.to_string(), "definition/2");
    }

    #[test]
    fn no_minor_is_never_known_against_a_real_minor() {
        let expected = FileFormat::new("definition", 2, 5);
        let found_no_minor = FileFormat::without_minor("definition", 2);
        assert!(!expected.is_known_minor(&found_no_minor));
    }

    #[test]
    fn serialize_as_string() {
        let v = FileFormat::new("manifest", 2, 0);
        let json = serde_json::to_string(&v).expect("serialize");
        assert_eq!(json, r#""manifest/2.0""#);
    }

    #[test]
    fn deserialize_from_string() {
        let v: FileFormat = serde_json::from_str(r#""resolved/2.0""#).expect("deserialize");
        assert_eq!(v, FileFormat::new("resolved", 2, 0));
    }

    #[test]
    fn deserialize_rejects_non_string() {
        let err = serde_json::from_str::<FileFormat>(r#"{"prefix":"resolved"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn debug_matches_display() {
        let v = FileFormat::new("manifest", 2, 0);
        assert_eq!(format!("{v:?}"), "manifest/2.0");
        assert_eq!(format!("{v:?}"), format!("{v}"));
    }

    #[test]
    fn json_schema_metadata() {
        assert_eq!(FileFormat::schema_name(), "FileFormat");
        assert!(FileFormat::schema_id().ends_with("::FileFormat"));
    }

    // ---- validate ----

    fn p() -> &'static std::path::Path {
        std::path::Path::new("/tmp/t.yaml")
    }

    #[test]
    fn validate_ok_when_equal() {
        let expected = FileFormat::new("resolved", 2, 3);
        let found = FileFormat::new("resolved", 2, 3);
        expected.validate(&found, p()).expect("equal versions");
    }

    #[test]
    fn validate_ok_on_older_minor() {
        let expected = FileFormat::new("resolved", 2, 5);
        let found = FileFormat::new("resolved", 2, 1);
        expected.validate(&found, p()).expect("older minor OK");
    }

    #[test]
    fn validate_ok_on_newer_minor() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("resolved", 2, 99);
        expected
            .validate(&found, p())
            .expect("newer minor returns Ok");
    }

    #[test]
    fn validate_rejects_prefix_mismatch() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("manifest", 2, 0);
        let err = expected
            .validate(&found, p())
            .expect_err("prefix mismatch should be rejected");
        match err {
            Error::UnrecognizedFileFormat {
                found,
                expected: exp,
                ..
            } => {
                assert_eq!(found, "manifest/2.0");
                assert_eq!(exp.to_string(), "resolved/2.0");
            }
            other => panic!("expected UnrecognizedFileFormat, got: {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_smaller_major() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("resolved", 1, 0);
        let err = expected
            .validate(&found, p())
            .expect_err("smaller major should be rejected");
        assert!(matches!(
            err,
            Error::IncompatibleFileFormatMajorVersion { .. }
        ));
    }

    #[test]
    fn validate_rejects_larger_major() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("resolved", 3, 0);
        let err = expected
            .validate(&found, p())
            .expect_err("larger major should be rejected");
        assert!(matches!(
            err,
            Error::IncompatibleFileFormatMajorVersion { .. }
        ));
    }

    #[test]
    fn validate_treats_found_no_minor_as_newer_when_expected_has_minor() {
        // Unreachable via the parser; pins behavior when `without_minor` is used directly.
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::without_minor("resolved", 2);
        expected
            .validate(&found, p())
            .expect("found-no-minor passes (treated as newer)");
    }

    #[test]
    fn validate_ok_when_expected_has_no_minor() {
        let expected = FileFormat::without_minor("definition", 2);
        let with_minor = FileFormat::new("definition", 2, 5);
        let no_minor = FileFormat::without_minor("definition", 2);
        expected
            .validate(&with_minor, p())
            .expect("expected-no-minor + found-with-minor OK");
        expected.validate(&no_minor, p()).expect("both no-minor OK");
    }

    // ---- is_known_minor ----

    #[test]
    fn is_known_minor_true_on_equal() {
        let expected = FileFormat::new("resolved", 2, 3);
        let found = FileFormat::new("resolved", 2, 3);
        assert!(expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_true_on_older() {
        let expected = FileFormat::new("resolved", 2, 3);
        let found = FileFormat::new("resolved", 2, 0);
        assert!(expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_false_on_newer() {
        let expected = FileFormat::new("resolved", 2, 3);
        let found = FileFormat::new("resolved", 2, 99);
        assert!(!expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_false_on_different_prefix() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("manifest", 2, 0);
        assert!(!expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_false_on_different_major() {
        let expected = FileFormat::new("resolved", 2, 0);
        let found = FileFormat::new("resolved", 3, 0);
        assert!(!expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_true_when_both_no_minor() {
        let expected = FileFormat::without_minor("definition", 2);
        let found = FileFormat::without_minor("definition", 2);
        assert!(expected.is_known_minor(&found));
    }

    #[test]
    fn is_known_minor_false_when_expected_no_minor_but_found_has_minor() {
        let expected = FileFormat::without_minor("definition", 2);
        let found = FileFormat::new("definition", 2, 5);
        assert!(!expected.is_known_minor(&found));
    }

    #[test]
    fn json_schema_shape() {
        let schema = FileFormat::json_schema(&mut SchemaGenerator::default());
        let json = serde_json::to_value(&schema).expect("schema to json");
        assert_eq!(json["type"], "string");
        assert_eq!(json["pattern"], r"^[A-Za-z][A-Za-z0-9_-]*/\d+(\.\d+)?$");
        assert!(json["description"].is_string());

        let pattern = json["pattern"].as_str().expect("pattern is string");
        let re = regex::Regex::new(pattern).expect("pattern compiles");
        assert!(re.is_match("manifest/2.0"));
        assert!(re.is_match("resolved/10.123"));
        assert!(re.is_match("definition/2"));
        assert!(!re.is_match("manifest/x.y"));
        assert!(!re.is_match("1bad/2.0"));
    }
}
