// SPDX-License-Identifier: Apache-2.0

//! Detect fields the typed schema didn't recognize by comparing the user's raw
//! YAML against the typed form after a deserialize/serialize round-trip.
//!
//! # How it works
//!
//! The caller parses the same YAML document twice:
//! 1. As `serde_yaml::Value` — keeps every key the user wrote, including typos.
//! 2. Into the typed schema structs, then re-serializes back to
//!    `serde_yaml::Value`.
//!
//! Because the schema types do not set `#[serde(deny_unknown_fields)]`, serde
//! silently drops keys it doesn't recognize during step (2). The re-serialized
//! form therefore contains only known fields, and walking the two trees in
//! parallel surfaces any key present in (1) but missing from (2) — i.e. a key
//! the typed schema didn't recognize.
//!
//! Tolerating unknown keys at deserialize time is what makes newer-minor
//! schemas forward-compatible; this diff is how we report them as errors on
//! known/past-minor schemas, where unknowns indicate a typo or a misdeclared
//! file_format.
//!
//! # False-positive guard
//!
//! Many fields use `#[serde(skip_serializing_if = "...")]` to omit defaults from
//! the re-serialized form. A user who writes those defaults explicitly (e.g.
//! `note: ""`) would otherwise look like they wrote an unknown key, since the key
//! is on the raw side but not the normalized side. A default-equivalent guard
//! suppresses those — null, empty string, empty seq, empty map are all treated as
//! "would have been skipped anyway".

use crate::Error;
use serde::Serialize;
use weaver_common::file_format::FileFormat;

/// Validates `found` against `file_format` (the expected version), then — if the
/// file's minor is known to this build — diffs `raw` against the re-serialized `typed`
/// form and returns [`Error::UnexpectedFields`] if any key in `raw` was dropped by the
/// typed schema. Newer-minor files short-circuit (forward compat — unknowns are tolerated).
///
/// `typed` is the deserialized struct; this helper round-trips it to a YAML tree
/// internally. The round-trip cannot fail in practice — `typed` was just produced
/// by a successful deserialize, and `serde_yaml::Value` is the most permissive
/// shape we can serialize into.
pub fn check<T: Serialize>(
    raw: &serde_yaml::Value,
    typed: &T,
    file_format: &FileFormat,
    found: &FileFormat,
    path: &std::path::Path,
) -> Result<(), Error> {
    file_format.validate(found, path)?;
    if !file_format.is_known_minor(found) {
        return Ok(());
    }
    let normalized = serde_yaml::to_value(typed)
        .expect("re-serializing a deserialized typed struct to serde_yaml::Value cannot fail");
    let unexpected = collect_paths(raw, &normalized);
    if unexpected.is_empty() {
        return Ok(());
    }
    Err(Error::UnexpectedFields {
        path_or_url: path.display().to_string(),
        file_format: file_format.clone(),
        fields: unexpected,
    })
}

/// Returns dotted paths (e.g. `groups[2].attributes[0].typo`) of every key
/// present in `raw` but missing or non-default in `normalized`. See module
/// docs for the algorithm. Most callers should use [`check`]; this lower-level
/// entry point is exposed for diagnostic use.
#[must_use]
pub fn collect_paths(raw: &serde_yaml::Value, normalized: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    diff("", raw, normalized, &mut out);
    out
}

