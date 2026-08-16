// SPDX-License-Identifier: Apache-2.0

//! Steps 1 and 2 of the lookup, for a v1 registry. A v1 metric or event is a
//! group holding its own attributes, so a matched signal's refinement applies
//! to samples on it, as in v2. See `docs/attribute_lookup.md`.
//!
//! Without these steps a v1 lookup reads only the flat map of every group,
//! where the last group to define a key wins. The registry group of `base`
//! comes after the metric group, so its unrefined copy would hide the
//! refinement that `acme.uptime` makes.
//!
//! These run the CLI over a JSON file of samples, so no OTLP receiver is
//! needed.

use std::fs;
use std::process::Command as StdCommand;

use serde_json::Value;

/// Registry that defines every attribute and every signal.
const DEFINING_REGISTRY: &str = "data/model/imported_attributes/base";

/// A metric that the registry defines. It refines `acme.legacy.id` to
/// `development` for itself alone.
const MATCHED_METRIC: &str = "acme.uptime";

/// A metric that the registry does not define, used as the control.
const UNMATCHED_METRIC: &str = "acme.unmatched";

/// Runs a v1 live check over `samples`, and returns the report.
fn report(dir: &std::path::Path, samples: &str) -> Value {
    let input = dir.join("samples.json");
    fs::write(&input, samples).expect("failed to write the input file");
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
            DEFINING_REGISTRY,
            "--v2=false",
            "--config",
            config.to_str().expect("config path is not utf-8"),
            "--input-source",
            input.to_str().expect("input path is not utf-8"),
            "--input-format",
            "json",
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
    serde_json::from_str(&report).expect("report is not valid JSON")
}

/// The finding ids on `attribute`, when it is carried by `carrier`.
///
/// `carrier` is the name of the metric that holds the sample, or `"bare"` for a
/// sample with no carrier.
fn finding_ids(report: &Value, carrier: &str, attribute: &str) -> Vec<String> {
    fn walk(value: &Value, context: &str, out: &mut Vec<(String, String, String)>) {
        match value {
            Value::Object(map) => {
                let mut context = context;
                if let Some(name) = map.get("metric").and_then(|m| m["name"].as_str()) {
                    context = name;
                }
                if let (Some(name), Some(result)) = (map.get("name"), map.get("live_check_result"))
                {
                    // A sample attribute has both a name and a value
                    if let (Some(name), true) = (name.as_str(), map.contains_key("value")) {
                        for finding in result["all_advice"].as_array().into_iter().flatten() {
                            out.push((
                                context.to_owned(),
                                name.to_owned(),
                                finding["id"].as_str().unwrap_or_default().to_owned(),
                            ));
                        }
                    }
                }
                for nested in map.values() {
                    walk(nested, context, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, context, out);
                }
            }
            _ => {}
        }
    }

    let mut all = Vec::new();
    walk(&report["samples"], "bare", &mut all);
    let mut ids: Vec<String> = all
        .into_iter()
        .filter(|(found_carrier, found_attribute, _)| {
            found_carrier == carrier && found_attribute == attribute
        })
        .map(|(_, _, id)| id)
        .collect();
    ids.sort();
    ids
}

fn metric(name: &str, attribute: &str) -> String {
    format!(
        r#"{{ "metric": {{ "name": "{name}", "instrument": "gauge", "unit": "s",
             "data_points": [ {{ "attributes": [
                 {{ "name": "{attribute}", "value": "L-1" }} ], "value": 1 }} ] }} }}"#
    )
}

/// The refinement that `acme.uptime` makes for itself reaches the sample on
/// that metric, and does not reach a sample on any other carrier.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_v1_matched_signal_refinement_is_honoured() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let samples = format!(
        "[{}, {}, {{ \"attribute\": {{ \"name\": \"acme.legacy.id\", \"value\": \"L-1\" }} }}]",
        metric(MATCHED_METRIC, "acme.legacy.id"),
        metric(UNMATCHED_METRIC, "acme.legacy.id"),
    );
    let report = report(dir.path(), &samples);

    assert!(
        finding_ids(&report, MATCHED_METRIC, "acme.legacy.id").contains(&"not_stable".to_owned()),
        "`{MATCHED_METRIC}` refines `acme.legacy.id` to development for itself:\n{report:#}"
    );
    assert!(
        !finding_ids(&report, UNMATCHED_METRIC, "acme.legacy.id")
            .contains(&"not_stable".to_owned()),
        "the refinement must not reach an unmatched metric:\n{report:#}"
    );
    assert!(
        !finding_ids(&report, "bare", "acme.legacy.id").contains(&"not_stable".to_owned()),
        "the refinement must not reach a sample with no carrier:\n{report:#}"
    );
}

/// A template that the matched signal declares outranks an exact attribute of
/// the registry, in a v1 registry as in a v2 one.
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn a_v1_matched_signal_template_outranks_an_exact_registry_attribute() {
    let dir = tempfile::tempdir().expect("failed to create the temp dir");
    let samples = format!("[{}]", metric(MATCHED_METRIC, "acme.header.host"));
    let report = report(dir.path(), &samples);

    let ids = finding_ids(&report, MATCHED_METRIC, "acme.header.host");
    assert!(
        ids.contains(&"template_attribute".to_owned()),
        "`{MATCHED_METRIC}` declares the `acme.header` template, which outranks the exact \
         `acme.header.host` of the registry:\n{report:#}"
    );
    assert!(
        ids.contains(&"not_stable".to_owned()),
        "the template on `{MATCHED_METRIC}` is refined to development:\n{report:#}"
    );
}
