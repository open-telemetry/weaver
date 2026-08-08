// SPDX-License-Identifier: Apache-2.0

//! Which registry definition `weaver registry live-check --v2` finds for a
//! sample attribute, and what it advises as a result.
//!
//! Two failure modes are covered: no definition is found for an attribute the
//! registry reached through a dependency, and the wrong one is found when a
//! signal refines the attribute for itself.
//!
//! Telemetry is produced with the real OpenTelemetry SDK over OTLP/gRPC and sent
//! to a spawned `weaver registry live-check` process, which returns its JSON
//! report from `POST /stop`. OTLP is used because it is the only way to set
//! resource attributes and a named instrumentation scope, both of which carry
//! attributes that no signal can be matched to.
//!
//! Two fixture registries under `data/model/imported_attributes/` carry
//! identical definitions from different directions: `base` defines everything,
//! `main` defines nothing and imports it all. Sending the same telemetry to both
//! isolates what changes when an attribute arrives through a dependency.
//!
//! Assertions key off finding **ids** and statistics, never rendered message
//! text, so rewording a message cannot silently turn a test green.

use std::process::{Child, Command as StdCommand};
use std::thread::sleep;
use std::time::Duration;

use opentelemetry::logs::{LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::{Span, Tracer, TracerProvider};
use opentelemetry::{InstrumentationScope, KeyValue};
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use serde_json::Value;
use serial_test::serial;
use weaver_test_support::reserve_test_port;

/// Registry that defines every attribute and signal itself.
///
/// Paths are relative to the crate root, which is the working directory for this
/// crate's tests. Unlike `livecheck_emit.rs`, they cannot be made absolute with
/// `CARGO_MANIFEST_DIR`: `main`'s manifest points at `base` with a
/// `registry_path` that the resolver reads relative to the process working
/// directory, so both ends must agree. This matches the `crates/weaver_resolver/data`
/// fixtures.
const DEFINING_REGISTRY: &str = "data/model/imported_attributes/base";
/// Registry that defines nothing and imports everything from `base`.
const IMPORTING_REGISTRY: &str = "data/model/imported_attributes/main";

const SCOPE_NAME: &str = "acme.lib";
const SCOPE_VERSION: &str = "1.2.3";

/// Kills the child on drop so a failing assertion cannot orphan a weaver process.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Poll `GET /health` until the admin server answers.
///
/// The gRPC listener is bound before the admin server starts, so a healthy
/// admin port means the OTLP port is already accepting connections.
fn wait_for_health(port: u16) {
    let url = format!("http://127.0.0.1:{port}/health");
    for attempt in 0..60 {
        match ureq::get(&url).call() {
            Ok(resp) if resp.status() == 200 => return,
            _ => {
                assert!(
                    attempt < 59,
                    "live-check did not become healthy on port {port}"
                );
                sleep(Duration::from_millis(500));
            }
        }
    }
}

/// The resource every session sends. `acme.service.name` is the identity of the
/// `acme.service` entity, so it also satisfies the metric's entity association.
///
/// Built empty on purpose: the SDK's default resource detectors would add
/// `telemetry.sdk.*` and `service.name`, which the fixture registry does not
/// define, drowning the findings under test in unrelated ones.
fn resource() -> Resource {
    Resource::builder_empty()
        .with_attribute(KeyValue::new("acme.service.name", "shoppingcart"))
        .build()
}

/// A scope carrying its own attribute, which reaches live-check as a root
/// `instrumentation_scope` sample.
fn scope() -> InstrumentationScope {
    InstrumentationScope::builder(SCOPE_NAME)
        .with_version(SCOPE_VERSION)
        .with_attributes([KeyValue::new("acme.scope.env", "test")])
        .build()
}

/// Runs one live-check session: spawn, wait, emit, stop, parse the report.
///
/// `emit` receives the OTLP endpoint and must flush before returning. It runs on
/// its own thread with its own multi-threaded runtime, mirroring
/// `weaver_emit::emit`: the SDK's `force_flush` blocks until the batch processor
/// drains, which deadlocks on the current-thread runtime `#[tokio::test]` gives us.
async fn run_session(registry: &str, emit: impl FnOnce(&str) + Send + 'static) -> (Value, i32) {
    let grpc_port = reserve_test_port();
    let admin_port = reserve_test_port();

    #[allow(deprecated)] // cargo_bin() is the only cross-crate way to find the binary
    let weaver_bin = assert_cmd::cargo::cargo_bin("weaver");

    let mut guard = ChildGuard(Some(
        StdCommand::new(weaver_bin)
            .args([
                "registry",
                "live-check",
                "-r",
                registry,
                "--v2",
                "--format",
                "json",
                "--output",
                "http",
                "--otlp-grpc-port",
                &grpc_port.to_string(),
                "--admin-port",
                &admin_port.to_string(),
                "--inactivity-timeout",
                "15",
            ])
            .spawn()
            .expect("failed to start weaver live-check"),
    ));

    wait_for_health(admin_port);

    let endpoint = format!("http://127.0.0.1:{grpc_port}");
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("emit runtime");
        rt.block_on(async move { emit(&endpoint) });
        // The providers are already flushed and shut down; dropping the runtime
        // would otherwise block on the exporter's lingering connection tasks.
        rt.shutdown_timeout(Duration::from_millis(200));
    })
    .join()
    .expect("emit thread panicked");

    // Give the receiver a moment to drain what the exporters flushed.
    sleep(Duration::from_millis(500));

    let body = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .expect("failed to build HTTP/2 client")
        .post(format!("http://127.0.0.1:{admin_port}/stop"))
        .send()
        .await
        .expect("POST /stop failed")
        .error_for_status()
        .expect("POST /stop returned an error status")
        .text()
        .await
        .expect("failed to read the /stop body");

    let status = guard
        .0
        .as_mut()
        .expect("child present")
        .wait()
        .expect("failed to wait for weaver");

    let report: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("report is not valid JSON: {e}\nbody:\n{body}"));

    // Guard against a `/stop` error body (e.g. the 60s report timeout) being
    // mistaken for a clean report: every negative assertion in this file would
    // pass vacuously against an empty sample list.
    let samples = report["samples"]
        .as_array()
        .unwrap_or_else(|| panic!("report has no samples array:\n{report:#}"));
    assert!(
        !samples.is_empty(),
        "live-check received no telemetry, so no assertion below is meaningful:\n{report:#}"
    );

    (report, status.code().unwrap_or(-1))
}

