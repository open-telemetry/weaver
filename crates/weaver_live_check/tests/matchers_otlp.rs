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
//! Add `--search-all-attributes` to resolve `acme.tenant.id`, which only the
//! dependency registry defines. Without it that attribute reports
//! `missing_attribute`, because the catalog holds this registry's attributes
//! alone.
//!
//! then send it this telemetry, and stop it to print the report:
//!
//! ```text
//! cargo nextest run -p weaver_live_check emit_to_a_running_live_check \
//!   --run-ignored ignored-only
//! curl -X POST http://localhost:4320/stop
//! ```

use std::process::{Child, Command as StdCommand};
use std::thread::sleep;
use std::time::Duration;

use opentelemetry::logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::{
    Link, Span, SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState, Tracer,
    TracerProvider,
};
use opentelemetry::{InstrumentationScope, Key, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use std::collections::BTreeMap;

use serde_json::Value;
use weaver_test_support::reserve_test_port;

const REGISTRY: &str = "data/model/matchers";
const CONFIG: &str = "data/matchers/livecheck.toml";
const ADVICE: &str = "data/matchers/advice";

/// The scope of the spans that carry the checkout flow.
const CHECKOUT_SCOPE: &str = "acme.checkout";
/// The scope a matcher selects on.
const CART_SCOPE: &str = "acme.cart";

/// Kills the child on drop, so a panic does not orphan it.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Polls `/health` until the child answers.
fn wait_for_health(port: u16) {
    let url = format!("http://127.0.0.1:{port}/health");
    for _ in 0..60 {
        if let Ok(response) = ureq::get(&url).call() {
            if response.status() == 200 {
                return;
            }
        }
        sleep(Duration::from_millis(500));
    }
    panic!("live-check never became healthy on port {port}");
}

/// Runs the whole cycle and returns the report the child produces.
async fn run(extra_args: &[&str]) -> Value {
    let grpc_port = reserve_test_port();
    let admin_port = reserve_test_port();

    #[allow(deprecated)] // cargo_bin() is the only cross-crate way to find the binary
    let weaver = assert_cmd::cargo::cargo_bin("weaver");
    let mut guard = ChildGuard(Some(
        StdCommand::new(weaver)
            .args(["registry", "live-check", "-r", REGISTRY, "--v2"])
            .args(["--config", CONFIG, "--advice-policies", ADVICE])
            .args(["--input-source", "otlp", "--format", "json"])
            .args(["--output", "http", "--fail-on", "none"])
            .args(["--otlp-grpc-port", &grpc_port.to_string()])
            .args(["--admin-port", &admin_port.to_string()])
            .args(["--inactivity-timeout", "30"])
            .args(extra_args)
            .spawn()
            .expect("failed to start weaver live-check"),
    ));

    wait_for_health(admin_port);
    emit_telemetry(&format!("http://localhost:{grpc_port}")).await;
    // The exports are acknowledged; give the receiver time to check them.
    sleep(Duration::from_secs(1));

    let body = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{admin_port}/stop"))
        .send()
        .await
        .expect("POST /stop failed")
        .error_for_status()
        .expect("POST /stop returned an error status")
        .text()
        .await
        .expect("failed to read the /stop body");

    let _ = guard
        .0
        .as_mut()
        .expect("child is running")
        .wait()
        .expect("failed to wait for live-check to exit");

    serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("report is not JSON: {error}\n{body}"))
}

/// The resource every provider carries. `acme.tenant.id` is defined only in the
/// dependency registry, which is what `--search-all-attributes` is shown by.
fn resource() -> Resource {
    Resource::builder()
        .with_service_name("acme-checkout")
        .with_attribute(KeyValue::new("acme.tenant.id", "t-42"))
        .build()
}

