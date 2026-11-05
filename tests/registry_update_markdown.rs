// SPDX-License-Identifier: Apache-2.0

//! Test diagnostics delivered by the registry update-markdown command.

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

fn update_markdown(markdown_dir: &Path, dry_run: bool) -> Vec<Value> {
    let output = Command::cargo_bin("weaver")
        .expect("weaver binary")
        .args([
            "--quiet",
            "registry",
            "update-markdown",
            "--registry",
            "data/update_markdown/registry",
            "--templates",
            "data/update_markdown/templates",
            "--target",
            "markdown",
            "--diagnostic-format",
            "json",
            "--diagnostic-stdout=true",
        ])
        .arg(markdown_dir)
        .arg(format!("--dry-run={dry_run}"))
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON diagnostics")
}

fn assert_marker_warning(diagnostics: &[Value], path: &Path) {
    let warnings: Vec<_> = diagnostics
        .iter()
        .filter(|message| message["error"].get("InvalidSnippetMarker").is_some())
        .collect();
    assert_eq!(warnings.len(), 1, "{diagnostics:#?}");
    let warning = warnings[0];
    assert_eq!(warning["diagnostic"]["severity"], "Warning");
    assert_eq!(
        warning["error"]["InvalidSnippetMarker"]["path"],
        path.to_str().expect("valid UTF-8 path")
    );
    assert_eq!(warning["error"]["InvalidSnippetMarker"]["line"], 1);
    assert_eq!(
        warning["error"]["InvalidSnippetMarker"]["marker"],
        "<!-- endweaverded -->"
    );
}

#[test]
fn marker_warning_with_stale_snippet() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("markers.md");
    let markdown =
        "<!-- endweaverded -->\n<!-- semconv registry.user_agent -->\nstale\n<!-- endsemconv -->\n";
    std::fs::write(&path, markdown).unwrap();

    let diagnostics = update_markdown(dir.path(), true);
    assert_marker_warning(&diagnostics, &path);
    assert!(diagnostics
        .iter()
        .any(|message| message["error"].get("MarkdownIsNotEqual").is_some()));
    assert_eq!(std::fs::read_to_string(path).unwrap(), markdown);
}

#[test]
fn marker_warning_before_snippet_failure() {
    for (header, trailer, error) in [
        ("semconv does.not.exist", "endsemconv", "GroupNotFound"),
        (
            "weaver .registry.metrics",
            "endweaver",
            "WeaverSnippetNotSupported",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("markers.md");
        let markdown =
            format!("<!-- endweaverded -->\n<!-- {header} -->\nstale\n<!-- {trailer} -->\n");
        std::fs::write(&path, &markdown).unwrap();

        let diagnostics = update_markdown(dir.path(), false);
        assert_marker_warning(&diagnostics, &path);
        assert!(diagnostics
            .iter()
            .any(|message| message["error"].get(error).is_some() || message["error"] == error));
        assert_eq!(std::fs::read_to_string(path).unwrap(), markdown);
    }
}

#[test]
fn marker_warning_with_failure_in_another_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "<!-- endweaverded -->\n").unwrap();
    std::fs::write(
        dir.path().join("b.md"),
        "<!-- semconv does.not.exist -->\nstale\n<!-- endsemconv -->\n",
    )
    .unwrap();

    for dry_run in [false, true] {
        let diagnostics = update_markdown(dir.path(), dry_run);
        assert_marker_warning(&diagnostics, &path);
        assert!(diagnostics
            .iter()
            .any(|message| message["error"].get("GroupNotFound").is_some()));
    }
}

#[test]
fn marker_warning_with_write_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("markers.md");
    std::fs::write(&path, "<!-- endweaverded -->\n").unwrap();
    let permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&path, read_only).unwrap();

    let diagnostics = update_markdown(dir.path(), false);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert_marker_warning(&diagnostics, &path);
    assert!(diagnostics
        .iter()
        .any(|message| message["error"].get("StdIoError").is_some()));
}
