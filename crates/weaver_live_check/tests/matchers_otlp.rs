// SPDX-License-Identifier: Apache-2.0

//! End-to-end matcher coverage.
//!
//! Emits telemetry with the OpenTelemetry SDK into a `weaver registry
//! live-check` child process reading OTLP, and asserts the matcher features
//! from the report the child returns on `POST /stop`.
//!
//! The fixture registry is `data/model/matchers`, which depends on
//! `data/model/matchers_dep`, and the matchers are `data/matchers/livecheck.toml`.
//!
//! To read the report on a terminal instead, start a receiver from this
//! directory, because the fixture's dependency path is relative to the working
//! directory:
//!
//! ```text
//! cargo run --manifest-path ../../Cargo.toml -- registry live-check \
//!   -r data/model/matchers --v2 \
//!   --config data/matchers/livecheck.toml \
//!   --advice-policies data/matchers/advice --fail-on none \
//!   --inactivity-timeout 300
//! ```
//!
//! The timeout matters: the receiver stops after 10 seconds of quiet by
//! default, which is not long enough to start the emitter by hand.
//!
//! Add `--search-all-attributes` to resolve `acme.tenant.id`, and
//! `acme.tenant.tag.region` through the template the dependency declares.
//! Without it both report `missing_attribute`, because the catalog holds this
//! registry's attributes alone.
//!
//! then send it this telemetry, and stop it to print the report:
//!
//! ```text
//! cargo nextest run -p weaver_live_check emit_to_a_running_live_check \
//!   --run-ignored ignored-only
//! curl -X POST http://localhost:4320/stop
//! ```

mod common;

use common::{
    annotation_source, collect_findings, emit_telemetry, finding_for, findings, run, scopes, span,
    with_id, CHECKOUT_SCOPE, CONFIG, REGISTRY,
};
use serde_json::Value;
use std::collections::BTreeMap;
/// What a matcher did, from `statistics.matchers`.
fn matcher<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["statistics"]["matchers"]
        .as_array()
        .expect("the statistics list the matchers")
        .iter()
        .find(|matcher| matcher["id"] == id)
        .unwrap_or_else(|| panic!("no matcher `{id}` in {}", report["statistics"]["matchers"]))
}

/// The span sample with this name.

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn matchers_check_telemetry_from_the_sdk() {
    let report = run(REGISTRY, &["--v2", "--config", CONFIG]).await;
    let findings = findings(&report);

    every_matcher_counts_what_it_matched(&report);
    a_dead_matcher_and_a_failing_one_are_reported(&report);
    the_findings_name_the_telemetry_that_caused_them(&findings);
    the_definition_used_comes_from_the_signal_then_the_first_group(&report);
    span_events_and_links_match_on_their_own(&report);
    a_scope_takes_attribute_groups(&report);
    every_sample_records_its_match(&report);
    the_advisors_run_on_the_matched_definitions(&findings);
    a_matcher_signal_drives_the_metric_checks(&findings);
    a_matcher_signal_answers_the_natural_match(&findings);
    a_matched_attribute_group_checks_its_requirement_levels(&findings);
    a_data_point_checks_its_attributes_against_the_metric_match(&findings);
    the_entity_associations_are_checked_against_the_resource(&findings);
    the_coverage_credits_the_signal_the_match_resolved(&report);
    the_statistics_count_every_finding(&report, &findings);
}

/// A matcher's `signal` renames what a metric or log is counted under, so the
/// registry signal takes the coverage and the wire name is not a stranger.
fn the_coverage_credits_the_signal_the_match_resolved(report: &Value) {
    let statistics = &report["statistics"];
    for (seen, unseen, signal, wire_name) in [
        (
            "seen_registry_metrics",
            "seen_non_registry_metrics",
            "acme.checkout.attempts",
            "acme.legacy.checkout.attempts",
        ),
        (
            "seen_registry_events",
            "seen_non_registry_events",
            "acme.checkout.abandoned",
            "acme.checkout.dropped",
        ),
    ] {
        assert_eq!(
            statistics[seen][signal], 1,
            "`{signal}` is the signal the matcher named: {}",
            statistics[seen]
        );
        assert!(
            statistics[unseen][wire_name].is_null(),
            "`{wire_name}` resolved a registry signal: {}",
            statistics[unseen]
        );
    }
}

