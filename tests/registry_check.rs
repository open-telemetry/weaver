// SPDX-License-Identifier: Apache-2.0

//! Test the registry check command.

use assert_cmd::Command;
use std::ffi::OsString;
use std::fs;
use std::time::Duration;

/// This test checks the CLI interface for the registry generate command.
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
        .timeout(Duration::from_secs(60))
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
        .timeout(Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(!output.status.success());

    // We should be able to parse the JSON output from stdout.
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let json_value: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("Invalid JSON");
    // We expect 25 policy violations.
    // - 13 violations before resolution
    // - 3 violations for metrics after resolution
    // - 9 violations for http after resolution
    assert_eq!(json_value.len(), 25);
}

/// Test compatibility with older versions
/// This will run a registry check on all the versions in the history folder
#[test]
fn test_history() {
    let root = "history/";
    let mut entries: Vec<OsString> = fs::read_dir(root)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.file_name()))
        .collect();

    entries.sort();

    for entry in entries {
        let path = std::path::Path::new(root).join(&entry);
        if path.is_dir() {
            let path_str = path.to_str().unwrap();
            let mut cmd = Command::cargo_bin("weaver").unwrap();
            let output = cmd
                .arg("--quiet")
                .arg("registry")
                .arg("check")
                .arg("-r")
                .arg(path_str)
                .arg("--diagnostic-format")
                .arg("json")
                .timeout(Duration::from_secs(60))
                .output()
                .expect("failed to execute process");

            assert!(output.status.success(), "Failed for directory: {:?}", path);
            let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
            let json_value: Vec<serde_json::Value> =
                serde_json::from_str(&stdout).expect("Invalid JSON");

            println!("{path_str}, {}", json_value.len());
        }
    }
}
