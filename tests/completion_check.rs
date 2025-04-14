// SPDX-License-Identifier: Apache-2.0

//! Test the completion command.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

/// This test checks the CLI interface for the completion command.
#[test]
fn test_generate_completion() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let completion_file_path = temp_dir.path().join("completion.sh");

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("completion")
        .arg("bash")
        .arg("--completion-file")
        .arg(completion_file_path.to_str().unwrap())
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    assert!(completion_file_path.exists(), "Output file was not created");

    let file_content =
        fs::read_to_string(completion_file_path).expect("Failed to read output file");
    assert!(
        file_content.contains("weaver__completion"),
        "Output file does not contain expected completion script"
    );
}
