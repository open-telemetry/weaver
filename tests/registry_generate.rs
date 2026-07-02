// SPDX-License-Identifier: Apache-2.0

//! Test the registry generate command.

use assert_cmd::Command;
use std::fs;

/// This test checks the CLI interface for the registry generate command.
/// This test doesn't count for the coverage report as it runs a separate process.
#[test]
fn test_cli_interface() {
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("registry")
        .arg("generate")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    // The target and the output directory are not provided, so the command should fail.
    assert!(!output.status.success());

    // Test a local semantic convention registry.
    // There are policy violations in this registry.
    // This test should fail with a non-zero exit code and display the policy violations.
    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .arg("--quiet")
        .arg("registry")
        .arg("generate")
        .arg("-r")
        .arg("crates/weaver_codegen_test/semconv_registry/")
        .arg("-t")
        .arg("crates/weaver_codegen_test/templates/")
        .arg("--skip-policies=false")
        .arg("--diagnostic-format")
        .arg("json")
        .arg("--diagnostic-stdout")
        .arg("true")
        .arg("rust")
        .arg("output/")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(!output.status.success());

    // We should be able to parse the JSON output from stdout.
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let json_value: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("Invalid JSON");
    // We expect 31 policy violations.
    assert_eq!(json_value.len(), 31);
}

/// End-to-end check that the project-level `[template]` section of a discovered
/// `.weaver.toml` is merged into the template package's own `weaver.yaml`
/// acronyms, with the project winning on case-insensitive conflicts.
///
/// The command runs with its working directory set to the temp project so the
/// `.weaver.toml` is found by the upward-walk discovery (not the `--config`
/// flag, which collides with `generate`'s own `-c`).
#[test]
fn test_generate_merges_template_acronyms_from_weaver_toml() {
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let registry = format!("{repo_root}/crates/weaver_codegen_test/semconv_registry/");

    let project = tempfile::tempdir().expect("Failed to create temp dir");
    let proj = project.path();

    // Project config: add gRPC/SDK and re-case the package's `iOS` as `IOS`.
    fs::write(
        proj.join(".weaver.toml"),
        "[template]\nacronyms = [\"gRPC\", \"SDK\", \"IOS\"]\n",
    )
    .expect("Failed to write .weaver.toml");

    // Minimal template package with its own acronyms and a template that
    // exercises the `acronym` filter on a fixed string.
    let tdir = proj.join("templates").join("registry").join("tgt");
    fs::create_dir_all(&tdir).expect("Failed to create template dir");
    fs::write(
        tdir.join("weaver.yaml"),
        "acronyms: [\"iOS\", \"API\", \"URL\"]\n\
         templates:\n\
         \x20 - template: \"out.md\"\n\
         \x20   filter: \".\"\n\
         \x20   application_mode: single\n",
    )
    .expect("Failed to write weaver.yaml");
    fs::write(
        tdir.join("out.md"),
        "{{ \"api url ios grpc sdk http\" | acronym }}\n",
    )
    .expect("Failed to write template");

    let mut cmd = Command::cargo_bin("weaver").unwrap();
    let output = cmd
        .current_dir(proj)
        .arg("--quiet")
        .arg("registry")
        .arg("generate")
        .arg("-r")
        .arg(&registry)
        .arg("-t")
        .arg("templates")
        .arg("--skip-policies")
        .arg("tgt")
        .arg("out")
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute process");

    assert!(
        output.status.success(),
        "generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated =
        fs::read_to_string(proj.join("out").join("out.md")).expect("Failed to read generated file");
    // Package acronyms (API, URL) survive, the project's (gRPC, SDK) are added,
    // and the project's `IOS` wins the case-insensitive collision with `iOS`.
    assert_eq!(generated.trim(), "API URL IOS gRPC SDK http");
}
