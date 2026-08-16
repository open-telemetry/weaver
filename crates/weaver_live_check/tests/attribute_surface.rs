// SPDX-License-Identifier: Apache-2.0

//! The registry surface: which attributes a v2 registry is responsible for, and
//! which of the three `seen_*` records a sample lands in. The rules are in
//! `docs/attribute_lookup.md`.
//!
//! These run the CLI over a text file of attribute names, so every sample
//! arrives with no matched signal and no OTLP receiver is needed.

use std::fs;
use std::process::Command as StdCommand;

use serde_json::Value;

/// Registry that defines nothing and imports everything from `base`. Its two
/// attribute groups, `acme.scope` and `acme.deploy`, are the only route by
/// which `acme.scope.env` and `acme.deploy.tier` reach it.
const IMPORTING_REGISTRY: &str = "data/model/imported_attributes/main";

/// Runs a live check over `attributes`, and returns the report statistics.
fn statistics(dir: &std::path::Path, attributes: &str) -> Value {
    report(dir, attributes)["statistics"].clone()
}

/// Runs a live check over `attributes`, and returns the whole report.
///
/// `fail_on` is off because an undefined attribute is a violation, which would
/// otherwise exit non-zero and fail this helper.
fn report(dir: &std::path::Path, attributes: &str) -> Value {
    let input = dir.join("attributes.txt");
    fs::write(&input, attributes).expect("failed to write the input file");
    let config = dir.join("weaver.toml");
    fs::write(&config, "[live-check]\nfail_on = \"none\"\n").expect("failed to write the config");
    let output = dir.join("report");

    #[allow(deprecated)] // cargo_bin() is the only cross-crate way to find the binary
    let weaver_bin = assert_cmd::cargo::cargo_bin("weaver");
    let status = StdCommand::new(weaver_bin)
        .args([
            "registry",
            "live-check",
            "-r",
            IMPORTING_REGISTRY,
            "--v2",
            "--config",
            config.to_str().expect("config path is not utf-8"),
            "--input-source",
            input.to_str().expect("input path is not utf-8"),
            "--input-format",
            "text",
            "--format",
            "json",
            "-o",
            output.to_str().expect("output path is not utf-8"),
        ])
        .status()
        .expect("failed to run weaver live-check");
    assert!(status.success(), "live-check exited with {status}");

    let report = fs::read_dir(&output)
        .expect("no report directory")
        .next()
        .expect("no report file")
        .expect("failed to read the report entry")
        .path();
    let report = fs::read_to_string(report).expect("failed to read the report");
    serde_json::from_str(&report).expect("report is not valid JSON")
}

/// The keys of a statistics map, sorted.
fn keys(statistics: &Value, field: &str) -> Vec<String> {
    let mut keys: Vec<String> = statistics[field]
        .as_object()
        .unwrap_or_else(|| panic!("no {field} in:\n{statistics:#}"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// Every public attribute group is part of the registry. Both of these
/// attributes reach the importing registry only through a group.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn public_attribute_groups_are_part_of_the_registry() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let statistics = statistics(dir.path(), "acme.scope.env=test\nacme.deploy.tier=canary\n");

    assert_eq!(
        statistics["seen_registry_attributes"]["acme.scope.env"], 1,
        "the `acme.scope` group is part of the registry: {statistics:#}"
    );
    assert_eq!(
        statistics["seen_registry_attributes"]["acme.deploy.tier"], 1,
        "the `acme.deploy` group is part of the registry: {statistics:#}"
    );
    assert!(
        keys(&statistics, "seen_dependency_attributes").is_empty(),
        "both groups are part of the registry, so nothing falls to the dependency: {statistics:#}"
    );
}

/// Surface membership decides before the definition's origin, so an imported
/// signal's attribute is a registry attribute even though only the dependency
/// holds the definition. Otherwise an importing registry could never score.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn an_imported_signal_attribute_counts_as_a_registry_attribute() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let statistics = statistics(dir.path(), "acme.host.id=abc\n");

    assert_eq!(
        statistics["seen_registry_attributes"]["acme.host.id"], 1,
        "the imported `acme.uptime` metric declares this attribute: {statistics:#}"
    );
    assert!(
        keys(&statistics, "seen_dependency_attributes").is_empty(),
        "the registry surface decides before the source of the definition: {statistics:#}"
    );
}

/// An attribute that the registry imports nothing for is outside the surface,
/// but the dependency search still finds a definition for it.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn an_attribute_only_the_dependency_holds_is_counted_apart() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let statistics = statistics(dir.path(), "acme.header.host=example.com\n");

    assert_eq!(
        statistics["seen_dependency_attributes"]["acme.header.host"], 1,
        "no signal and no attribute group of this registry holds `acme.header.host`, \
         so only the dependency defines it: {statistics:#}"
    );
    assert!(
        keys(&statistics, "seen_non_registry_attributes").is_empty(),
        "the dependency defines it, so it is not unknown: {statistics:#}"
    );
    assert!(
        !keys(&statistics, "seen_registry_attributes").contains(&"acme.header.host".to_owned()),
        "an attribute outside the surface must not be part of coverage: {statistics:#}"
    );
}

/// Span attributes are pooled and counted, even though a span is never a
/// matched signal. `acme.operation.step` reaches this registry through the
/// imported `acme.operation` span alone, so nothing else can put it in the
/// surface.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_span_only_attribute_is_part_of_the_registry_surface() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let statistics = statistics(dir.path(), "acme.operation.step=validate\n");

    assert_eq!(
        statistics["seen_registry_attributes"]["acme.operation.step"], 1,
        "the imported `acme.operation` span declares this attribute: {statistics:#}"
    );
    assert!(
        keys(&statistics, "seen_dependency_attributes").is_empty(),
        "a span attribute is part of the surface, so it does not fall to the \
         dependency: {statistics:#}"
    );
}

/// A pooled span attribute resolves against the original definition, never the
/// span's refinement of it. `acme.operation.step` is stable in the registry, so
/// a bare sample drawing no stability finding shows which one was used.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_span_attribute_resolves_against_the_original_definition() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let report = report(dir.path(), "acme.operation.step=validate\n");

    let findings: Vec<String> = report["samples"][0]["attribute"]["live_check_result"]
        ["all_advice"]
        .as_array()
        .map(|advice| {
            advice
                .iter()
                .map(|finding| finding["id"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default();

    assert!(
        findings.is_empty(),
        "the original definition is stable and not deprecated, so there is \
         nothing to report: {report:#}"
    );
}

/// An attribute that no registry in the closure defines is unknown.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn an_undefined_attribute_is_unknown() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let statistics = statistics(dir.path(), "acme.not.defined.anywhere=1\n");

    assert_eq!(
        keys(&statistics, "seen_non_registry_attributes"),
        vec!["acme.not.defined.anywhere".to_owned()],
        "no registry defines this key: {statistics:#}"
    );
    assert!(
        keys(&statistics, "seen_dependency_attributes").is_empty(),
        "an unknown attribute is not a dependency attribute: {statistics:#}"
    );
}
