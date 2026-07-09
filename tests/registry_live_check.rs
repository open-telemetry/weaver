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