// ---------------------------------------------------------------------------
// Emitters
// ---------------------------------------------------------------------------

/// Resource and instrumentation scope only, carried by a single span.
fn emit_resource_and_scope(endpoint: &str) {
    emit_span(endpoint, &[KeyValue::new("acme.host.id", "abc123")]);
}

/// A span — an untyped carrier, never matched to a registry definition —
/// carrying a key that matches the `acme.header` template.
fn emit_span_with_template_attribute(endpoint: &str) {
    emit_span(
        endpoint,
        &[KeyValue::new("acme.header.accept", "application/json")],
    );
}

fn emit_span(endpoint: &str, attributes: &[KeyValue]) {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .expect("span exporter");
    let provider = SdkTracerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(exporter)
        .build();

    let tracer = provider.tracer_with_scope(scope());
    tracer
        .span_builder("acme.operation")
        .with_attributes(attributes.to_vec())
        .start(&tracer)
        .end();

    provider.force_flush().expect("flush spans");
    provider.shutdown().expect("shutdown tracer provider");
}

/// A metric whose name matches the registry, carrying the attributes that metric
/// declares.
fn emit_matched_metric(endpoint: &str) {
    emit_metric(
        endpoint,
        "acme.uptime",
        &[
            KeyValue::new("acme.host.id", "abc123"),
            KeyValue::new("acme.legacy.id", "L-1"),
        ],
    );
}

/// A metric whose name is NOT in the registry, still carrying a registry
/// attribute.
fn emit_unmatched_metric(endpoint: &str) {
    emit_metric(
        endpoint,
        "acme.uptime.typo",
        &[KeyValue::new("acme.host.id", "abc123")],
    );
}

/// A matched metric carrying a key that matches the `acme.header` template the
/// metric declares.
fn emit_matched_metric_with_template_attribute(endpoint: &str) {
    emit_metric(
        endpoint,
        "acme.uptime",
        &[KeyValue::new("acme.header.accept", "application/json")],
    );
}

/// The same templated key on a metric whose name is NOT in the registry.
fn emit_unmatched_metric_with_template_attribute(endpoint: &str) {
    emit_metric(
        endpoint,
        "acme.uptime.typo",
        &[KeyValue::new("acme.header.accept", "application/json")],
    );
}

/// A matched metric carrying an attribute the registry defines but this metric
/// does not declare.
fn emit_metric_with_undeclared_attribute(endpoint: &str) {
    emit_metric(
        endpoint,
        "acme.uptime",
        &[KeyValue::new("acme.scope.env", "test")],
    );
}