/// The per-attribute advisors read the definition the match resolved, and the
/// requirement levels come from the signal the matcher chose.
fn the_advisors_run_on_the_matched_definitions(findings: &[&Value]) {
    for (id, key) in [
        ("not_stable", "acme.customer.tier"),
        ("deprecated", "acme.checkout.legacy.id"),
        ("template_attribute", "acme.request.header.accept"),
        ("type_mismatch", "acme.cart.item.count"),
        ("undefined_enum_variant", "acme.checkout.stage"),
        ("required_attribute_not_present", "acme.checkout.id"),
        ("recommended_attribute_not_present", "acme.checkout.stage"),
        ("opt_in_attribute_not_present", "acme.checkout.coupon"),
        (
            "conditionally_required_attribute_not_present",
            "acme.checkout.error.code",
        ),
    ] {
        // Panics when the finding is not there, which is the assertion.
        let _ = finding_for(findings, id, key);
    }
}

/// The resource sets none of the entity's attributes, and satisfies neither
/// branch of the event's `one_of`.
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

    // A span checks its associations too, against the same resource.
    let on_a_span = finding_for(
        findings,
        "entity_required_attribute_not_present",
        "acme.cluster.id",
    );
    assert_eq!(on_a_span["context"]["entity_type"], "acme.cluster");
    assert_eq!(on_a_span["signal_name"], "checkout");

    let unsatisfied = with_id(findings, "entity_association_not_satisfied");
    assert_eq!(unsatisfied.len(), 1, "got: {unsatisfied:?}");
    assert_eq!(
        unsatisfied[0]["context"]["entity_type"],
        serde_json::json!(["acme.deployment", "acme.cluster"])
    );
    assert_eq!(unsatisfied[0]["signal_name"], "acme.checkout.completed");
}

fn a_data_point_checks_its_attributes_against_the_metric_match(findings: &[&Value]) {
    let stray = finding_for(findings, "unexpected_attribute", "acme.point.stray");
    assert_eq!(stray["signal_name"], "acme.checkout.duration");
}

fn a_matched_attribute_group_checks_its_requirement_levels(findings: &[&Value]) {
    let missing: Vec<&str> = with_id(findings, "recommended_attribute_not_present")
        .iter()
        .filter(|finding| finding["context"]["attribute_key"] == "acme.customer.tier")
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    assert!(
        missing.contains(&"checkout-legacy"),
        "`acme.span.on-error` puts `acme.customer` on it: {missing:?}"
    );
}

/// A missing metric or event is raised only when no matcher named a signal.
fn a_matcher_signal_answers_the_natural_match(findings: &[&Value]) {
    let missing: Vec<&str> = with_id(findings, "missing_metric")
        .iter()
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    assert_eq!(missing, ["acme.unknown.total"]);

    let missing: Vec<&str> = with_id(findings, "missing_event")
        .iter()
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    assert_eq!(missing, ["acme.checkout.failed"]);
}

/// A metric is checked against the signal its matcher names, not the one its
/// own name resolves to.
fn a_matcher_signal_drives_the_metric_checks(findings: &[&Value]) {
    for id in ["unit_mismatch", "unexpected_instrument"] {
        let found = with_id(findings, id);
        assert_eq!(found.len(), 1, "got: {found:?}");
        assert_eq!(found[0]["signal_name"], "acme.checkout.attempts");
    }
    // Only the natural signal declares the coupon, so this is unexpected
    // against the one the matcher named instead.
    let coupon = finding_for(findings, "unexpected_attribute", "acme.checkout.coupon");
    assert_eq!(coupon["signal_name"], "acme.checkout.attempts");
}

