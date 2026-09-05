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
const V1_REGISTRY: &str = "crates/weaver_live_check/data/model/metrics";

fn run_live_check(extra_args: &[&str]) -> Output {
    run_live_check_on(REGISTRY, extra_args)
}

fn run_live_check_on(registry: &str, extra_args: &[&str]) -> Output {
    let mut cmd = Command::cargo_bin("weaver").expect("weaver binary not found");
    cmd.arg("registry")
        .arg("live-check")
        .arg("-r")
        .arg(registry)
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

/// Writes a `.weaver.toml` holding one matcher and runs live-check against it.
fn run_with_matcher(matcher: &str) -> (Output, tempfile::TempDir) {
    run_with_matcher_on(REGISTRY, &["--v2"], matcher)
}

fn run_with_matcher_on(
    registry: &str,
    extra_args: &[&str],
    matcher: &str,
) -> (Output, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join(".weaver.toml");
    std::fs::write(&config, format!("[[\"live-check\".matchers]]\n{matcher}\n"))
        .expect("write config");
    let mut args = vec!["--config", config.to_str().expect("utf-8 path")];
    args.extend_from_slice(extra_args);
    let out = run_live_check_on(registry, &args);
    (out, dir)
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A `when` that does not compile stops the run, naming the matcher.
#[test]
fn a_matcher_that_does_not_compile_fails_startup() {
    let (out, _dir) = run_with_matcher(
        r#"id = "myapp.broken"
sample_type = "span"
when = 'attributes['"#,
    );
    assert_ne!(exit_code(&out), 0);
    let output = combined(&out);
    assert!(output.contains("myapp.broken"), "got: {output}");
}

/// A `when` reading a variable its sample type does not have stops the run.
#[test]
fn a_matcher_reading_an_unknown_variable_fails_startup() {
    let (out, _dir) = run_with_matcher(
        r#"id = "myapp.wrong.type"
sample_type = "span"
when = 'unit == "s"'"#,
    );
    assert_ne!(exit_code(&out), 0);
    let output = combined(&out);
    assert!(
        output.contains("myapp.wrong.type") && output.contains("unit"),
        "got: {output}"
    );
}

/// A matcher that compiles and resolves lets the run finish.
#[test]
fn a_valid_matcher_does_not_change_the_run() {
    let (out, _dir) = run_with_matcher_on(
        REGISTRY,
        &["--v2", "--fail-on", "none"],
        r#"id = "myapp.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'"#,
    );
    assert_eq!(exit_code(&out), 0, "got: {}", combined(&out));
}

/// A literal `matches` pattern that is not a valid regex stops the run, rather
/// than erroring on every sample.
#[test]
fn a_matcher_with_an_invalid_regex_fails_startup() {
    let (out, _dir) = run_with_matcher(
        r#"id = "myapp.checkout"
sample_type = "span"
when = 'name.matches("^(?<=cart)payment$")'"#,
    );
    assert_ne!(exit_code(&out), 0);
    let output = combined(&out);
    assert!(
        output.contains("myapp.checkout") && output.contains("pattern"),
        "got: {output}"
    );
}

/// A `signal` that is not in the registry stops the run.
#[test]
fn a_matcher_naming_an_unknown_signal_fails_startup() {
    let (out, _dir) = run_with_matcher(
        r#"id = "myapp.checkout"
sample_type = "span"
signal = "myapp.absent""#,
    );
    assert_ne!(exit_code(&out), 0);
    let output = combined(&out);
    assert!(
        output.contains("myapp.absent") && output.contains("span type"),
        "got: {output}"
    );
}

/// Matchers need a v2 registry, and v1 behaviour is unchanged without them.
#[test]
fn a_matcher_against_a_v1_registry_fails_startup() {
    let (out, _dir) = run_with_matcher_on(
        V1_REGISTRY,
        &[],
        r#"id = "myapp.checkout"
sample_type = "span""#,
    );
    assert_ne!(exit_code(&out), 0);
    let output = combined(&out);
    assert!(output.contains("v2 registry"), "got: {output}");
}

#[test]
fn a_v1_registry_without_matchers_still_runs() {
    let out = run_live_check_on(V1_REGISTRY, &["--fail-on", "none"]);
    assert_eq!(exit_code(&out), 0, "got: {}", combined(&out));
}

/// One span sample, for the matcher tests.
const SPAN_INPUT: &str = "crates/weaver_live_check/data/matcher_span.json";
/// Two spans, only one of which carries `myapp.checkout.id`.
const MIXED_SPAN_INPUT: &str = "crates/weaver_live_check/data/matcher_spans_mixed.json";

fn run_with_matcher_on_spans(matcher: &str) -> (Output, tempfile::TempDir) {
    run_with_matcher_on_input(matcher, SPAN_INPUT)
}

fn run_with_matcher_on_input(matcher: &str, input: &str) -> (Output, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join(".weaver.toml");
    std::fs::write(&config, format!("[[\"live-check\".matchers]]\n{matcher}\n"))
        .expect("write config");
    let mut cmd = Command::cargo_bin("weaver").expect("weaver binary not found");
    let out = cmd
        .arg("registry")
        .arg("live-check")
        .args(["-r", REGISTRY, "--v2"])
        .args(["--input-source", input])
        .args(["--input-format", "json"])
        .args(["--format", "json"])
        .args(["--fail-on", "none"])
        .args(["--config", config.to_str().expect("utf-8 path")])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute weaver binary");
    (out, dir)
}

/// A span that matches no matcher says so, showing a gap in the matchers.
#[test]
fn a_span_that_matches_no_matcher_says_nothing_applied() {
    let (out, _dir) = run_with_matcher_on_spans(
        r#"id = "myapp.never"
sample_type = "span"
when = 'name == "no-such-span"'"#,
    );
    let output = combined(&out);
    assert!(output.contains("\"unmatched\": true"), "got: {output}");
}

/// Without matchers the report is what it always was.
#[test]
fn no_matchers_reports_no_match_problem() {
    let out = Command::cargo_bin("weaver")
        .expect("weaver binary not found")
        .arg("registry")
        .arg("live-check")
        .args(["-r", REGISTRY, "--v2"])
        .args(["--input-source", SPAN_INPUT])
        .args(["--input-format", "json"])
        .args(["--format", "json"])
        .args(["--fail-on", "none"])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute weaver binary");
    let output = combined(&out);
    assert!(!output.contains("\"unmatched\": true"), "got: {output}");
}

/// A matcher that applies to no sample is reported, so dead config shows.
#[test]
fn a_matcher_that_applies_to_nothing_warns() {
    let (out, _dir) = run_with_matcher_on_spans(
        r#"id = "myapp.never"
sample_type = "span"
when = 'name == "no-such-span"'"#,
    );
    let output = combined(&out);
    assert!(
        output.contains("Matcher `myapp.never` applied to no samples."),
        "got: {output}"
    );
}

#[test]
fn the_statistics_count_what_each_matcher_matched() {
    let (out, _dir) = run_with_matcher_on_spans(
        r#"id = "myapp.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'"#,
    );
    let output = combined(&out);
    assert!(!output.contains("applied to no samples"), "got: {output}");
    // Streaming mode prints a document per sample, then the statistics.
    let statistics = serde_json::Deserializer::from_slice(&out.stdout)
        .into_iter::<serde_json::Value>()
        .last()
        .expect("something was printed")
        .expect("the statistics parse");
    let matchers = &statistics["matchers"];
    assert_eq!(matchers[0]["id"], "myapp.checkout", "got: {output}");
    assert_eq!(matchers[0]["matched"], 1, "got: {output}");
    assert_eq!(matchers[0]["errors"], 0, "got: {output}");
}

/// A `when` that errors is reported once per matcher, with a count.
#[test]
fn a_when_that_errors_at_runtime_warns_once_with_a_count() {
    let (out, _dir) = run_with_matcher_on_spans(
        r#"id = "myapp.errors"
sample_type = "span"
when = 'instrumentation_scope.name == "nope"'"#,
    );
    let output = combined(&out);
    assert!(
        output.contains("myapp.errors") && output.contains("errored on 1 sample"),
        "got: {output}"
    );
    // The matcher errored, so it matched nothing and the span is unmatched.
    assert!(output.contains("\"unmatched\": true"), "got: {output}");
}

/// An absent key is a CEL error, not `false`, so a `when` indexing an optional
/// attribute matches some samples and errors on others.
#[test]
fn a_matcher_that_both_matches_and_errors_reports_only_the_error_count() {
    let (out, _dir) = run_with_matcher_on_input(
        r#"id = "myapp.optional"
sample_type = "span"
when = 'attributes["myapp.checkout.id"] == "abc"'"#,
        MIXED_SPAN_INPUT,
    );
    let output = combined(&out);
    assert!(
        output.contains("Matcher `myapp.optional` errored on 1 sample(s)."),
        "got: {output}"
    );
    assert!(
        !output.contains("matched none"),
        "the report says it matched one: {output}"
    );
    assert!(!output.contains("applied to no samples"), "got: {output}");
}

/// Renders the ansi template, which `run_live_check_on` mutes, with the colour
/// escapes removed so a label can be matched.
fn run_ansi(extra_args: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("weaver").expect("weaver binary not found");
    let out = cmd
        .arg("registry")
        .arg("live-check")
        .args(["-r", V1_REGISTRY])
        .args(["--input-source", INPUT, "--input-format", "text"])
        .args(["--format", "ansi", "--fail-on", "none"])
        .args(extra_args)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute weaver binary");
    strip_ansi(&combined(&out))
}

/// The text with the colour escapes removed.
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
        } else {
            plain.push(character);
        }
    }
    plain
}

/// The ansi output labels a finding with its level by default.
#[test]
fn the_ansi_output_labels_a_finding_with_its_level() {
    let output = run_ansi(&[]);
    assert!(output.contains("[violation]"), "got: {output}");
    assert!(!output.contains("[missing_attribute]"), "got: {output}");
}

/// `--param show_finding_id=true` labels it with the finding id instead.
#[test]
fn a_param_switches_the_ansi_label_to_the_finding_id() {
    let output = run_ansi(&["-D", "show_finding_id=true"]);
    assert!(output.contains("[missing_attribute]"), "got: {output}");
    assert!(!output.contains("[violation]"), "got: {output}");
}