/// Emits every signal the matchers are written against, then flushes.
async fn emit_telemetry(endpoint: &str) {
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(
            opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()
                .expect("span exporter"),
        )
        .build();
    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(
            opentelemetry_otlp::LogExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()
                .expect("log exporter"),
        )
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource())
        .with_reader(
            PeriodicReader::builder(
                opentelemetry_otlp::MetricExporter::builder()
                    .with_tonic()
                    // Delta, so shutting the provider down does not export the
                    // same points a second time.
                    .with_temporality(Temporality::Delta)
                    .with_endpoint(endpoint)
                    .build()
                    .expect("metric exporter"),
            )
            // Longer than the test, so force_flush is the only export.
            .with_interval(Duration::from_secs(600))
            .build(),
        )
        .build();

    emit_spans(&tracer_provider);
    emit_logs(&logger_provider);
    emit_metrics(&meter_provider);

    // The batch processors export on their own threads; force_flush blocks, so
    // yield first and let a batch go before waiting on one.
    tokio::time::sleep(Duration::from_millis(500)).await;
    tracer_provider.force_flush().expect("flush spans");
    logger_provider.force_flush().expect("flush logs");
    meter_provider.force_flush().expect("flush metrics");
}

fn emit_spans(provider: &SdkTracerProvider) {
    // Matched by `acme.scope.checkout`, whose group declares the session id.
    let checkout = provider.tracer_with_scope(
        InstrumentationScope::builder(CHECKOUT_SCOPE)
            .with_attributes([
                KeyValue::new("acme.session.id", "s-9"),
                KeyValue::new("acme.scope.stray", "x"),
            ])
            .build(),
    );

    // Matched by `acme.checkout.by-name`. `acme.stray.field` is defined
    // nowhere and `acme.tenant.id` only in the dependency, so both are
    // unexpected on the signal and its groups.
    let mut span = checkout
        .span_builder("checkout")
        .with_kind(SpanKind::Server)
        .with_attributes([
            KeyValue::new("acme.checkout.id", "c-1"),
            KeyValue::new("acme.checkout.stage", "payment"),
            KeyValue::new("acme.session.id", "s-9"),
            KeyValue::new("acme.customer.tier", "gold"),
            KeyValue::new("acme.checkout.legacy.id", "l-1"),
            KeyValue::new("acme.request.header.accept", "application/json"),
            KeyValue::new("acme.stray.field", "x"),
            KeyValue::new("acme.tenant.id", "t-42"),
        ])
        .with_links(vec![Link::new(
            SpanContext::new(
                TraceId::from_bytes([1; 16]),
                SpanId::from_bytes([1; 8]),
                TraceFlags::SAMPLED,
                false,
                TraceState::default(),
            ),
            vec![KeyValue::new("acme.session.id", "s-9")],
            0,
        )])
        .start(&checkout);
    span.add_event(
        "acme.checkout.step",
        vec![KeyValue::new("acme.checkout.stage", "cart")],
    );
    span.set_status(Status::Ok);
    span.end();

    // Matched by `acme.checkout.legacy` and `acme.span.on-error`. The registry
    // span is a server span, requires `acme.checkout.id` and recommends
    // `acme.checkout.stage`, and this carries neither.
    let mut span = checkout
        .span_builder("checkout-legacy")
        .with_kind(SpanKind::Client)
        .start(&checkout);
    span.set_status(Status::error("payment declined"));
    span.end();

    // Matched by nothing.
    checkout
        .span_builder("unknown-op")
        .with_kind(SpanKind::Internal)
        .start(&checkout)
        .end();

    // Matched by `acme.cart.by-attribute`, `acme.cart.conflict`,
    // `acme.span.by-scope` and `acme.span.by-resource`. The scope carries an
    // attribute only the dependency defines, and `acme.scope.acme` names no
    // attribute groups, so the scope has no expected set of its own.
    let cart = provider.tracer_with_scope(
        InstrumentationScope::builder(CART_SCOPE)
            .with_attributes([KeyValue::new("acme.tenant.id", "t-42")])
            .build(),
    );
    cart.span_builder("cart")
        .with_kind(SpanKind::Internal)
        // The registry types the count as an int.
        .with_attributes([KeyValue::new("acme.cart.item.count", "three")])
        .start(&cart)
        .end();
}