/// A finding a match raises is added after the advisors have run, so it is the
/// one a sample type can leave out of the statistics.
fn the_statistics_count_every_finding(report: &Value, findings: &[&Value]) {
    let mut counted = BTreeMap::new();
    for finding in findings {
        let id = finding["id"].as_str().expect("a finding id");
        *counted.entry(id).or_insert(0_u64) += 1;
    }
    let reported: BTreeMap<&str, u64> = report["statistics"]["advice_type_counts"]
        .as_object()
        .expect("the statistics count the advice types")
        .iter()
        .map(|(id, count)| (id.as_str(), count.as_u64().expect("a count")))
        .collect();
    assert_eq!(reported, counted);
    assert_eq!(
        report["statistics"]["total_advisories"],
        Value::from(findings.len())
    );
}

fn every_matcher_counts_what_it_matched(report: &Value) {
    for (id, matched) in [
        ("acme.checkout.by-name", 1),
        ("acme.checkout.legacy", 1),
        ("acme.span.on-error", 1),
        ("acme.cart.by-attribute", 1),
        ("acme.cart.conflict", 1),
        ("acme.span.by-scope", 1),
        ("acme.span.by-resource", 1),
        ("acme.log.by-event-name", 1),
        ("acme.log.by-severity", 1),
        ("acme.log.by-body", 1),
        ("acme.metric.by-name", 1),
        ("acme.metric.by-unit", 1),
        ("acme.metric.mismatched", 1),
        ("acme.span-event.step", 1),
        ("acme.span-link.session", 1),
        ("acme.resource.tenant", 3),
        ("acme.scope.acme", 4),
        ("acme.scope.checkout", 3),
        ("acme.metric.renamed", 1),
        ("acme.log.renamed", 1),
    ] {
        assert_eq!(matcher(report, id)["matched"], matched, "matcher `{id}`");
    }
}

fn a_dead_matcher_and_a_failing_one_are_reported(report: &Value) {
    assert_eq!(matcher(report, "acme.never")["matched"], 0);
    assert_eq!(matcher(report, "acme.never")["errors"], 0);

    let failing = matcher(report, "acme.errors");
    assert_eq!(failing["matched"], 0);
    assert_eq!(
        failing["errors"], 4,
        "the `when` reads an absent key on all four spans"
    );
    assert!(
        failing["first_error"]
            .as_str()
            .expect("a first error")
            .contains("No such key"),
        "got: {}",
        failing["first_error"]
    );
}

fn the_findings_name_the_telemetry_that_caused_them(findings: &[&Value]) {
    let unexpected: Vec<&str> = with_id(findings, "unexpected_attribute")
        .iter()
        .filter_map(|finding| finding["context"]["attribute_key"].as_str())
        .collect();
    assert!(
        unexpected.contains(&"acme.stray.field"),
        "got: {unexpected:?}"
    );
    assert!(
        unexpected.contains(&"acme.tenant.id"),
        "got: {unexpected:?}"
    );
    assert!(
        !unexpected.contains(&"acme.session.id"),
        "an attribute group holds it: {unexpected:?}"
    );
    assert!(
        !unexpected.contains(&"acme.request.header.accept"),
        "the signal's template `acme.request.header` covers it: {unexpected:?}"
    );
    // A resource has an expected set of its own, from the matcher's group.
    assert!(
        !unexpected.contains(&"service.name"),
        "`acme.service` holds it: {unexpected:?}"
    );
    assert!(
        unexpected.contains(&"telemetry.sdk.language"),
        "got: {unexpected:?}"
    );

    let kinds = with_id(findings, "kind_mismatch");
    assert_eq!(kinds.len(), 1, "got: {kinds:?}");
    assert_eq!(kinds[0]["signal_name"], "checkout-legacy");

    let mut missing: Vec<(&str, &str)> = with_id(findings, "required_attribute_not_present")
        .iter()
        .map(|finding| {
            (
                finding["context"]["attribute_key"]
                    .as_str()
                    .expect("an attribute key"),
                finding["signal_name"].as_str().expect("a signal name"),
            )
        })
        .collect();
    missing.sort_unstable();
    // The second is on the span event, from the event its matcher names.
    assert_eq!(
        missing,
        [
            ("acme.checkout.id", "checkout-legacy"),
            ("acme.session.id", "checkout"),
        ]
    );
}

