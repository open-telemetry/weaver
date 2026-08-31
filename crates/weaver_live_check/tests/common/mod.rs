// SPDX-License-Identifier: Apache-2.0

//! The OTLP harness the matcher end-to-end tests share: the live-check child
//! process, the telemetry sent to it, and accessors for the report.

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

use serde_json::Value;
use weaver_test_support::reserve_test_port;

pub const REGISTRY: &str = "data/model/matchers";
pub const REGISTRY_V1: &str = "data/model/matchers_v1";
pub const CONFIG: &str = "data/matchers/livecheck.toml";
pub const ADVICE: &str = "data/matchers/advice";

/// The scope of the spans that carry the checkout flow.
pub const CHECKOUT_SCOPE: &str = "acme.checkout";
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
pub async fn run(registry: &str, extra_args: &[&str]) -> Value {
    let grpc_port = reserve_test_port();
    let admin_port = reserve_test_port();

    #[allow(deprecated)] // cargo_bin() is the only cross-crate way to find the binary
    let weaver = assert_cmd::cargo::cargo_bin("weaver");
    let mut guard = ChildGuard(Some(
        StdCommand::new(weaver)
            .args(["registry", "live-check", "-r", registry])
            .args(["--advice-policies", ADVICE])
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
pub async fn emit_telemetry(endpoint: &str) {
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
    // Matched by nothing, and a span event resolves no signal by name.
    span.add_event("acme.checkout.note", Vec::new());
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
    // `acme.span.by-scope` and `acme.span.by-resource`. The scope carries two
    // attributes only the dependency defines, one of them by extending a
    // template, and `acme.scope.acme` names no attribute groups, so the scope
    // has no expected set of its own.
    let cart = provider.tracer_with_scope(
        InstrumentationScope::builder(CART_SCOPE)
            .with_attributes([
                KeyValue::new("acme.tenant.id", "t-42"),
                KeyValue::new("acme.tenant.tag.region", "eu-west"),
            ])
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

    // No event name, so not a typed signal and no matcher looks at it.
    let mut record = logger.create_log_record();
    record.set_severity_number(Severity::Info);
    record.set_severity_text("INFO");
    record.set_body(AnyValue::from("no event name"));
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
        .record(
            0.42,
            &[
                KeyValue::new("acme.checkout.stage", "payment"),
                KeyValue::new("acme.point.stray", "x"),
            ],
        );
    // `acme.metric.mismatched` names `acme.checkout.duration`, a histogram in
    // seconds, and the stage is not one of the variants that signal allows.
    meter
        .u64_counter("acme.checkout.attempts")
        .with_unit("{attempt}")
        .build()
        .add(
            1,
            &[
                KeyValue::new("acme.checkout.stage", "refund"),
                KeyValue::new("acme.checkout.coupon", "SAVE10"),
            ],
        );
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

pub fn span<'a>(report: &'a Value, name: &str) -> &'a Value {
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
pub fn annotation_source<'a>(sample: &'a Value, key: &str) -> Option<&'a str> {
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
pub fn scopes<'a>(report: &'a Value, name: &str) -> Vec<&'a Value> {
    report["samples"]
        .as_array()
        .expect("the report carries the samples")
        .iter()
        .map(|sample| &sample["instrumentation_scope"])
        .filter(|scope| scope["name"] == name)
        .collect()
}

/// Every finding in the report, from every sample and every nested sample.
pub fn findings(report: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    collect_findings(&report["samples"], &mut found);
    found
}

pub fn collect_findings<'a>(value: &'a Value, found: &mut Vec<&'a Value>) {
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
pub fn finding_for<'a>(findings: &[&'a Value], id: &str, key: &str) -> &'a Value {
    let found = with_id(findings, id);
    found
        .iter()
        .find(|finding| finding["context"]["attribute_key"] == key)
        .copied()
        .unwrap_or_else(|| panic!("no `{id}` for `{key}`: {found:?}"))
}

/// The findings with this id.
pub fn with_id<'a>(findings: &[&'a Value], id: &str) -> Vec<&'a Value> {
    findings
        .iter()
        .filter(|finding| finding["id"] == id)
        .copied()
        .collect()
}
