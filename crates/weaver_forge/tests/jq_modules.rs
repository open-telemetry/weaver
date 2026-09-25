// SPDX-License-Identifier: Apache-2.0

//! Public JQ API compatibility and configured-module regression tests.
#![allow(clippy::unwrap_used)]

use std::{collections::BTreeMap, fs, path::PathBuf};

use serde_json::json;
use weaver_forge::{
    error::FilterErrorDetail,
    jq::{execute_jq, execute_jq_with_modules},
};

#[test]
fn legacy_error_literals_and_serialization_remain_compatible() {
    let detail = FilterErrorDetail {
        error: "failure".to_owned(),
        source: None,
    };
    assert_eq!(
        serde_json::to_value(detail).unwrap(),
        json!({"error": "failure"})
    );
}

#[test]
fn unconfigured_filters_cannot_load_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("helper.jq"), "def helper: 42;").unwrap();
    let path = dir
        .path()
        .join("helper")
        .to_string_lossy()
        .replace('\\', "/");
    let expression = format!("include \"{path}\"; helper");
    for result in [
        execute_jq(&json!(null), &expression, &BTreeMap::new()),
        execute_jq_with_modules(&json!(null), &expression, &BTreeMap::new(), dir.path(), &[]),
    ] {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("module loading not supported"));
    }
}

#[test]
fn absolute_parent_and_escaped_paths_execute_repeatedly() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    fs::create_dir_all(&nested).unwrap();
    let module = dir.path().join("space and unicode 模块.jq");
    fs::write(&module, "def transform: .value + $increment;").unwrap();
    for path in [
        module.clone(),
        PathBuf::from("../space and unicode 模块.jq"),
    ] {
        for increment in 0..4 {
            let params = BTreeMap::from([("increment".to_owned(), json!(increment))]);
            assert_eq!(
                execute_jq_with_modules(
                    &json!({"value": 40}),
                    "transform",
                    &params,
                    &nested,
                    std::slice::from_ref(&path)
                )
                .unwrap(),
                json!(40 + increment)
            );
        }
    }
}

#[test]
fn nested_namespaced_imports_and_duplicate_modules() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("helper.jq"), "def value: 42;").unwrap();
    fs::write(
        dir.path().join("main.jq"),
        "import \"helper\" as h; def value: h::value;",
    )
    .unwrap();
    let modules = [PathBuf::from("main.jq"), PathBuf::from("./main.jq")];
    assert_eq!(
        execute_jq_with_modules(
            &json!(null),
            "value",
            &BTreeMap::new(),
            dir.path(),
            &modules
        )
        .unwrap(),
        json!(42)
    );
}

#[test]
fn module_changes_and_runtime_errors_are_observed_on_next_call() {
    let dir = tempfile::tempdir().unwrap();
    let module = dir.path().join("main.jq");
    let run = || {
        execute_jq_with_modules(
            &json!(null),
            "value",
            &BTreeMap::new(),
            dir.path(),
            std::slice::from_ref(&module),
        )
    };
    fs::write(&module, "def value: 42;").unwrap();
    assert_eq!(run().unwrap(), json!(42));
    fs::write(&module, "def value: error(\"module failure\");").unwrap();
    assert!(run().unwrap_err().to_string().contains("module failure"));
    fs::remove_file(&module).unwrap();
    assert!(run().unwrap_err().to_string().contains("main.jq"));
}

#[test]
fn cycles_invalid_utf8_and_missing_nested_imports_fail_with_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let module = dir.path().join("main.jq");
    let run = || {
        execute_jq_with_modules(
            &json!(null),
            ".",
            &BTreeMap::new(),
            dir.path(),
            std::slice::from_ref(&module),
        )
    };
    fs::write(&module, "include \"main\";").unwrap();
    assert!(run().is_err());
    fs::write(&module, [0xff, 0xfe]).unwrap();
    let message = run().unwrap_err().to_string();
    assert!(message.contains("UTF-8"));
    assert!(message.contains("main.jq"));
    assert!(!message.contains("__weaver_configured_module_"));
    fs::write(&module, "include \"missing\";").unwrap();
    let message = run().unwrap_err().to_string();
    // Nested imports resolve from the canonical importing module's directory.
    let missing = module.canonicalize().unwrap().with_file_name("missing.jq");
    assert!(
        message.contains(&missing.display().to_string()),
        "expected missing module path {} in diagnostic: {message}",
        missing.display()
    );
}

#[test]
fn module_errors_serialize_file_and_local_span() {
    let dir = tempfile::tempdir().unwrap();
    let module = dir.path().join("broken.jq");
    fs::write(&module, "def broken: undefined_filter;").unwrap();
    let error = execute_jq_with_modules(
        &json!(null),
        "broken",
        &BTreeMap::new(),
        dir.path(),
        std::slice::from_ref(&module),
    )
    .unwrap_err();
    let value = serde_json::to_value(&error).unwrap();
    let detail = &value["ModuleFilterError"]["details"][0];
    assert_eq!(detail["file"], json!(module.canonicalize().unwrap()));
    assert_eq!(detail["source"]["start"], json!({"line": 1, "col": 13}));
    assert!(error.to_string().contains("broken.jq:1:13"));
}

#[test]
fn module_named_like_the_old_entrypoint_retains_its_source_location() {
    let dir = tempfile::tempdir().unwrap();
    let module = dir.path().join(".weaver-jq-entrypoint.jq");
    fs::write(&module, "def broken: undefined_filter;").unwrap();
    let error = execute_jq_with_modules(
        &json!(null),
        "broken",
        &BTreeMap::new(),
        &dir.path().canonicalize().unwrap(),
        std::slice::from_ref(&module),
    )
    .unwrap_err();
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(
        value["ModuleFilterError"]["details"][0]["file"],
        json!(module.canonicalize().unwrap())
    );
    assert_eq!(
        value["ModuleFilterError"]["details"][0]["source"]["start"]["line"],
        json!(1)
    );
}

#[cfg(unix)]
#[test]
fn configured_paths_with_quotes_and_newlines() {
    let dir = tempfile::tempdir().unwrap();
    let module = dir.path().join("quote\"and\nnewline.jq");
    fs::write(&module, "def value: 42;").unwrap();
    assert_eq!(
        execute_jq_with_modules(
            &json!(null),
            "value",
            &BTreeMap::new(),
            dir.path(),
            &[module]
        )
        .unwrap(),
        json!(42)
    );
}
