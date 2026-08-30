// SPDX-License-Identifier: Apache-2.0

//! The v1 mirror of `matchers_otlp`.
//!
//! Sends the same telemetry to a v1 registry with the same attribute and signal
//! names, and asserts v1 checks every attribute against the registry as a whole
//! and gains none of the matcher behaviour.
//!
//! To read the report on a terminal instead, start a receiver from this
//! directory:
//!
//! ```text
//! cargo run --manifest-path ../../Cargo.toml -- registry live-check \
//!   -r data/model/matchers_v1 \
//!   --advice-policies data/matchers/advice --fail-on none \
//!   --inactivity-timeout 300
//! ```
//!
//! then send it the same telemetry the v2 test emits, and stop it to print the
//! report:
//!
//! ```text
//! cargo nextest run -p weaver_live_check emit_to_a_running_live_check \
//!   --run-ignored ignored-only
//! curl -X POST http://localhost:4320/stop
//! ```

mod common;

use common::{annotation_source, finding_for, findings, run, span, with_id, REGISTRY_V1};
use serde_json::Value;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn a_v1_registry_checks_every_attribute_against_the_registry() {
    let report = run(REGISTRY_V1, &[]).await;
    let findings = findings(&report);

    the_attribute_advisors_run_on_every_sample(&findings);
    the_signal_lookups_are_by_name(&findings);
    the_entity_associations_are_checked_against_the_resource(&findings);
    one_definition_serves_every_sample(&report);
    no_v2_matcher_behaviour_appears(&report, &findings);
}

/// v1 flattens every group's attributes into one map keyed by name.
fn one_definition_serves_every_sample(report: &Value) {
    let on_the_span = annotation_source(span(report, "checkout"), "acme.checkout.stage");
    assert_eq!(on_the_span, Some("span"));

    let metric = report["samples"]
        .as_array()
        .expect("the report carries the samples")
        .iter()
        .map(|sample| &sample["metric"])
        .find(|metric| metric["name"] == "acme.cart.items")
        .expect("the metric sample");
    let on_the_point = annotation_source(&metric["data_points"][0], "acme.checkout.stage");
    assert_eq!(
        on_the_point, on_the_span,
        "v1 resolves one definition per attribute name"
    );
}

/// v1 compares each attribute with the registry, whatever sample carries it.
fn the_attribute_advisors_run_on_every_sample(findings: &[&Value]) {
    for (id, key) in [
        ("not_stable", "acme.customer.tier"),
        ("deprecated", "acme.checkout.legacy.id"),
        ("template_attribute", "acme.request.header.accept"),
        ("type_mismatch", "acme.cart.item.count"),
        ("undefined_enum_variant", "acme.checkout.stage"),
        ("missing_attribute", "acme.stray.field"),
        ("missing_attribute", "acme.tenant.id"),
        ("missing_attribute", "telemetry.sdk.language"),
    ] {
        // Panics when the finding is not there, which is the assertion.
        let _ = finding_for(findings, id, key);
    }

    // Declared by the registry, so checked and clean.
    let missing: Vec<&str> = with_id(findings, "missing_attribute")
        .iter()
        .filter_map(|finding| finding["context"]["attribute_key"].as_str())
        .collect();
    for key in ["acme.checkout.id", "acme.session.id", "service.name"] {
        assert!(!missing.contains(&key), "got: {missing:?}");
    }
}

/// A metric and a log reach their signal by name, as on v2.
fn the_signal_lookups_are_by_name(findings: &[&Value]) {
    let mut missing_metrics: Vec<&str> = with_id(findings, "missing_metric")
        .iter()
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    missing_metrics.sort_unstable();
    assert_eq!(
        missing_metrics,
        ["acme.legacy.checkout.attempts", "acme.unknown.total"]
    );

    let mut missing_events: Vec<&str> = with_id(findings, "missing_event")
        .iter()
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    missing_events.sort_unstable();
    assert_eq!(
        missing_events,
        ["acme.checkout.dropped", "acme.checkout.failed"]
    );

    // The metric resolves by name, so its unit and instrument agree.
    assert!(with_id(findings, "unit_mismatch").is_empty());
    assert!(with_id(findings, "unexpected_instrument").is_empty());
}

fn the_entity_associations_are_checked_against_the_resource(findings: &[&Value]) {
    for (id, key) in [
        (
            "entity_required_attribute_not_present",
            "acme.deployment.id",
        ),
        (
            "entity_recommended_attribute_not_present",
            "acme.deployment.name",
        ),
        (
            "entity_opt_in_attribute_not_present",
            "acme.deployment.owner",
        ),
        (
            "entity_conditionally_required_attribute_not_present",
            "acme.deployment.region",
        ),
    ] {
        let finding = finding_for(findings, id, key);
        assert_eq!(finding["context"]["entity_type"], "acme.deployment");
        assert_eq!(finding["signal_name"], "acme.cart.items");
    }

    let unsatisfied = with_id(findings, "entity_association_not_satisfied");
    assert_eq!(unsatisfied.len(), 1, "got: {unsatisfied:?}");
    assert_eq!(unsatisfied[0]["signal_name"], "acme.checkout.completed");
}

fn no_v2_matcher_behaviour_appears(report: &Value, findings: &[&Value]) {
    assert!(
        with_id(findings, "unexpected_attribute").is_empty(),
        "v2 only"
    );
    assert!(with_id(findings, "kind_mismatch").is_empty(), "v2 only");

    let matchers = report["statistics"]["matchers"]
        .as_array()
        .expect("the statistics carry the matchers");
    assert!(matchers.is_empty(), "got: {matchers:?}");

    // A v1 registry takes no matchers, so no sample carries a match.
    let mut samples = 0;
    for sample in report["samples"]
        .as_array()
        .expect("the report carries the samples")
    {
        for kind in ["span", "log", "metric", "resource", "instrumentation_scope"] {
            let result = &sample[kind]["live_check_result"];
            if result.is_object() {
                samples += 1;
                assert!(result["match_info"].is_null(), "got: {result}");
            }
        }
    }
    assert!(samples > 0, "the report carries samples to check");
}
