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

/// This test verifies that resource attributes from the original OTLP source are included
/// in emitted findings when using --emit-otlp-logs.
///
/// Triple-weaver setup:
///   weaver1 (emit) --> weaver2 (live-check --emit-otlp-logs) --> weaver3 (live-check --format json)
///
/// weaver3's JSON output is checked for `weaver.finding.resource.attribute.*` attributes
/// on the ingested log samples, confirming the resource attributes flow through.
#[test]
fn test_emit_with_resource_attributes() {
    // Ports for weaver3 (final collector)
    let w3_grpc_port = "5400";
    let w3_admin_port = "5401";
    // Ports for weaver2 (middle live-check with emit)
    let w2_grpc_port = "5402";
    let w2_admin_port = "5403";

    // Temp dir for weaver3's JSON output
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let temp_dir_path = temp_dir
        .path()
        .to_str()
        .expect("Failed to convert temp directory path to string");

    // --- Start weaver3: receives OTLP logs from weaver2, writes JSON report ---
    let mut w3_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
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
            "8",
            "--otlp-grpc-port",
            w3_grpc_port,
            "--admin-port",
            w3_admin_port,
        ])
        .spawn()
        .expect("Failed to start weaver3 (collector)");

    sleep(Duration::from_secs(2));

    // --- Start weaver2: receives OTLP from weaver1, emits findings as OTLP logs to weaver3 ---
    let mut w2_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
        .args([
            "registry",
            "live-check",
            "-r",
            "crates/weaver_emit/data",
            "--output",
            "none",
            "--no-stats",
            "--inactivity-timeout",
            "4",
            "--otlp-grpc-port",
            w2_grpc_port,
            "--admin-port",
            w2_admin_port,
            "--emit-otlp-logs",
            "--otlp-logs-endpoint",
            &format!("http://localhost:{w3_grpc_port}"),
        ])
        .spawn()
        .expect("Failed to start weaver2 (live-check with emit)");

    sleep(Duration::from_secs(2));

    // --- Run weaver1: emits OTLP telemetry to weaver2 ---
    let mut emit_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("weaver"));
    let emit_output = emit_cmd
        .arg("registry")
        .arg("emit")
        .arg("-r")
        .arg("crates/weaver_emit/data")
        .arg("--skip-policies")
        .arg("--quiet")
        .arg("--endpoint")
        .arg(format!("http://localhost:{w2_grpc_port}"))
        .timeout(Duration::from_secs(60))
        .output()
        .expect("Failed to execute weaver1 (emit)");

    assert!(
        emit_output.status.success(),
        "weaver1 emit failed: {}",
        String::from_utf8_lossy(&emit_output.stderr)
    );

    // Wait for weaver2 to finish (inactivity timeout)
    let w2_status = w2_cmd
        .wait()
        .expect("Failed to wait for weaver2 to terminate");
    assert!(
        w2_status.success(),
        "weaver2 did not exit successfully: {w2_status:?}"
    );

    // Wait for weaver3 to finish (inactivity timeout)
    // Note: weaver3 may exit with code 1 because the findings logs it receives
    // (event_name "weaver.live_check.finding") don't exist in the registry, causing violations.
    // This is expected - we only care that it produced output.
    let _w3_status = w3_cmd
        .wait()
        .expect("Failed to wait for weaver3 to terminate");

    // --- Verify weaver3's output contains resource attributes ---
    let report = fs::read_to_string(format!("{temp_dir_path}/live_check.json"))
        .expect("Failed to read weaver3 live check report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report).expect("Failed to parse weaver3 report JSON");

    let samples = report_json["samples"]
        .as_array()
        .expect("Failed to get samples array from weaver3 report");

    // Look for log samples that have weaver.finding.resource.attribute.* attributes
    let mut found_resource_attr = false;
    for sample in samples {
        if let Some(log) = sample.get("log") {
            if let Some(attrs) = log.get("attributes").and_then(|a| a.as_array()) {
                for attr in attrs {
                    if let Some(name) = attr.get("name").and_then(|n| n.as_str()) {
                        if name.starts_with("weaver.finding.resource.attribute.") {
                            found_resource_attr = true;
                            // Verify the service.name resource attribute is present
                            if name == "weaver.finding.resource.attribute.service.name" {
                                assert_eq!(
                                    attr.get("value").and_then(|v| v.as_str()),
                                    Some("weaver"),
                                    "Expected service.name resource attribute to be 'weaver'"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        found_resource_attr,
        "No weaver.finding.resource.attribute.* attributes found in weaver3's log samples. \
         Resource attributes should flow from weaver1 → weaver2 → weaver3."
    );
}
