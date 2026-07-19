// SPDX-License-Identifier: Apache-2.0

//! Test the registry generate command.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

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
    let registry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("weaver_codegen_test")
        .join("semconv_registry");

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

#[test]
fn test_generate_merges_template_text_maps_from_weaver_toml() {
    let registry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("weaver_codegen_test")
        .join("semconv_registry");

    let project = tempfile::tempdir().expect("Failed to create temp dir");
    let proj = project.path();

    // Project config overrides only the `namespace_mapping` map.
    fs::write(
        proj.join(".weaver.toml"),
        "[template.text_maps.namespace_mapping]\nCICD = \"CI/CD\"\n",
    )
    .expect("Failed to write .weaver.toml");

    // Package declares two named maps and a template exercising `map_text`.
    let tdir = proj.join("templates").join("registry").join("tgt");
    fs::create_dir_all(&tdir).expect("Failed to create template dir");
    fs::write(
        tdir.join("weaver.yaml"),
        r#"text_maps:
  type_mapping:
    int: int64
  namespace_mapping:
    CICD: WRONG
    EXTRA: kept
templates:
  - template: "out.md"
    filter: "."
    application_mode: single
"#,
    )
    .expect("Failed to write weaver.yaml");
    fs::write(
        tdir.join("out.md"),
        "{{ \"CICD\" | map_text(\"namespace_mapping\") }}\n\
         {{ \"EXTRA\" | map_text(\"namespace_mapping\", \"dropped\") }}\n\
         {{ \"int\" | map_text(\"type_mapping\") }}\n",
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
    let lines: Vec<&str> = generated.lines().collect();
    // `namespace_mapping` is replaced wholesale by the project map: `CICD`
    // resolves to the project's value, and the package-only `EXTRA` entry is
    // gone (falls back to the default). The package-only `type_mapping` survives.
    assert_eq!(lines[0], "CI/CD");
    assert_eq!(lines[1], "dropped");
    assert_eq!(lines[2], "int64");
}

/// End-to-end check that a template `when` clause (a JQ expression over the
/// template params under `$params`) gates whether the template is applied. The
/// unconditional template is always generated; the conditional one appears only
/// when the param flips the `when` expression to true.
#[test]
fn test_generate_template_when_gates_output() {
    let registry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("weaver_codegen_test")
        .join("semconv_registry");

    // Two templates: `always.md` (no `when`) and `readme.md` gated on the
    // `gen_readme` param, defaulting to false at the top level.
    let templates = tempfile::tempdir().expect("Failed to create temp dir");
    let tdir = templates.path().join("registry").join("tgt");
    fs::create_dir_all(&tdir).expect("Failed to create template dir");
    fs::write(
        tdir.join("weaver.yaml"),
        r#"params:
  gen_readme: false
templates:
  - template: "always.md"
    filter: "."
    application_mode: single
  - template: "readme.md"
    filter: "."
    application_mode: single
    when: "$params.gen_readme == true"
"#,
    )
    .expect("Failed to write weaver.yaml");
    fs::write(tdir.join("always.md"), "always\n").expect("Failed to write template");
    fs::write(tdir.join("readme.md"), "readme\n").expect("Failed to write template");

    let templates_path = templates.path().to_str().unwrap();

    let registry = registry.to_str().unwrap();

    let run = |out_dir: &Path, extra: &[&str]| {
        let out = out_dir.to_str().unwrap();
        let mut args: Vec<&str> = vec![
            "--quiet",
            "registry",
            "generate",
            "-r",
            registry,
            "-t",
            templates_path,
            "--skip-policies",
        ];
        args.extend_from_slice(extra);
        args.push("tgt");
        args.push(out);
        let mut cmd = Command::cargo_bin("weaver").unwrap();
        let output = cmd
            .args(&args)
            .timeout(std::time::Duration::from_secs(60))
            .output()
            .expect("failed to execute process");
        assert!(
            output.status.success(),
            "generate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    // Default: gen_readme is false, so the conditional template is skipped.
    let off = templates.path().join("out_off");
    run(&off, &[]);
    assert!(
        off.join("always.md").exists(),
        "always.md should be generated"
    );
    assert!(
        !off.join("readme.md").exists(),
        "readme.md should be skipped when the `when` clause is false"
    );

    // Flip the flag via a CLI param: the conditional template is now applied.
    let on = templates.path().join("out_on");
    run(&on, &["-D", "gen_readme=true"]);
    assert!(
        on.join("always.md").exists(),
        "always.md should be generated"
    );
    assert!(
        on.join("readme.md").exists(),
        "readme.md should be generated when the `when` clause is true"
    );
}
