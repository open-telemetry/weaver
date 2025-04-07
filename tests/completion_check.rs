// SPDX-License-Identifier: Apache-2.0

//! Test the completion command.

use assert_cmd::Command;

/// This test checks the CLI interface for the completion command.
#[test]
fn test_generate_completion() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("completion")
        .arg("bash")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(
        stdout.contains("weaver__completion"),
        "Output does not contain expected completion script"
    );
}