fn emit_metric(endpoint: &str, name: &'static str, attributes: &[KeyValue]) {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .expect("metric exporter");
    let provider = SdkMeterProvider::builder()
        .with_resource(resource())
        .with_reader(PeriodicReader::builder(exporter).build())
        .build();

    provider
        .meter_with_scope(scope())
        .u64_gauge(name)
        .with_unit("s")
        .build()
        .record(1, attributes);

    provider.force_flush().expect("flush metrics");
    provider.shutdown().expect("shutdown meter provider");
}

/// A log record with an `event_name` that matches the registry.
fn emit_matched_event(endpoint: &str) {
    emit_log(
        endpoint,
        Some("acme.request.done"),
        &[("acme.host.id", "abc123"), ("acme.legacy.id", "L-1")],
    );
}

/// A plain log record with NO `event_name`, which live-check cannot match to any
/// registry signal.
fn emit_log_without_event_name(endpoint: &str) {
    emit_log(endpoint, None, &[("acme.host.id", "abc123")]);
}

fn emit_log(endpoint: &str, event_name: Option<&'static str>, attributes: &[(&str, &str)]) {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .expect("log exporter");
    let provider = SdkLoggerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(exporter)
        .build();

    let logger = provider.logger_with_scope(scope());
    let mut record = logger.create_log_record();
    if let Some(name) = event_name {
        record.set_event_name(name);
    }
    record.set_severity_number(Severity::Info);
    record.set_severity_text(Severity::Info.name());
    record.set_body("acme".into());
    for (key, value) in attributes {
        record.add_attribute((*key).to_owned(), (*value).to_owned());
    }
    logger.emit(record);

    provider.force_flush().expect("flush logs");
    provider.shutdown().expect("shutdown logger provider");
}

// ---------------------------------------------------------------------------
// Report helpers
// ---------------------------------------------------------------------------

/// Every finding in the report, as `(id, level)`, with the sample it came from.
fn findings(report: &Value) -> Vec<(String, String, String)> {
    fn walk(value: &Value, sample: &str, out: &mut Vec<(String, String, String)>) {
        match value {
            Value::Object(map) => {
                // The key of a sample wrapper, e.g. {"metric": {...}}, names the
                // sample kind for the findings below it.
                let sample = map
                    .keys()
                    .find(|k| {
                        matches!(
                            k.as_str(),
                            "attribute"
                                | "metric"
                                | "span"
                                | "log"
                                | "resource"
                                | "instrumentation_scope"
                        )
                    })
                    .map_or(sample, |k| k.as_str());
                if let Some(list) = map
                    .get("live_check_result")
                    .and_then(|r| r.get("all_advice"))
                    .and_then(Value::as_array)
                {
                    for advice in list {
                        out.push((
                            advice["id"].as_str().unwrap_or_default().to_owned(),
                            advice["level"].as_str().unwrap_or_default().to_owned(),
                            sample.to_owned(),
                        ));
                    }
                }
                for v in map.values() {
                    walk(v, sample, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, sample, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(report, "root", &mut out);
    out
}

/// Finding ids present anywhere in the report.
fn finding_ids(report: &Value) -> Vec<String> {
    findings(report).into_iter().map(|(id, _, _)| id).collect()
}

/// Asserts no `missing_attribute` finding, printing the offenders.
///
/// `sample_kind` scopes the assertion to attributes carried by that kind of
/// sample; `None` covers the whole report. One session's telemetry carries
/// attributes on several carriers at once — a resource, a scope and a signal —
/// so a test about one of them must not be reddened by another.
fn assert_no_missing_attribute(report: &Value, sample_kind: Option<&str>, context: &str) {
    let missing: Vec<_> = findings(report)
        .into_iter()
        .filter(|(id, _, sample)| {
            id == "missing_attribute" && sample_kind.is_none_or(|kind| sample == kind)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{context}: attributes reachable through an imported signal are part of the \
         registry and must not be reported missing. Got: {missing:?}\n\nreport:\n{report:#}"
    );
}

// ---------------------------------------------------------------------------
// Group 1 — untyped carriers, against the importing registry
// ---------------------------------------------------------------------------

/// A resource attribute has no signal to match against, so it is resolved purely
/// by key.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn resource_attribute_from_dependency_is_not_missing() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_resource_and_scope).await;
    assert_no_missing_attribute(
        &report,
        Some("resource"),
        "resource attribute `acme.service.name`",
    );
}

/// Instrumentation scope attributes take the same untyped path (added by #1605).
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn instrumentation_scope_attribute_from_dependency_is_not_missing() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_resource_and_scope).await;

    let scope_sample = report["samples"]
        .as_array()
        .expect("samples array")
        .iter()
        .find(|s| s.get("instrumentation_scope").is_some())
        .unwrap_or_else(|| panic!("no instrumentation_scope sample in:\n{report:#}"));
    assert_eq!(scope_sample["instrumentation_scope"]["name"], SCOPE_NAME);

    assert_no_missing_attribute(
        &report,
        Some("instrumentation_scope"),
        "scope attribute `acme.scope.env`",
    );
}

/// A log with no `event_name` is explicitly allowed and cannot be matched, but
/// its attributes must still be checked.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn log_without_event_name_still_gets_attribute_advice() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_log_without_event_name).await;
    let ids = finding_ids(&report);
    assert!(
        !ids.contains(&"missing_event".to_owned()),
        "an empty event_name is not a missing event: {ids:?}"
    );
    assert_no_missing_attribute(
        &report,
        Some("log"),
        "attribute on a log with no event_name",
    );
}

/// Spans are never matched to a registry definition, so span attributes are an
/// untyped carrier too.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn span_attributes_from_dependency_are_not_missing() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_resource_and_scope).await;
    assert_no_missing_attribute(&report, Some("span"), "span attribute `acme.host.id`");
}

/// A templated key on an untyped carrier is resolved by prefix against the
/// registry's templates, which an imported signal does not populate either.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn template_attribute_on_an_untyped_carrier_from_dependency_is_not_missing() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_span_with_template_attribute).await;
    let ids = finding_ids(&report);
    assert_no_missing_attribute(
        &report,
        Some("span"),
        "span attribute `acme.header.accept` matches the imported `acme.header` template",
    );
    assert!(
        ids.contains(&"template_attribute".to_owned()),
        "`acme.header.accept` must be recognised as an instance of the `acme.header` \
         template. Findings: {:?}",
        findings(&report)
    );
}

/// A matched signal carries the definition of every attribute it declares, so
/// reaching those attributes through an import must make no difference.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn matched_signal_attributes_from_dependency_are_not_missing() {
    let (metric_report, _) = run_session(IMPORTING_REGISTRY, emit_matched_metric).await;
    assert_no_missing_attribute(
        &metric_report,
        Some("metric"),
        "attributes of the matched metric `acme.uptime`",
    );

    let (event_report, _) = run_session(IMPORTING_REGISTRY, emit_matched_event).await;
    assert_no_missing_attribute(
        &event_report,
        Some("log"),
        "attributes of the matched event `acme.request.done`",
    );
}