fn diff(
    path: &str,
    raw: &serde_yaml::Value,
    normalized: &serde_yaml::Value,
    out: &mut Vec<String>,
) {
    match (raw, normalized) {
        // Walk only `raw_map`'s keys: a key in `norm_map` but not `raw_map`
        // means a default the user didn't write, which is not a typo.
        (serde_yaml::Value::Mapping(raw_map), serde_yaml::Value::Mapping(norm_map)) => {
            for (key, raw_val) in raw_map {
                // Schema only uses string keys; skip exotic YAML key types.
                let Some(key_str) = key.as_str() else {
                    continue;
                };
                let sub_path = if path.is_empty() {
                    key_str.to_owned()
                } else {
                    format!("{path}.{key_str}")
                };
                match norm_map.get(key) {
                    // Key survived the round-trip; recurse, unknowns may hide deeper.
                    Some(norm_val) => diff(&sub_path, raw_val, norm_val, out),
                    // Key was dropped by serde and the value isn't a default
                    // that would have been stripped by `skip_serializing_if`.
                    None if !is_default_equivalent(raw_val) => out.push(sub_path),
                    None => {}
                }
            }
        }
        // Pairwise diff. Lengths normally match; if raw is longer, report the
        // extras so dropped elements don't slip through.
        (serde_yaml::Value::Sequence(raw_seq), serde_yaml::Value::Sequence(norm_seq)) => {
            for (i, (raw_val, norm_val)) in raw_seq.iter().zip(norm_seq.iter()).enumerate() {
                diff(&format!("{path}[{i}]"), raw_val, norm_val, out);
            }
            for (i, raw_val) in raw_seq.iter().enumerate().skip(norm_seq.len()) {
                if !is_default_equivalent(raw_val) {
                    out.push(format!("{path}[{i}]"));
                }
            }
        }
        // Leaves and shape mismatches: nothing to recurse into.
        _ => {}
    }
}

