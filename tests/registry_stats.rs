// SPDX-License-Identifier: Apache-2.0

//! Test the registry stats command.

use assert_cmd::Command;

/// This test checks the CLI interface for the registry stats command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("stats")
        .arg("-r")
        .arg("tests/custom_registry")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        output.status.success(),
        "Process did not exit successfully. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_published_v2_registry_stats() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("stats")
        .arg("--v2")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        result.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
