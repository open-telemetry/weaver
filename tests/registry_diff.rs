// SPDX-License-Identifier: Apache-2.0

//! Test the registry diff command.

use assert_cmd::Command;
use weaver_version::schema_changes::SchemaChanges;

fn v2_diff(head: &std::path::Path, format: &str) -> Command {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let _ = cmd
        .args(["registry", "diff", "--v2", "--baseline-registry"])
        .arg("tests/published_v2_registry")
        .arg("--registry")
        .arg(head)
        .args(["--diff-format", format])
        .timeout(std::time::Duration::from_secs(60));
    cmd
}

fn assert_v2_contract(value: &serde_json::Value, head_url: &str) {
    assert_eq!(value["head_schema_url"], head_url);
    assert_eq!(value["baseline_schema_url"], "https://test/schemas/1.0.0");
    // Round-tripping catches omitted fields and legacy template-only fields.
    let changes: weaver_version::v2::SchemaChanges =
        serde_json::from_value(value.clone()).expect("valid V2 diff");
    assert_eq!(serde_json::to_value(changes).unwrap(), *value);
}

#[test]
fn test_v2_diff_machine_formats() {
    for format in ["json", "JSON", "yaml", "jsonl"] {
        let output = v2_diff(std::path::Path::new("tests/published_v2_registry"), format)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = if format == "yaml" {
            serde_saphyr::from_str(&text).unwrap()
        } else {
            serde_json::from_str(&text).unwrap()
        };
        assert_v2_contract(&value, "https://test/schemas/1.0.0");
        assert!(value["registry"]
            .as_object()
            .unwrap()
            .values()
            .all(|changes| changes.as_array().unwrap().is_empty()));
        if format == "jsonl" {
            assert_eq!(text.lines().count(), 1);
        }
    }
}

fn changed_v2_registry() -> tempfile::TempDir {
    let head = tempfile::tempdir().unwrap();
    for file in ["manifest.yaml", "resolved.yaml"] {
        let source = std::fs::read_to_string(format!("tests/published_v2_registry/{file}"))
            .unwrap()
            .replace("https://test/schemas/1.0.0", "https://test/schemas/1.1.0")
            .replace("test.attr", "test.new_attr")
            .replace("entities: []", "entities:\n  - type: test.entity\n    identity: [0]\n    brief: A test entity.\n    stability: stable");
        std::fs::write(head.path().join(file), source).unwrap();
    }
    head
}

#[test]
fn test_v2_diff_nonempty_file_output() {
    let head = changed_v2_registry();
    for format in ["json", "yaml", "jsonl"] {
        let output = tempfile::tempdir().unwrap();
        let _ = v2_diff(head.path(), format)
            .arg("--output")
            .arg(output.path())
            .assert()
            .success();
        let text = std::fs::read_to_string(output.path().join(format!("diff.{format}"))).unwrap();
        if format == "jsonl" {
            assert_eq!(text.lines().count(), 1);
        }
        let value: serde_json::Value = if format == "yaml" {
            serde_saphyr::from_str(&text).unwrap()
        } else {
            serde_json::from_str(&text).unwrap()
        };
        assert_v2_contract(&value, "https://test/schemas/1.1.0");
        let changes = value["registry"]["attribute_changes"].as_array().unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes.contains(&serde_json::json!({"type": "added", "name": "test.new_attr"})));
        assert!(changes.contains(&serde_json::json!({"type": "removed", "name": "test.attr"})));
        assert_eq!(
            value["registry"]["entity_changes"],
            serde_json::json!([
                {"type": "added", "name": "test.entity"}
            ])
        );
    }
}

#[test]
fn test_v2_diff_template_context() {
    let head = changed_v2_registry();
    for format in ["ansi", "markdown"] {
        let output = v2_diff(head.path(), format)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("1.0.0"), "{format}: {text}");
        assert!(text.contains("1.1.0"), "{format}: {text}");
        assert!(text.contains("test.new_attr"), "{format}: {text}");
        assert!(text.contains("test.entity"), "{format}: {text}");
    }

    // A template named after a builtin format must still receive the template context.
    let templates = tempfile::tempdir().unwrap();
    let json = templates.path().join("json");
    std::fs::create_dir(&json).unwrap();
    std::fs::write(
        json.join("weaver.yaml"),
        "templates:\n  - template: diff.j2\n    filter: .\n    application_mode: single\n",
    )
    .unwrap();
    std::fs::write(json.join("diff.j2"), "{{ ctx.head.semconv_version }}|{{ ctx.baseline.semconv_version }}|{{ ctx.changes.registry_attributes | length }}|{{ ctx.changes.resources | length }}").unwrap();
    let output = v2_diff(head.path(), "json")
        .arg("--templates")
        .arg(templates.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(output).unwrap().trim(), "1.1.0|1.0.0|2|1");
}

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

#[test]
fn test_published_v2_registry_diff() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let result = cmd
        .arg("registry")
        .arg("diff")
        .arg("--v2")
        .arg("-r")
        .arg("tests/published_v2_registry/")
        .arg("--baseline-registry")
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