fn is_default_equivalent(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => true,
        serde_yaml::Value::Sequence(seq) => seq.is_empty(),
        serde_yaml::Value::Mapping(map) => map.is_empty(),
        serde_yaml::Value::String(s) => s.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> serde_yaml::Value {
        serde_yaml::from_str(s).expect("test yaml should parse")
    }

    fn fmt() -> FileFormat {
        FileFormat::new("resolved", 2, 3)
    }

    // ---- collect_paths ----

    #[test]
    fn collect_paths_returns_empty_when_trees_match() {
        let raw = yaml("name: foo\nvalue: 1");
        let normalized = yaml("name: foo\nvalue: 1");
        assert!(collect_paths(&raw, &normalized).is_empty());
    }

    #[test]
    fn collect_paths_reports_extra_top_level_key() {
        let raw = yaml("name: foo\ntyp0: extra");
        let normalized = yaml("name: foo");
        assert_eq!(collect_paths(&raw, &normalized), vec!["typ0".to_owned()]);
    }

    #[test]
    fn collect_paths_reports_nested_key_with_dotted_path() {
        let raw = yaml("outer:\n  inner: ok\n  oops: bad");
        let normalized = yaml("outer:\n  inner: ok");
        assert_eq!(
            collect_paths(&raw, &normalized),
            vec!["outer.oops".to_owned()]
        );
    }

    #[test]
    fn collect_paths_reports_key_inside_sequence_element_with_index() {
        let raw = yaml("items:\n  - id: a\n    extra: x\n  - id: b");
        let normalized = yaml("items:\n  - id: a\n  - id: b");
        assert_eq!(
            collect_paths(&raw, &normalized),
            vec!["items[0].extra".to_owned()]
        );
    }

    #[test]
    fn collect_paths_suppresses_default_equivalents() {
        // Keys whose values would have been stripped by `skip_serializing_if`
        // are not reported even when absent from `normalized`.
        let raw = yaml("a: ~\nb: ''\nc: []\nd: {}");
        let normalized = yaml("{}");
        assert!(collect_paths(&raw, &normalized).is_empty());
    }

    #[test]
    fn collect_paths_ignores_value_only_diffs() {
        // Same keys, different scalar values — keys present in both, nothing to report.
        let raw = yaml("name: foo");
        let normalized = yaml("name: bar");
        assert!(collect_paths(&raw, &normalized).is_empty());
    }

    #[test]
    fn collect_paths_reports_dropped_trailing_sequence_elements() {
        // Simulates a (de)serializer that drops trailing elements: raw has 3
        // elements, normalized has 1. Trailing raw elements with non-default
        // values are reported by index; default-equivalent trailing elements
        // are suppressed by the same guard used for map values.
        let raw = yaml("items:\n  - id: a\n  - id: b\n  - {}\n  - id: d");
        let normalized = yaml("items:\n  - id: a");
        assert_eq!(
            collect_paths(&raw, &normalized),
            vec!["items[1]".to_owned(), "items[3]".to_owned()]
        );
    }

    #[test]
    fn collect_paths_reports_multiple_unexpected_in_order() {
        let raw = yaml("a: 1\nb: 2\nc: 3");
        let normalized = yaml("b: 2");
        assert_eq!(
            collect_paths(&raw, &normalized),
            vec!["a".to_owned(), "c".to_owned()]
        );
    }

    // ---- check ----

    fn path() -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/t.yaml")
    }

    fn parsed(s: &str) -> FileFormat {
        s.parse().expect("test file_format should parse")
    }

    #[test]
    fn check_returns_ok_when_no_unexpected_on_known_minor() {
        let raw = yaml("a: 1");
        let normalized = yaml("a: 1");
        check(&raw, &normalized, &fmt(), &parsed("resolved/2.3"), &path()).expect("no diff = Ok");
    }

    #[test]
    fn check_flags_unexpected_on_current_minor() {
        let raw = yaml("a: 1\ntyp0: bad");
        let normalized = yaml("a: 1");
        match check(&raw, &normalized, &fmt(), &parsed("resolved/2.3"), &path()) {
            Err(Error::UnexpectedFields { fields, .. }) => {
                assert_eq!(fields, vec!["typ0".to_owned()]);
            }
            other => panic!("expected UnexpectedFields, got: {other:?}"),
        }
    }

    #[test]
    fn check_flags_unexpected_on_older_minor() {
        // Older minors are still "known" to this binary, so unknowns there are
        // typos, not forward-compat tolerance.
        let raw = yaml("a: 1\ntyp0: bad");
        let normalized = yaml("a: 1");
        match check(&raw, &normalized, &fmt(), &parsed("resolved/2.0"), &path()) {
            Err(Error::UnexpectedFields { fields, .. }) => {
                assert_eq!(fields, vec!["typ0".to_owned()]);
            }
            other => panic!("expected UnexpectedFields, got: {other:?}"),
        }
    }

    #[test]
    fn check_short_circuits_on_newer_minor() {
        // Forward-compat: newer minor may carry fields this binary doesn't know.
        let raw = yaml("a: 1\nfuture_field: x");
        let normalized = yaml("a: 1");
        check(&raw, &normalized, &fmt(), &parsed("resolved/2.99"), &path())
            .expect("newer minor tolerates unknowns");
    }

    #[test]
    fn check_rejects_different_prefix() {
        let raw = yaml("a: 1\ntyp0: bad");
        let normalized = yaml("a: 1");
        let err = check(&raw, &normalized, &fmt(), &parsed("manifest/2.3"), &path())
            .expect_err("different prefix should be rejected");
        assert!(matches!(
            err,
            Error::VirtualDirectoryError(weaver_common::Error::UnrecognizedFileFormat { .. })
        ));
    }

    #[test]
    fn check_rejects_incompatible_major() {
        let raw = yaml("a: 1\ntyp0: bad");
        let normalized = yaml("a: 1");
        let err = check(&raw, &normalized, &fmt(), &parsed("resolved/3.0"), &path())
            .expect_err("incompatible major should be rejected");
        assert!(matches!(
            err,
            Error::VirtualDirectoryError(
                weaver_common::Error::IncompatibleFileFormatMajorVersion { .. }
            )
        ));
    }

    #[test]
    fn check_propagates_path_and_file_format_in_error() {
        let raw = yaml("name: ok\ntyp0: bad\nnested:\n  deeper: bad");
        let normalized = yaml("name: ok\nnested: {}");
        let p = std::path::PathBuf::from("/some/where.yaml");
        match check(&raw, &normalized, &fmt(), &parsed("resolved/2.3"), &p) {
            Err(Error::UnexpectedFields {
                path_or_url,
                file_format,
                fields,
            }) => {
                assert_eq!(path_or_url, "/some/where.yaml");
                assert_eq!(file_format, fmt());
                assert_eq!(fields, vec!["typ0".to_owned(), "nested.deeper".to_owned()]);
            }
            other => panic!("expected UnexpectedFields, got: {other:?}"),
        }
    }
}
