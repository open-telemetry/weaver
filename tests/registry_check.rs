// SPDX-License-Identifier: Apache-2.0

//! Test the registry check command.

use assert_cmd::Command;

/// This test checks the CLI interface for the registry check command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    // Test OTel official semantic convention registry.
    // This test requires internet access to fetch the registry.
    // This registry should always be valid!
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("check")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());

    // Test a local semantic convention registry.
    // There are policy violations in this registry.
    // This test should fail with a non-zero exit code and display the policy violations.
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("--quiet")
        .arg("registry")
        .arg("check")
        .arg("-r")
        .arg("crates/weaver_codegen_test/semconv_registry/")
        .arg("--diagnostic-format")
        .arg("json")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(!output.status.success());

    // We should be able to parse the JSON output from stdout.
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let json_value: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("Invalid JSON");
    // We expect 41 policy violations.
    // - 12 allow_custom_values
    // - 3 missing stability on enum members
    // - 13 violations before resolution
    // - 3 violations for metrics after resolution
    // - 9 violations for http after resolution
    assert_eq!(json_value.len(), 40);
}
