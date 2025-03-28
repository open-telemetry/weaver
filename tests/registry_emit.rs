// SPDX-License-Identifier: Apache-2.0

//! Test the registry emit command with registry health.

use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use std::thread::sleep;
use std::time::Duration;
use tempfile::tempdir;

/// This test verifies the roundtrip functionality of registry health and emit commands.
/// This test doesn't count for the coverage report as it runs separate processes.
#[test]
fn test_emit_with_health() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    // Start registry health command as a background process
    let mut health_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
        .args([
            "registry",
            "health",
            "--ingester",
            "ao",
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
        .expect("Failed to start registry health process");

    // Allow health command to initialize
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

    // Wait for health process to terminate due to inactivity timeout
    let status = health_cmd
        .wait()
        .expect("Failed to wait for health process to terminate");

    // Verify the health command exited with a failure status
    assert!(
        status.success(),
        "Health command did not exit successfully: {:?}",
        status
    );

    // Verify the health report in the temporary output directory
    let health_report = fs::read_to_string(format!("{}/health.json", temp_dir_path))
        .expect("Failed to read health report from output directory");
    let health_json: serde_json::Value =
        serde_json::from_str(&health_report).expect("Failed to parse health report JSON");
    /*
    "statistics": {
        "advisory_counts": {
            "improvement": 1,
            "information": 2
        },
        "highest_advisory_counts": {
            "improvement": 1,
            "information": 2
        },
        "no_advice_count": 17,
        "total_advisories": 3,
        "total_attributes": 20
    }
    */
    let statistics = health_json["statistics"].as_object().unwrap();
    let no_advice_count = statistics["no_advice_count"].as_u64().unwrap();
    let total_advisories = statistics["total_advisories"].as_u64().unwrap();
    let total_attributes = statistics["total_attributes"].as_u64().unwrap();

    assert_eq!(no_advice_count, 17);
    assert_eq!(total_advisories, 3);
    assert_eq!(total_attributes, 20);

    // The temporary directory will be automatically cleaned up when temp_dir goes out of scope
}
