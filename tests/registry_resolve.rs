// SPDX-License-Identifier: Apache-2.0

//! Test the registry resolve command.

use assert_cmd::Command;

#[test]
fn test_published_v2_registry_resolve() {
    let output_dir = tempfile::tempdir().expect("failed to create tempdir");

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("resolve")
        .arg("--v2")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .arg("-o")
        .arg(output_dir.path().join("out.yaml"))
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        result.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
