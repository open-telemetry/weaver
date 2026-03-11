// SPDX-License-Identifier: Apache-2.0

//! Test the completion command.


/// This test checks the CLI interface for the completion command.
#[test]
fn test_generate_completion() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("weaver");
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

/// This test checks the CLI interface for the completion command with --completion-file.
#[test]
fn test_generate_completion_with_file() {
    let mut cmd_stdout = assert_cmd::cargo::cargo_bin_cmd!("weaver");
    let stdout_output = cmd_stdout
        .arg("completion")
        .arg("bash")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");
    assert!(stdout_output.status.success());
    let stdout = String::from_utf8(stdout_output.stdout).expect("Invalid UTF-8");

    let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    let mut cmd_file = assert_cmd::cargo::cargo_bin_cmd!("weaver");
    let file_output = cmd_file
        .arg("completion")
        .arg("bash")
        .arg("--completion-file")
        .arg(temp_file.path())
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");
    assert!(file_output.status.success());

    let file_contents =
        std::fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
    assert_eq!(
        stdout, file_contents,
        "STDOUT and --completion-file outputs do not match"
    );
}