fn emit_logs(provider: &SdkLoggerProvider) {
    let logger = provider.logger(CHECKOUT_SCOPE);

    let mut record = logger.create_log_record();
    record.set_event_name("acme.checkout.completed");
    record.set_severity_number(Severity::Info);
    record.set_severity_text("INFO");
    record.set_body(AnyValue::from("checkout completed"));
    record.add_attribute(Key::from("acme.checkout.id"), AnyValue::from("c-1"));
    logger.emit(record);

    let mut record = logger.create_log_record();
    record.set_event_name("acme.checkout.failed");
    record.set_severity_number(Severity::Error);
    record.set_severity_text("ERROR");
    record.set_body(AnyValue::from("payment declined"));
    record.add_attribute(Key::from("acme.session.id"), AnyValue::from("s-9"));
    logger.emit(record);

    // Not in the registry; `acme.log.renamed` names its signal.
    let mut record = logger.create_log_record();
    record.set_event_name("acme.checkout.dropped");
    record.set_severity_number(Severity::Info);
    record.set_severity_text("INFO");
    record.set_body(AnyValue::from("checkout abandoned"));
    record.add_attribute(Key::from("acme.session.id"), AnyValue::from("s-9"));
    logger.emit(record);
}

fn emit_metrics(provider: &SdkMeterProvider) {
    let meter = provider.meter(CHECKOUT_SCOPE);

    meter
        .u64_counter("acme.cart.items")
        .with_unit("{item}")
        .build()
        .add(3, &[KeyValue::new("acme.checkout.stage", "cart")]);
    meter
        .f64_histogram("acme.checkout.duration")
        .with_unit("s")
        .build()
        .record(0.42, &[KeyValue::new("acme.checkout.stage", "payment")]);
    // `acme.metric.mismatched` names `acme.checkout.duration`, a histogram in
    // seconds, and the stage is not one of the variants that signal allows.
    meter
        .u64_counter("acme.checkout.attempts")
        .with_unit("{attempt}")
        .build()
        .add(1, &[KeyValue::new("acme.checkout.stage", "refund")]);
    // In neither the registry nor a matcher.
    meter
        .u64_counter("acme.unknown.total")
        .with_unit("{thing}")
        .build()
        .add(1, &[]);
    // Not in the registry; `acme.metric.renamed` names its signal.
    meter
        .u64_counter("acme.legacy.checkout.attempts")
        .with_unit("{attempt}")
        .build()
        .add(1, &[KeyValue::new("acme.checkout.stage", "payment")]);
}

/// What a matcher did, from `statistics.matchers`.
fn matcher<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["statistics"]["matchers"]
        .as_array()
        .expect("statistics carry the matchers")
        .iter()
        .find(|matcher| matcher["id"] == id)
        .unwrap_or_else(|| panic!("no matcher `{id}` in {}", report["statistics"]["matchers"]))
}

/// The span sample with this name.
fn span<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["samples"]
        .as_array()
        .expect("the report carries the samples")
        .iter()
        .map(|sample| &sample["span"])
        .find(|span| span["name"] == name)
        .unwrap_or_else(|| panic!("no span sample named `{name}`"))
}

/// The `acme.source` annotation the policy reported for one of a sample's
/// attributes, which names the definition the checker resolved.
fn annotation_source<'a>(sample: &'a Value, key: &str) -> Option<&'a str> {
    sample["attributes"]
        .as_array()?
        .iter()
        .find(|attribute| attribute["name"] == key)?["live_check_result"]["all_advice"]
        .as_array()?
        .iter()
        .find(|finding| finding["id"] == "annotation_source")?["context"]["source"]
        .as_str()
}

/// The instrumentation scope samples with this name, one per signal type.
fn scopes<'a>(report: &'a Value, name: &str) -> Vec<&'a Value> {
    report["samples"]
        .as_array()
        .expect("the report carries the samples")
        .iter()
        .map(|sample| &sample["instrumentation_scope"])
        .filter(|scope| scope["name"] == name)
        .collect()
}

/// Every finding in the report, from every sample and every nested sample.
fn findings(report: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    collect_findings(&report["samples"], &mut found);
    found
}

