// SPDX-License-Identifier: Apache-2.0

//! Test the registry generate command.

use assert_cmd::Command;

/// This test checks the CLI interface for the registry generate command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("generate")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    // The target and the output directory are not provided, so the command should fail.
    assert!(!output.status.success());

    // Test a local semantic convention registry.
    // There are policy violations in this registry.
    // This test should fail with a non-zero exit code and display the policy violations.
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("--quiet")
        .arg("registry")
        .arg("generate")
        .arg("-r")
        .arg("crates/weaver_codegen_test/semconv_registry/")
        .arg("-t")
        .arg("crates/weaver_codegen_test/templates/rust")
        .arg("--diagnostic-format")
        .arg("json")
        .arg("output/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(!output.status.success());

    // We should be able to parse the JSON output from stdout.
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let json_value: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("Invalid JSON");
    // We expect 13 policy violations.
    assert_eq!(json_value.len(), 13);
}
