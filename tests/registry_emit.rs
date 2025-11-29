// SPDX-License-Identifier: Apache-2.0

//! Test the registry emit command with registry live check.

use std::fs;
use std::process::Command as StdCommand;
use std::thread::sleep;
use std::time::Duration;
use tempfile::tempdir;

/// This test verifies the roundtrip functionality of registry live check and emit commands.
/// This test doesn't count for the coverage report as it runs separate processes.
#[test]
fn test_emit_with_live_check() {
    run_emit_with_live_check_test(false);
}

#[test]
fn test_emit_with_live_check_v2() {
    run_emit_with_live_check_test(true);
}

fn run_emit_with_live_check_test(use_v2: bool) {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let temp_dir_path = temp_dir
        .path()
        .to_str()
        .expect("Failed to convert temp directory path to string");

    // Build the arguments for live check command
    let mut live_check_args = vec![
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
    ];
    if use_v2 {
        live_check_args.push("--v2");
        live_check_args.push("--otlp-grpc-port");
        live_check_args.push("5300");
        live_check_args.push("--admin-port");
        live_check_args.push("5301");
    } else {
        live_check_args.push("--otlp-grpc-port");
        live_check_args.push("5200");
        live_check_args.push("--admin-port");
        live_check_args.push("5201");
    }

    // Start registry live check command as a background process
    let mut live_check_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
        .args(&live_check_args)
        .spawn()
        .expect("Failed to start registry live check process");

    // Allow live check command to initialize
    sleep(Duration::from_secs(2));

    // Run registry emit command
    let mut emit_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("weaver"));
    let mut emit_args = emit_cmd
        .arg("registry")
        .arg("emit")
        .arg("-r")
        .arg("crates/weaver_emit/data")
        .arg("--skip-policies")
        .arg("--quiet");

    if use_v2 {
        emit_args = emit_args.arg("--v2");
        emit_args = emit_args.arg("--endpoint");
        emit_args = emit_args.arg("http://localhost:5300");
    } else {
        emit_args = emit_args.arg("--endpoint");
        emit_args = emit_args.arg("http://localhost:5200");
    }

    let emit_output = emit_args
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
        "Live check command did not exit successfully: {status:?}"
    );

    // Verify the live check report in the temporary output directory
    let live_check_report = fs::read_to_string(format!("{temp_dir_path}/live_check.json"))
        .expect("Failed to read live check report from output directory");
    let live_check_json: serde_json::Value =
        serde_json::from_str(&live_check_report).expect("Failed to parse live check report JSON");
    // println!("{live_check_json:#?}");
    let statistics = live_check_json["statistics"]
        .as_object()
        .expect("Failed to get statistics object from live check report");
    let no_advice_count = statistics["no_advice_count"]
        .as_u64()
        .expect("Failed to get no_advice_count as u64");
    let total_advisories = statistics["total_advisories"]
        .as_u64()
        .expect("Failed to get total_advisories as u64");
    let total_entities = statistics["total_entities"]
        .as_u64()
        .expect("Failed to get total_entities as u64");
    let registry_coverage = statistics["registry_coverage"]
        .as_f64()
        .expect("Failed to get registry_coverage as f64");

    assert_eq!(no_advice_count, 59);
    assert_eq!(total_advisories, 14);
    assert_eq!(total_entities, 73);
    assert!(registry_coverage > 0.7);

    // The temporary directory will be automatically cleaned up when temp_dir goes out of scope
}