fn the_definition_used_comes_from_the_signal_then_the_first_group(report: &Value) {
    let checkout = span(report, "checkout");
    // The span refines it, and the catalog definition says `catalog`.
    assert_eq!(
        annotation_source(checkout, "acme.checkout.stage"),
        Some("span")
    );
    // Both attribute groups declare it, and the matcher lists `acme.session`
    // first.
    assert_eq!(
        annotation_source(checkout, "acme.session.id"),
        Some("group-session")
    );

    // The metric declares no annotation, so it keeps the catalog's, not the
    // span's.
    let metric = report["samples"]
        .as_array()
        .expect("the report lists the samples")
        .iter()
        .map(|sample| &sample["metric"])
        .find(|metric| metric["name"] == "acme.cart.items")
        .expect("the metric sample");
    assert_eq!(
        annotation_source(&metric["data_points"][0], "acme.checkout.stage"),
        Some("catalog")
    );
}

/// An instrumentation scope cannot name a signal, but takes attribute groups.
fn a_scope_takes_attribute_groups(report: &Value) {
    // Only the tracer's copy of the scope has attributes.
    let scopes = scopes(report, CHECKOUT_SCOPE);
    // The catalog definition is annotated `catalog`.
    assert_eq!(
        scopes
            .iter()
            .find_map(|scope| annotation_source(scope, "acme.session.id")),
        Some("group-customer")
    );

    // The group gives the scope an expected set, so an attribute outside it is
    // unexpected.
    let mut findings = Vec::new();
    for scope in &scopes {
        collect_findings(scope, &mut findings);
    }
    let _ = finding_for(&findings, "unexpected_attribute", "acme.scope.stray");
}

/// Matched or not, a sample records what it was compared with.
fn every_sample_records_its_match(report: &Value) {
    let matched = &span(report, "checkout")["live_check_result"]["match_info"];
    assert_eq!(matched["signal"], "acme.checkout");
    assert_eq!(matched["signal_matcher"], "acme.checkout.by-name");
    assert_eq!(
        matched["attribute_groups"],
        serde_json::json!(["acme.session", "acme.customer"])
    );
    assert_eq!(
        matched["entries"],
        serde_json::json!([{
            "matcher": "acme.checkout.by-name",
            "signal": "acme.checkout",
            "attribute_groups": ["acme.session", "acme.customer"],
        }])
    );

    let unmatched = &span(report, "unknown-op")["live_check_result"]["match_info"];
    assert_eq!(unmatched["unmatched"], true);
    assert!(unmatched["signal"].is_null(), "got: {unmatched}");
    assert!(unmatched["attribute_groups"].is_null(), "got: {unmatched}");

    // The first matcher to set a signal wins; the later one is a conflict.
    let conflicted = &span(report, "cart")["live_check_result"]["match_info"];
    assert_eq!(conflicted["signal_matcher"], "acme.cart.by-attribute");
    let ignored: Vec<&str> = conflicted["entries"]
        .as_array()
        .expect("the entries")
        .iter()
        .filter(|entry| entry["ignored"] == true)
        .filter_map(|entry| entry["matcher"].as_str())
        .collect();
    assert_eq!(ignored, ["acme.cart.conflict"]);

    // The signal a matcher named, not the one the metric's own name gives.
    let renamed = &report["samples"]
        .as_array()
        .expect("the report lists the samples")
        .iter()
        .map(|sample| &sample["metric"])
        .find(|metric| metric["name"] == "acme.checkout.attempts")
        .expect("the metric sample")["live_check_result"]["match_info"];
    assert_eq!(renamed["signal"], "acme.checkout.duration");
    assert_eq!(renamed["signal_matcher"], "acme.metric.mismatched");

    // A span event resolves a signal only through a matcher, so one no matcher
    // claimed expects a signal it does not have.
    let note = &span(report, "checkout")["span_events"]
        .as_array()
        .expect("the span events")
        .iter()
        .find(|event| event["name"] == "acme.checkout.note")
        .expect("the span event")["live_check_result"]["match_info"];
    assert_eq!(note["signal_expected"], true);
    assert_eq!(note["unmatched"], true);

    // A log with no event name is not a typed signal, so having none is no gap.
    let untyped = log_match_info(report, "");
    assert_eq!(untyped["signal_expected"], false);
    assert_eq!(untyped["unmatched"], true);
    assert_eq!(
        log_match_info(report, "acme.checkout.failed")["signal_expected"],
        true
    );
}