/// The templated form of the test above: a matched signal declares the template,
/// so the key that matches it needs no registry-wide definition.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn matched_signal_template_attribute_from_dependency_is_not_missing() {
    let (report, _) = run_session(
        IMPORTING_REGISTRY,
        emit_matched_metric_with_template_attribute,
    )
    .await;
    let ids = finding_ids(&report);
    assert_no_missing_attribute(
        &report,
        Some("metric"),
        "`acme.header.accept` matches the `acme.header` template declared by `acme.uptime`",
    );
    assert!(
        ids.contains(&"template_attribute".to_owned()),
        "`acme.header.accept` must be recognised as an instance of the `acme.header` \
         template. Findings: {:?}",
        findings(&report)
    );
}

/// Telemetry that matches the registry must not fail the run.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn dependency_registry_run_does_not_fail() {
    let (report, exit_code) = run_session(IMPORTING_REGISTRY, emit_matched_metric).await;
    assert_eq!(
        exit_code,
        0,
        "conformant telemetry must not exit non-zero under the default --fail-on violation. \
         Findings: {:?}",
        findings(&report)
    );
}

/// Coverage must count imported attributes as part of the registry.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn dependency_attributes_count_as_registry_attributes() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_matched_metric).await;
    let stats = &report["statistics"];
    assert!(
        stats["seen_registry_attributes"]
            .get("acme.host.id")
            .is_some(),
        "expected `acme.host.id` among the registry attributes, got {}",
        stats["seen_registry_attributes"]
    );
    assert!(
        stats["seen_non_registry_attributes"]
            .get("acme.host.id")
            .is_none(),
        "`acme.host.id` is in the registry, so it must not count as unknown: {}",
        stats["seen_non_registry_attributes"]
    );
}

// ---------------------------------------------------------------------------
// Group 2 — advice must not depend on the signal matching
// ---------------------------------------------------------------------------

/// A misspelled metric name is itself a finding, but the attributes it carries
/// must still be checked.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn unmatched_signal_still_gets_attribute_advice() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_unmatched_metric).await;
    let ids = finding_ids(&report);
    assert!(
        ids.contains(&"missing_metric".to_owned()),
        "an unknown metric name must still be reported: {ids:?}"
    );
    assert_no_missing_attribute(
        &report,
        Some("metric"),
        "attribute carried by an unmatched metric",
    );
}