fn collect_findings<'a>(value: &'a Value, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Array(advice)) = object
                .get("live_check_result")
                .and_then(|result| result.get("all_advice"))
            {
                found.extend(advice);
            }
            for nested in object.values() {
                collect_findings(nested, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_findings(item, found);
            }
        }
        _ => {}
    }
}

/// The finding with this id that names this attribute.
fn finding_for<'a>(findings: &[&'a Value], id: &str, key: &str) -> &'a Value {
    let found = with_id(findings, id);
    found
        .iter()
        .find(|finding| finding["context"]["attribute_key"] == key)
        .copied()
        .unwrap_or_else(|| panic!("no `{id}` for `{key}`: {found:?}"))
}

/// The findings with this id.
fn with_id<'a>(findings: &[&'a Value], id: &str) -> Vec<&'a Value> {
    findings
        .iter()
        .filter(|finding| finding["id"] == id)
        .copied()
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn matchers_check_telemetry_from_the_sdk() {
    let report = run(&[]).await;
    let findings = findings(&report);

    every_matcher_counts_what_it_matched(&report);
    a_dead_matcher_and_a_failing_one_are_reported(&report);
    the_findings_name_the_telemetry_that_caused_them(&findings);
    the_definition_used_comes_from_the_signal_then_the_first_group(&report);
    span_events_and_links_match_on_their_own(&report);
    a_scope_takes_attribute_groups(&report);
    the_advisors_run_on_the_matched_definitions(&findings);
    a_matcher_signal_drives_the_metric_checks(&findings);
    a_matcher_signal_answers_the_natural_match(&findings);
    the_entity_associations_are_checked_against_the_resource(&findings);
    the_statistics_count_every_finding(&report, &findings);
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

/// The resource carries none of the entity's attributes, and satisfies neither
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
    // A resource has an expected set of its own, from the matcher's group.
    assert!(
        !unexpected.contains(&"service.name"),
        "`acme.service` holds it: {unexpected:?}"
    );
    assert!(
        unexpected.contains(&"telemetry.sdk.language"),
        "got: {unexpected:?}"
    );

    // Signal types arrive on their own export streams, so sort before comparing.
    let mut unmatched: Vec<&str> = with_id(findings, "unmatched_sample")
        .iter()
        .filter_map(|finding| finding["signal_name"].as_str())
        .collect();
    unmatched.sort_unstable();
    assert_eq!(unmatched, ["acme.unknown.total", "unknown-op"]);

    let conflicts = with_id(findings, "matcher_conflict");
    assert_eq!(conflicts.len(), 1, "got: {conflicts:?}");
    let message = conflicts[0]["message"].as_str().expect("a message");
    assert!(
        message.contains("acme.cart.conflict") && message.contains("acme.cart.by-attribute"),
        "got: {message}"
    );

    let kinds = with_id(findings, "kind_mismatch");
    assert_eq!(kinds.len(), 1, "got: {kinds:?}");
    assert_eq!(kinds[0]["signal_name"], "checkout-legacy");

    let missing = with_id(findings, "required_attribute_not_present");
    assert_eq!(missing.len(), 1, "got: {missing:?}");
    assert_eq!(missing[0]["context"]["attribute_key"], "acme.checkout.id");
    assert_eq!(missing[0]["signal_name"], "checkout-legacy");
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
}

/// An instrumentation scope cannot name a signal, but takes attribute groups.
fn a_scope_takes_attribute_groups(report: &Value) {
    // Only the tracer's copy of the scope carries attributes.
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
    let report = run(&["--search-all-attributes"]).await;
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
    let on_the_scope = unexpected
        .iter()
        .find(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("is not declared by this registry"))
        })
        .unwrap_or_else(|| panic!("nothing fell through to a dependency in {unexpected:?}"));
    assert_eq!(on_the_scope["context"]["attribute_key"], "acme.tenant.id");
    assert_eq!(
        on_the_scope["context"]["schema_url"],
        serde_json::json!(["https://acme.example.com/shared/1.0.0"])
    );
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
