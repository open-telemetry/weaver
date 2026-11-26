// SPDX-License-Identifier: Apache-2.0

//! Test the warnings from the registry update-markdown command.

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

/// Runs update-markdown on `markdown_dir` and returns its JSON diagnostics.
fn update_markdown(markdown_dir: &Path, dry_run: bool, exit_code: i32) -> Vec<Value> {
    let output = Command::cargo_bin("weaver")
        .expect("weaver binary")
        .args([
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
            "--diagnostic-stdout",
            "true",
        ])
        .arg(markdown_dir)
        .arg(format!("--dry-run={dry_run}"))
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(exit_code), "{stderr}");
    if output.stdout.is_empty() {
        return Vec::new();
    }
    serde_json::from_slice(&output.stdout).expect("JSON diagnostics")
}

/// Returns the path, line and marker of each marker warning.
fn marker_warnings(diagnostics: &[Value]) -> Vec<(String, u64, String)> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["error"].get("InvalidSnippetMarker"))
        .map(|warning| {
            (
                warning["path"].as_str().expect("path").to_owned(),
                warning["line"].as_u64().expect("line"),
                warning["marker"].as_str().expect("marker").to_owned(),
            )
        })
        .collect()
}

fn marker_warning(path: &Path) -> (String, u64, String) {
    (
        path.display().to_string(),
        1,
        "<!-- endweaverded -->".to_owned(),
    )
}

fn has_error(diagnostics: &[Value], error: &str) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic["error"] == error)
}

#[test]
fn marker_only_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("markers.md");
    let markdown = "<!-- endweaverded -->\n";
    std::fs::write(&path, markdown).unwrap();

    for dry_run in [true, false] {
        let diagnostics = update_markdown(dir.path(), dry_run, 0);
        assert_eq!(
            marker_warnings(&diagnostics),
            [marker_warning(&path)],
            "{diagnostics:?}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), markdown);
    }
}

#[test]
fn valid_markers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("valid.md");
    // Git may check this file out with CRLF on Windows, and the update writes LF.
    let markdown = std::fs::read_to_string("data/update_markdown/markdown/templates.md")
        .unwrap()
        .replace("\r\n", "\n");
    std::fs::write(&path, &markdown).unwrap();

    for dry_run in [true, false] {
        let diagnostics = update_markdown(dir.path(), dry_run, 0);
        assert!(marker_warnings(&diagnostics).is_empty(), "{diagnostics:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), markdown);
    }
}

#[test]
fn marker_warning_with_stale_snippet() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("markers.md");
    let markdown =
        "<!-- endweaverded -->\n<!-- semconv registry.user_agent -->\nstale\n<!-- endsemconv -->\n";
    std::fs::write(&path, markdown).unwrap();

    // A dry run fails on the stale snippet without changing the file.
    let diagnostics = update_markdown(dir.path(), true, 1);
    assert!(
        has_error(&diagnostics, "MarkdownNotUpToDate"),
        "{diagnostics:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), markdown);

    // A normal run updates the snippet and still warns about the marker.
    let diagnostics = update_markdown(dir.path(), false, 0);
    assert_eq!(
        marker_warnings(&diagnostics),
        [marker_warning(&path)],
        "{diagnostics:?}"
    );
    assert_ne!(std::fs::read_to_string(&path).unwrap(), markdown);
}
