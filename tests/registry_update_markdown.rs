// SPDX-License-Identifier: Apache-2.0

//! Test the warnings from the registry update-markdown command.

use assert_cmd::Command;
use std::path::Path;

fn update_markdown(markdown_dir: &Path, dry_run: bool, exit_code: i32) -> String {
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
        ])
        .arg(markdown_dir)
        .arg(format!("--dry-run={dry_run}"))
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(exit_code), "{stderr}");
    stderr
}

fn marker_warning(path: &Path) -> String {
    format!(
        "Invalid markdown snippet marker in {}:1: <!-- endweaverded -->",
        path.display()
    )
}

#[test]
fn marker_only_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("markers.md");
    let markdown = "<!-- endweaverded -->\n";
    std::fs::write(&path, markdown).unwrap();

    for dry_run in [true, false] {
        let stderr = update_markdown(dir.path(), dry_run, 0);
        assert!(stderr.contains(&marker_warning(&path)), "{stderr}");
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
        let stderr = update_markdown(dir.path(), dry_run, 0);
        assert!(
            !stderr.contains("Invalid markdown snippet marker"),
            "{stderr}"
        );
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

    let stderr = update_markdown(dir.path(), true, 1);
    assert!(stderr.contains(&marker_warning(&path)), "{stderr}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), markdown);
}
