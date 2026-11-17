// SPDX-License-Identifier: Apache-2.0

//! Test the registry stats command.

use std::path::Path;

use assert_cmd::Command;

/// This test checks the CLI interface for the registry stats command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("stats")
        .arg("-r")
        .arg("tests/custom_registry")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        output.status.success(),
        "Process did not exit successfully. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_published_v2_registry_stats() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("stats")
        .arg("--v2")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        result.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

const REGISTRY: &str = "crates/weaver_resolver/data/registry-test-1-single-attr-ref/registry";

fn assert_stats_load_registry(cmd: &mut Command, registry: &Path) {
    let output = cmd
        .arg("registry")
        .arg("stats")
        .arg("-r")
        .arg(registry)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("- 2 groups"));
}

#[test]
fn test_registry_in_current_dir() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    assert_stats_load_registry(cmd.current_dir(REGISTRY), Path::new("."));
}

#[cfg(unix)]
#[test]
fn test_symlink_to_hidden_registry() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let hidden = dir.path().join(".registry");
    std::fs::create_dir_all(&hidden).expect("failed to create hidden dir");
    for file in ["metrics-messaging.yaml", "registry-messaging.yaml"] {
        let _ = std::fs::copy(format!("{REGISTRY}/{file}"), hidden.join(file))
            .expect("failed to copy registry file");
    }
    let link = dir.path().join("registry");
    std::os::unix::fs::symlink(&hidden, &link).expect("failed to create symlink");

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    assert_stats_load_registry(&mut cmd, &link);
}
