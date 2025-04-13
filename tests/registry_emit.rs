// SPDX-License-Identifier: Apache-2.0

//! Test the registry emit command with registry live check.

use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use std::thread::sleep;
use std::time::Duration;
use tempfile::tempdir;

/// This test verifies the roundtrip functionality of registry live check and emit commands.
/// This test doesn't count for the coverage report as it runs separate processes.
#[test]
fn test_emit_with_live_check() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    // Start registry live check command as a background process
    let mut live_check_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
        .args([
            "registry",
            "live-check",
            "-r",
            "crates/weaver_emit/data",
            "--format",
            "json",
            "--output",
            temp_dir_path,
            "--inactivity-timeout",
            "4",
        ])
        .spawn()
        .expect("Failed to start registry live check process");

    // Allow live check command to initialize
    sleep(Duration::from_secs(2));

    // Run registry emit command
    let mut emit_cmd = Command::cargo_bin("weaver").unwrap();
    let emit_output = emit_cmd
        .arg("registry")
        .arg("emit")
        .arg("-r")
        .arg("crates/weaver_emit/data")
        .arg("--skip-policies")
        .arg("--quiet")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("Failed to execute registry emit process");

    // Check that emit command was successful
    assert!(
        emit_output.status.success(),
        "Registry emit command failed: {}",
        String::from_utf8_lossy(&emit_output.stderr)
    );

    // Wait for live check process to terminate due to inactivity timeout
    let status = live_check_cmd
        .wait()
        .expect("Failed to wait for live check process to terminate");

    // Verify the live check command exited with a failure status
    assert!(
        status.success(),
        "Live check command did not exit successfully: {:?}",
        status
    );

    // Verify the live check report in the temporary output directory
    let live_check_report = fs::read_to_string(format!("{}/live_check.json", temp_dir_path))
        .expect("Failed to read live check report from output directory");
    let live_check_json: serde_json::Value =
        serde_json::from_str(&live_check_report).expect("Failed to parse live check report JSON");

    let statistics = live_check_json["statistics"].as_object().unwrap();
    let no_advice_count = statistics["no_advice_count"].as_u64().unwrap();
    let total_advisories = statistics["total_advisories"].as_u64().unwrap();
    let total_entities = statistics["total_entities"].as_u64().unwrap();
    let registry_coverage = statistics["registry_coverage"].as_f64().unwrap();

    assert_eq!(no_advice_count, 24);
    assert_eq!(total_advisories, 3);
    assert_eq!(total_entities, 27);
    assert!(registry_coverage > 0.0);

    // The temporary directory will be automatically cleaned up when temp_dir goes out of scope
}