/// An attribute the registry defines but the matched signal does not declare is
/// still a registry attribute.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn attribute_not_declared_on_the_matched_signal_still_gets_advice() {
    let (report, _) = run_session(IMPORTING_REGISTRY, emit_metric_with_undeclared_attribute).await;
    assert_no_missing_attribute(
        &report,
        Some("metric"),
        "`acme.scope.env` is defined by the registry though `acme.uptime` does not declare it",
    );
}

// ---------------------------------------------------------------------------
// Group 3 — per-signal attribute variants, against the defining registry
// ---------------------------------------------------------------------------

/// `acme.uptime` refines `acme.legacy.id` to `development`, so telemetry
/// carrying it on that metric must draw a `not_stable` finding even though the
/// attribute is `stable` where it is defined.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn per_signal_stability_refinement_is_honoured() {
    let (report, _) = run_session(DEFINING_REGISTRY, emit_matched_metric).await;
    let ids = finding_ids(&report);
    assert!(
        ids.contains(&"not_stable".to_owned()),
        "`acme.uptime` refines `acme.legacy.id` to development, so a not_stable finding is \
         expected. Findings: {:?}\n\nreport:\n{report:#}",
        findings(&report)
    );
}

/// The same attribute on a signal that does not refine it must stay stable. This
/// is what makes the test above a variant test rather than a global-stability
/// test.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn unrefined_attribute_on_another_signal_stays_stable() {
    let (report, _) = run_session(DEFINING_REGISTRY, emit_matched_event).await;
    let ids = finding_ids(&report);
    assert!(
        !ids.contains(&"not_stable".to_owned()),
        "`acme.request.done` does not refine `acme.legacy.id`, so it must not be reported \
         unstable. Findings: {:?}",
        findings(&report)
    );
}

/// The templated form of `per_signal_stability_refinement_is_honoured`.
/// `acme.uptime` refines the `acme.header` template to `development`, so a key
/// matching that template on that metric must draw `not_stable` even though the
/// template is `stable` where it is defined. Reaching the definition by prefix
/// rather than by exact key is the only difference from `acme.legacy.id`.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn per_signal_template_stability_refinement_is_honoured() {
    let (report, _) = run_session(
        DEFINING_REGISTRY,
        emit_matched_metric_with_template_attribute,
    )
    .await;
    let ids = finding_ids(&report);
    assert!(
        ids.contains(&"template_attribute".to_owned()),
        "`acme.header.accept` must be recognised as an instance of the `acme.header` \
         template. Findings: {:?}",
        findings(&report)
    );
    assert!(
        ids.contains(&"not_stable".to_owned()),
        "`acme.uptime` refines the `acme.header` template to development, so a not_stable \
         finding is expected. Findings: {:?}\n\nreport:\n{report:#}",
        findings(&report)
    );
}

/// A key matching the same template on a signal that does not refine it must
/// stay stable. This is what makes the test above a variant test rather than a
/// global-stability test.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn template_attribute_on_an_unmatched_signal_stays_stable() {
    let (report, _) = run_session(
        DEFINING_REGISTRY,
        emit_unmatched_metric_with_template_attribute,
    )
    .await;
    let ids = finding_ids(&report);
    assert!(
        ids.contains(&"missing_metric".to_owned()),
        "the metric name must really be unmatched, or this proves nothing: {ids:?}"
    );
    assert!(
        ids.contains(&"template_attribute".to_owned()),
        "an unmatched signal's attributes are still resolved against the registry's \
         templates. Findings: {:?}",
        findings(&report)
    );
    assert!(
        !ids.contains(&"not_stable".to_owned()),
        "`acme.uptime.typo` matches no signal, so `acme.uptime`'s refinement of the \
         `acme.header` template must not apply. Findings: {:?}",
        findings(&report)
    );
}

// ---------------------------------------------------------------------------
// Group 4 — control
// ---------------------------------------------------------------------------

/// The same telemetry against the registry that owns the definitions. Proves the
/// fixture and harness are sound, and isolates the failures above to the import.
#[tokio::test]
#[cfg_attr(tarpaulin, ignore)]
#[serial]
async fn locally_defined_attributes_are_never_missing() {
    let (report, _) = run_session(DEFINING_REGISTRY, emit_resource_and_scope).await;
    assert_no_missing_attribute(
        &report,
        None,
        "attributes defined by the registry under check",
    );
}
