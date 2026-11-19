// SPDX-License-Identifier: Apache-2.0

//! Shared assertion helpers for `weaver_resolver` integration tests.

use weaver_diff::{canonicalize_json_string, diff_output};
use weaver_resolved_schema::v2::ResolvedTelemetrySchema;

/// Asserts semantic JSON equality of two V2 schemas (ignoring map key order, preserving array order), panicking with a YAML diff on mismatch.
pub fn assert_resolved_v2_schema_eq(
    test_dir: &str,
    expected_schema: &ResolvedTelemetrySchema,
    observed_schema: &ResolvedTelemetrySchema,
    observed_yaml: &str,
) {
    let expected_val = serde_json::to_value(expected_schema)
        .expect("Failed to serialize expected schema to value");
    let observed_val = serde_json::to_value(observed_schema)
        .expect("Failed to serialize observed schema to value");

    if expected_val != observed_val {
        let expected_yaml_reserialized = serde_saphyr::to_string(expected_schema)
            .expect("Failed to reserialize expected schema to YAML");
        panic!(
            "Observed and expected schema don't match for `{test_dir}`.\nDiff from expected:\n{}",
            diff_output(&expected_yaml_reserialized, observed_yaml)
        );
    }
}

/// Asserts canonicalized JSON equality against `expected-errors.json`, panicking with a unified diff on mismatch.
pub fn assert_resolved_errors_eq(
    test_dir: &str,
    expected_errors_json: &str,
    observed_errors_json: &str,
) {
    let canon_expected = canonicalize_json_string(expected_errors_json)
        .expect("Failed to canonicalize expected errors JSON string");
    let canon_observed = canonicalize_json_string(observed_errors_json)
        .expect("Failed to canonicalize observed errors JSON string");

    if canon_expected != canon_observed {
        panic!(
            "Observed and expected errors don't match for `{test_dir}`.\n{}",
            diff_output(expected_errors_json, observed_errors_json)
        );
    }
}

/// Asserts canonicalized JSON equality against `expected-warnings.json`, panicking with a unified diff on mismatch.
pub fn assert_resolved_warnings_eq(
    test_dir: &str,
    expected_warnings_json: &str,
    observed_warnings_json: &str,
) {
    let canon_expected = canonicalize_json_string(expected_warnings_json)
        .expect("Failed to canonicalize expected warnings JSON string");
    let canon_observed = canonicalize_json_string(observed_warnings_json)
        .expect("Failed to canonicalize observed warnings JSON string");

    if canon_expected != canon_observed {
        panic!(
            "Observed and expected warnings don't match for `{test_dir}`.\n{}",
            diff_output(expected_warnings_json, observed_warnings_json)
        );
    }
}