/// The match of the log sample with this event name.
fn log_match_info<'a>(report: &'a Value, event_name: &str) -> &'a Value {
    &report["samples"]
        .as_array()
        .expect("the report lists the samples")
        .iter()
        .map(|sample| &sample["log"])
        .find(|log| log["event_name"] == event_name)
        .unwrap_or_else(|| panic!("no log sample named `{event_name}`"))["live_check_result"]
        ["match_info"]
}

fn span_events_and_links_match_on_their_own(report: &Value) {
    let checkout = span(report, "checkout");
    // The span resolves these two from its own refinement and from
    // `acme.session`, so neither source comes from the span's match.
    assert_eq!(
        annotation_source(&checkout["span_events"][0], "acme.checkout.stage"),
        Some("group-step")
    );
    assert_eq!(
        annotation_source(&checkout["span_links"][0], "acme.session.id"),
        Some("group-customer")
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn searching_all_attributes_names_the_registry_that_defines_one() {
    let report = run(
        REGISTRY,
        &["--v2", "--config", CONFIG, "--search-all-attributes"],
    )
    .await;
    let unexpected = with_id(&findings(&report), "unexpected_attribute");

    let tenant = unexpected
        .iter()
        .find(|finding| finding["context"]["attribute_key"] == "acme.tenant.id")
        .unwrap_or_else(|| panic!("no finding for acme.tenant.id in {unexpected:?}"));
    assert_eq!(
        tenant["context"]["schema_url"],
        serde_json::json!(["https://acme.example.com/shared/1.0.0"])
    );

    let stray = unexpected
        .iter()
        .find(|finding| finding["context"]["attribute_key"] == "acme.stray.field")
        .unwrap_or_else(|| panic!("no finding for acme.stray.field in {unexpected:?}"));
    assert!(
        stray["context"]["schema_url"].is_null(),
        "nothing defines it: {stray}"
    );

    // `acme.scope.acme` names no attribute groups, so the scope has no expected
    // set: falling through to the dependency is what makes this unexpected.
    let mut on_the_scope: Vec<&str> = unexpected
        .iter()
        .filter(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("is not declared by this registry"))
        })
        .map(|finding| {
            assert_eq!(
                finding["context"]["schema_url"],
                serde_json::json!(["https://acme.example.com/shared/1.0.0"])
            );
            finding["context"]["attribute_key"]
                .as_str()
                .expect("an attribute key")
        })
        .collect();
    on_the_scope.sort_unstable();
    // The second extends a template the dependency declares.
    assert_eq!(on_the_scope, ["acme.tenant.id", "acme.tenant.tag.region"]);

    // Resolving it through the template is what puts it on that list, rather
    // than reporting it as missing.
    let all = findings(&report);
    let _ = finding_for(&all, "template_attribute", "acme.tenant.tag.region");
    let missing: Vec<&str> = with_id(&all, "missing_attribute")
        .iter()
        .filter_map(|finding| finding["context"]["attribute_key"].as_str())
        .collect();
    for key in ["acme.tenant.id", "acme.tenant.tag.region"] {
        assert!(
            !missing.contains(&key),
            "the dependency declares `{key}`: {missing:?}"
        );
    }
}

/// Without `--search-all-attributes` the dependency's template is out of
/// reach, so the key it declares has nothing to resolve against.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn a_dependencys_template_needs_search_all_attributes() {
    let report = run(REGISTRY, &["--v2", "--config", CONFIG]).await;
    let all = findings(&report);
    let _ = finding_for(&all, "missing_attribute", "acme.tenant.tag.region");
}

/// Emits the same telemetry into a live-check started by hand, for reading the
/// report on a terminal. See this file's module docs for the two commands.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a live-check receiver started by hand"]
async fn emit_to_a_running_live_check() {
    let endpoint = std::env::var("WEAVER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_owned());
    emit_telemetry(&endpoint).await;
}
