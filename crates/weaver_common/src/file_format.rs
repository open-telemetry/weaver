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
    pub minor: u32,
}

impl FileFormat {
    /// Construct a [`FileFormat`] from its parts.
    #[must_use]
    pub const fn new(prefix: &'static str, major: u32, minor: u32) -> Self {
        Self {
            prefix: Cow::Borrowed(prefix),
            major,
            minor,
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
        if found.minor > self.minor {
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
        self.prefix == other.prefix && self.major == other.major && other.minor <= self.minor
    }
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}.{}", self.prefix, self.major, self.minor)
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
        let (prefix, major, minor) = parse_file_format_version(s)
            .ok_or_else(|| format!("invalid file_format '{s}': expected 'prefix/MAJOR.MINOR'"))?;
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
            "pattern": r"^[A-Za-z][A-Za-z0-9_-]*/\d+\.\d+$",
            "description": "A file-format identifier of the form 'prefix/MAJOR.MINOR'.",
        })
    }
}

/// Parses a `"type/MAJOR.MINOR"` file-format string.
///
/// Returns `Some((prefix, major, minor))` or `None` if the string doesn't match the pattern.
#[must_use]
pub fn parse_file_format_version(s: &str) -> Option<(&str, u32, u32)> {
    let (prefix, ver) = s.split_once('/')?;
    let (major_s, minor_s) = ver.split_once('.')?;
    Some((prefix, major_s.parse().ok()?, minor_s.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        let v: FileFormat = "resolved/2.5".parse().expect("parse");
        assert_eq!(v.prefix, "resolved");
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 5);
        assert_eq!(v.to_string(), "resolved/2.5");
    }

    #[test]
    fn from_str_rejects_garbage() {
        assert!("garbage".parse::<FileFormat>().is_err());
        assert!("resolved/2".parse::<FileFormat>().is_err());
        assert!("resolved/x.y".parse::<FileFormat>().is_err());
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

    #[test]
    fn json_schema_shape() {
        let schema = FileFormat::json_schema(&mut SchemaGenerator::default());
        let json = serde_json::to_value(&schema).expect("schema to json");
        assert_eq!(json["type"], "string");
        assert_eq!(json["pattern"], r"^[A-Za-z][A-Za-z0-9_-]*/\d+\.\d+$");
        assert!(json["description"].is_string());

        let pattern = json["pattern"].as_str().expect("pattern is string");
        let re = regex::Regex::new(pattern).expect("pattern compiles");
        assert!(re.is_match("manifest/2.0"));
        assert!(re.is_match("resolved/10.123"));
        assert!(!re.is_match("manifest/2"));
        assert!(!re.is_match("manifest/x.y"));
        assert!(!re.is_match("1bad/2.0"));
    }
}
