// SPDX-License-Identifier: Apache-2.0

//! Test the registry search command.

use assert_cmd::Command;

#[test]
fn test_published_v2_registry_search() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("search")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        !result.status.success(),
        "expected failure for search without --v2, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("search")
        .arg("--v2")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        !result.status.success(),
        "expected failure for search with --v2, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
