// SPDX-License-Identifier: Apache-2.0

//! Test the registry package command.

use assert_cmd::Command;

#[test]
fn test_package_valid_registry() {
    let output_dir = tempfile::tempdir().expect("failed to create tempdir");

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("--quiet")
        .arg("registry")
        .arg("package")
        .arg("--v2")
        .arg("-r")
        .arg("tests/package/valid_registry/")
        .arg("-o")
        .arg(output_dir.path())
        .arg("--resolved-schema-uri")
        .arg("https://test/semconv/1.0.0/resolved.yaml")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        result.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(
        output_dir.path().join("resolved.yaml").exists(),
        "resolved.yaml not written"
    );
    assert!(
        output_dir.path().join("manifest.yaml").exists(),
        "manifest.yaml not written"
    );
}
