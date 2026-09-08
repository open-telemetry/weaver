// SPDX-License-Identifier: Apache-2.0

//! End-to-end CLI tests for `weaver registry live-check`.
//!
//! Drives the compiled `weaver` binary against the bundled live-check model
//! registry and the `attributes.txt` fixture (which deliberately contains
//! findings at multiple severity levels), and asserts that the process exit
//! code respects the `--fail-on` threshold.

use assert_cmd::Command;
use std::process::Output;

const REGISTRY: &str = "crates/weaver_live_check/model";
const INPUT: &str = "crates/weaver_live_check/data/attributes.txt";

fn run_live_check(extra_args: &[&str]) -> Output {
    let mut cmd = Command::cargo_bin("weaver").expect("weaver binary not found");
    cmd.arg("registry")
        .arg("live-check")
        .arg("-r")
        .arg(REGISTRY)
        .arg("--input-source")
        .arg(INPUT)
        .arg("--input-format")
        .arg("text")
        .arg("--output")
        .arg("none")
        .args(extra_args)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute weaver binary")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().expect("process terminated by signal")
}

/// Default `--fail-on` is `violation`.
#[test]
fn fail_on_default_is_violation() {
    let out = run_live_check(&[]);
    assert_eq!(
        exit_code(&out),
        1,
        "default (violation) must fail when input contains a violation"
    );
}

/// `--fail-on=violation` exits 1 when at least one violation is recorded.
#[test]
fn fail_on_violation_exits_one() {
    let out = run_live_check(&["--fail-on", "violation"]);
    assert_eq!(exit_code(&out), 1);
}

/// Lower thresholds still exit 1 for input that contains a violation, because
/// the gate matches at-or-above the chosen severity.
#[test]
fn fail_on_improvement_exits_one_for_violation_input() {
    let out = run_live_check(&["--fail-on", "improvement"]);
    assert_eq!(exit_code(&out), 1);
}

#[test]
fn fail_on_information_exits_one_for_violation_input() {
    let out = run_live_check(&["--fail-on", "information"]);
    assert_eq!(exit_code(&out), 1);
}

/// `--fail-on=none` disables the severity gate entirely.
#[test]
fn fail_on_none_exits_zero() {
    let out = run_live_check(&["--fail-on", "none"]);
    assert_eq!(
        exit_code(&out),
        0,
        "--fail-on=none must never produce a non-zero exit from findings"
    );
}

/// Unknown values are rejected by clap before any work is done.
#[test]
fn fail_on_invalid_value_is_rejected() {
    let out = run_live_check(&["--fail-on", "bogus"]);
    assert_ne!(exit_code(&out), 0);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid fail-on level") || stderr.contains("invalid value 'bogus'"),
        "expected a clap parse error, got stderr: {stderr}"
    );
}

/// `--no-stats` disables the stats accumulator, so the severity gate can't be
/// evaluated. Preserve the pre-#1473 behavior of always exiting 0 in that
/// mode, but warn the user when they also configured a stats-dependent
/// `--fail-on` value.
#[test]
fn no_stats_with_violation_threshold_warns_and_exits_zero() {
    let out = run_live_check(&["--no-stats", "--fail-on", "violation"]);
    assert_eq!(
        exit_code(&out),
        0,
        "--no-stats must always exit 0 (preserves pre-#1473 behavior)"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("--no-stats")
            && combined.contains("--fail-on")
            && combined.contains("cannot be enforced"),
        "expected a warning explaining the --no-stats / --fail-on conflict, got: {combined}"
    );
}

/// `--no-stats --fail-on=none` is the unambiguous, warning-free combination.
#[test]
fn no_stats_with_none_threshold_is_silent_and_exits_zero() {
    let out = run_live_check(&["--no-stats", "--fail-on", "none"]);
    assert_eq!(exit_code(&out), 0);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !combined.contains("cannot be enforced"),
        "should not warn when --fail-on=none, got: {combined}"
    );
}

/// Advice policies and data can be loaded from a virtual directory archive (`.zip` with `[sub_folder]`).
#[test]
fn live_check_archive_advice_policies_and_data() {
    use std::fs::File;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let archive_path = temp_dir.path().join("bundle.zip");

    let rego_content = r#"
        package live_check_advice

        import rego.v1

        make_advice(advice_type, advice_level, advice_context, message) := {
            "type": "advice",
            "advice_type": advice_type,
            "advice_level": advice_level,
            "advice_context": advice_context,
            "message": message,
        }

        deny contains make_advice(advice_type, advice_level, advice_context, message) if {
            input.sample.attribute
            input.sample.attribute.name == "task.id"
            data.settings.enabled == true
            advice_type := "custom_archive_violation"
            advice_level := "violation"
            advice_context := {"attribute_key": input.sample.attribute.name}
            message := "Custom violation triggered from archive policy and data"
        }
    "#;

    let json_content = r#"
        {
            "enabled": true
        }
    "#;

    let zip_file = File::create(&archive_path).expect("Failed to create archive file");
    let mut zip = ZipWriter::new(zip_file);
    let options = SimpleFileOptions::default();

    zip.start_file("bundle/policies/custom.rego", options)
        .expect("Failed to add rego to zip");
    zip.write_all(rego_content.as_bytes())
        .expect("Failed to write rego");

    zip.start_file("bundle/data/settings.json", options)
        .expect("Failed to add json to zip");
    zip.write_all(json_content.as_bytes())
        .expect("Failed to write json");

    let _ = zip.finish().expect("Failed to finish zip archive");

    let archive_str = archive_path.to_str().expect("valid utf8 path");
    let policies_arg = format!("{}[policies]", archive_str);
    let data_arg = format!("{}[data]", archive_str);

    let out = run_live_check(&[
        "--advice-policies",
        &policies_arg,
        "--advice-data",
        &data_arg,
        "--fail-on",
        "violation",
    ]);

    assert_eq!(
        exit_code(&out),
        1,
        "custom advice finding from archive should trigger violation failure"
    );
}
