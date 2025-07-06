// SPDX-License-Identifier: Apache-2.0

//! Test the registry diff command.

use assert_cmd::Command;
use weaver_version::schema_changes::SchemaChanges;

/// This test checks the CLI interface for the registry diff command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("diff")
        .arg("--baseline-registry")
        .arg("tests/diff/registry_baseline/")
        .arg("-r")
        .arg("tests/diff/registry_head/")
        .arg("--diff-format")
        .arg("json")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());

    // We should be able to parse the JSON output from stdout.
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let schema_changes: SchemaChanges = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("Invalid JSON: {}\n{}", err, &stdout));
    assert_eq!(schema_changes.count_registry_attribute_changes(), 5);
    // We expect 5 types of schema changes and 5 schema changes per telemetry object type.
    // => 5*5 = 25 schema changes.
    assert_eq!(schema_changes.count_changes(), 25);
}
