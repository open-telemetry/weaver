// SPDX-License-Identifier: Apache-2.0

//! Where the dependency chain sits in the lookup order: the registry under
//! check is searched in full, steps 3 and 4, before any dependency, steps 5 and
//! 6. Exact-beats-template and longest-template-wins apply only within a step
//! pair, so neither lets a dependency win. See `docs/attribute_lookup.md`.
//!
//! These run the CLI over a text file of attribute names, so no OTLP receiver
//! is needed.

use std::fs;
use std::process::Command as StdCommand;

use serde_json::Value;

/// Registry holding only templates. Its dependency holds an exact key and a
/// longer template, and both lose to the local templates.
const LOCAL_REGISTRY: &str = "data/model/lookup_precedence/local";

/// Runs a live check over `attributes`, and returns the samples.
fn samples(dir: &std::path::Path, attributes: &str) -> Vec<Value> {
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
            LOCAL_REGISTRY,
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

    let path = fs::read_dir(&output)
        .expect("no report directory")
        .next()
        .expect("no report file")
        .expect("failed to read the report entry")
        .path();
    let report = fs::read_to_string(path).expect("failed to read the report");
    let report: Value = serde_json::from_str(&report).expect("report is not valid JSON");
    report["samples"]
        .as_array()
        .expect("no samples in the report")
        .clone()
}

/// The findings on the one sample in `report`, as (id, template name) pairs.
fn findings(sample: &Value) -> Vec<(String, Option<String>)> {
    sample["attribute"]["live_check_result"]["all_advice"]
        .as_array()
        .map(|advice| {
            advice
                .iter()
                .map(|finding| {
                    (
                        finding["id"].as_str().unwrap_or_default().to_owned(),
                        finding["context"]["template_name"]
                            .as_str()
                            .map(str::to_owned),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A template in the registry under check beats an exact key that only a
/// dependency defines. Exact beats template only within a step pair, and step 4
/// runs before step 5.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_local_template_outranks_a_dependency_exact_key() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let samples = samples(dir.path(), "probe.a.b=x\n");

    assert_eq!(
        findings(&samples[0]),
        vec![("template_attribute".to_owned(), Some("probe.a".to_owned()))],
        "the local `probe.a` template is found at step 4, before the \
         dependency's exact `probe.a.b` at step 5: {:#}",
        samples[0]
    );
}

/// A shorter template in the registry under check beats a longer one in a
/// dependency. Length only decides between templates found in the same step.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_local_template_outranks_a_longer_dependency_template() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let samples = samples(dir.path(), "probe.long.prefix.x=y\n");

    assert_eq!(
        findings(&samples[0]),
        vec![(
            "template_attribute".to_owned(),
            Some("probe.long".to_owned())
        )],
        "the local `probe.long` is shorter than the dependency's \
         `probe.long.prefix`, and still wins: {:#}",
        samples[0]
    );
}

/// The policies see the same copy the lookup uses. This covers the advice
/// preprocessor, which merges the two attribute maps and must resolve a
/// collision the way the lookup does.
///
/// The assertion is indirect: `illegal_namespace` is suppressed for a
/// deprecated attribute, and only the dependency's copy of `probe.dup` is
/// deprecated, so the finding fires only if the live copy won.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_local_definition_wins_over_a_deprecated_dependency_one() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let samples = samples(dir.path(), "probe.dup.child=x\n");

    let ids: Vec<String> = findings(&samples[0])
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(
        ids.contains(&"illegal_namespace".to_owned()),
        "`probe.dup` is live in the registry under check, so extending it is a \
         collision. Seeing the dependency's deprecated copy instead would \
         suppress this finding: {:#}",
        samples[0]
    );
}
