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

    assert!(output.status.success());
}
