// SPDX-License-Identifier: Apache-2.0

//! Integration test: emit findings via `OtlpEmitter` into a `weaver registry live-check`
//! instance that uses the live_check model as its registry. Validates that the emitted
//! OTLP log records conform to the model (zero violations).
//!
//! It also guards the fix for #1657: the report is read from `GET /report` while the
//! process is alive, so a client that drains a large body late still gets all of it.

mod common;

use std::io::Read;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

use common::{shutdown, start_live_check, stop, wait_for_health, ChildGuard};
use weaver_test_support::reserve_test_port;

use serde_json::json;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_live_check::otlp_logger::OtlpEmitter;
use weaver_live_check::sample_attribute::SampleAttribute;
use weaver_live_check::sample_log::SampleLog;
use weaver_live_check::sample_metric::{SampleInstrument, SampleMetric};
use weaver_live_check::sample_resource::SampleResource;
use weaver_live_check::sample_span::SampleSpan;
use weaver_live_check::{Sample, SampleRef};
use weaver_semconv::v1::group::{InstrumentSpec, SpanKindSpec};

/// Larger than a socket send buffer, so the server cannot finish writing the
/// report until the client reads it. Kept under tonic's 4 MiB gRPC message
/// limit (travels as a flattened `weaver.finding.context.padding` attribute —
/// see Finding 4).
const RESPONSE_PADDING_SIZE: usize = 2 * 1024 * 1024;

/// Delay between receiving the `/report` headers and reading its body. A busy
/// CI client drains late; #1657 showed that used to truncate the body.
const DELAYED_READ: Duration = Duration::from_secs(1);

/// `GET /report` over HTTP/1.1, read the headers, wait, then drain the body.
/// Asserts the child is still alive and that the whole body arrived.
fn collect_report_slowly(admin_port: u16, guard: &mut ChildGuard) -> String {
    let response = ureq::get(format!("http://127.0.0.1:{admin_port}/report"))
        .call()
        .expect("GET /report failed");
    assert_eq!(response.status(), 200);
    let content_length: usize = response
        .headers()
        .get("content-length")
        .expect("/report sets Content-Length")
        .to_str()
        .expect("Content-Length is ASCII")
        .parse()
        .expect("Content-Length is a number");

    sleep(DELAYED_READ);

    assert!(
        guard
            .0
            .as_mut()
            .expect("child is running")
            .try_wait()
            .expect("failed to check child process status")
            .is_none(),
        "weaver exited before the /report response was fully read"
    );

    let mut body = String::new();
    let _ = response
        .into_body()
        .as_reader()
        .read_to_string(&mut body)
        .expect("failed to read the /report body");
    assert_eq!(body.len(), content_length, "the /report body was cut short");
    body
}

fn make_finding(
    id: &str,
    message: &str,
    level: FindingLevel,
    signal_type: Option<&str>,
    signal_name: Option<&str>,
    context: serde_json::Value,
) -> PolicyFinding {
    PolicyFinding {
        id: id.to_owned(),
        message: message.to_owned(),
        level,
        signal_type: signal_type.map(|s| s.to_owned()),
        signal_name: signal_name.map(|s| s.to_owned()),
        context: Some(context),
    }
}

fn make_attribute(name: &str) -> SampleAttribute {
    SampleAttribute {
        name: name.to_owned(),
        value: None,
        r#type: None,
        live_check_result: None,
    }
}

/// Recursively collect violation messages from a JSON report.
fn collect_violation_messages(value: &serde_json::Value, messages: &mut Vec<String>) {
    if let Some(obj) = value.as_object() {
        if let Some(result) = obj.get("live_check_result") {
            if let Some(advice_list) = result.get("all_advice").and_then(|a| a.as_array()) {
                for advice in advice_list {
                    if advice.get("level").and_then(|l| l.as_str()) == Some("violation") {
                        let msg = advice
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("<no message>");
                        messages.push(msg.to_owned());
                    }
                }
            }
        }
        for (_k, v) in obj {
            collect_violation_messages(v, messages);
        }
    } else if let Some(arr) = value.as_array() {
        for item in arr {
            collect_violation_messages(item, messages);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(tarpaulin, ignore)]
async fn test_livecheck_emit_roundtrip() {
    // 1. Allocate dynamic ports
    let grpc_port = reserve_test_port();
    let admin_port = reserve_test_port();

    // 2. Start weaver live-check as a child process using the live_check model as registry
    //    The model dir is relative to this crate's manifest, so build an absolute path.
    let model_dir = format!("{}/model", env!("CARGO_MANIFEST_DIR"));
    let mut guard = start_live_check(&model_dir, grpc_port, admin_port, &[]);

    // 3. Wait for the health endpoint to respond
    wait_for_health(admin_port);

    // 4. Create OtlpEmitter and emit diverse findings
    let endpoint = format!("http://localhost:{grpc_port}");
    let emitter = OtlpEmitter::new_grpc(&endpoint).expect("Failed to create OtlpEmitter");

    // --- Finding 1: Violation with span sample ---
    {
        let span = SampleSpan {
            name: "http.server".to_owned(),
            kind: SpanKindSpec::Server,
            status: None,
            attributes: vec![],
            span_events: vec![],
            span_links: vec![],
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
            trace_id: Some("00000000000000000000000000000001".to_owned()),
            span_id: Some("0000000000000001".to_owned()),
            parent_span_id: None,
            trace_state: None,
            start_time: None,
            end_time: None,
        };
        let parent = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = make_finding(
            "missing_attribute",
            "Attribute http.request.method is missing",
            FindingLevel::Violation,
            Some("span"),
            Some("http.server"),
            json!({"attribute_key": "http.request.method"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 2: Improvement with metric sample ---
    {
        let metric = SampleMetric {
            name: "http.server.request.duration".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "s".to_owned(),
            data_points: None,
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
        };
        let parent = Sample::Metric(metric.clone());
        let sample_ref = SampleRef::Metric(&metric);
        let finding = make_finding(
            "not_stable",
            "Attribute http.request.method has development stability",
            FindingLevel::Improvement,
            Some("metric"),
            Some("http.server.request.duration"),
            json!({"attribute_key": "http.request.method", "stability": "development"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 3: Information with attribute sample ---
    {
        let attr = make_attribute("http.request.method");
        let parent = Sample::Attribute(attr.clone());
        let sample_ref = SampleRef::Attribute(&attr);
        let finding = make_finding(
            "type_mismatch",
            "Expected string, got int",
            FindingLevel::Information,
            None,
            None,
            json!({"attribute_key": "http.request.method", "attribute_type": "string"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 4: Complex nested context ---
    // Carries a large padding value to reproduce the shutdown race below.
    {
        let attr = make_attribute("db.system");
        let parent = Sample::Attribute(attr.clone());
        let sample_ref = SampleRef::Attribute(&attr);
        let finding = make_finding(
            "undefined_enum_variant",
            "Enum variant 'postgresql' is not defined",
            FindingLevel::Violation,
            None,
            None,
            json!({
                "attribute_key": "db.system",
                "attribute_value": "postgresql",
                "expected": "postgres",
                "padding": "x".repeat(RESPONSE_PADDING_SIZE),
            }),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 5: Finding with resource attributes on parent signal ---
    {
        let resource = SampleResource {
            attributes: vec![
                SampleAttribute {
                    name: "service.name".to_owned(),
                    value: Some(json!("my-test-service")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "service.version".to_owned(),
                    value: Some(json!("1.0.0")),
                    r#type: None,
                    live_check_result: None,
                },
            ],
            live_check_result: None,
        };
        let span = SampleSpan {
            name: "db.query".to_owned(),
            kind: SpanKindSpec::Client,
            status: None,
            attributes: vec![],
            span_events: vec![],
            span_links: vec![],
            instrumentation_scope: None,
            live_check_result: None,
            resource: Some(Rc::new(resource)),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            trace_state: None,
            start_time: None,
            end_time: None,
        };
        let parent = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = make_finding(
            "deprecated",
            "Attribute db.statement is deprecated",
            FindingLevel::Improvement,
            Some("span"),
            Some("db.query"),
            json!({"attribute_key": "db.statement", "deprecation_reason": "Use db.query.text"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 6: Entity required attribute not present (log sample with resource) ---
    {
        let resource = SampleResource {
            attributes: vec![SampleAttribute {
                name: "service.name".to_owned(),
                value: Some(json!("my-test-service")),
                r#type: None,
                live_check_result: None,
            }],
            live_check_result: None,
        };
        let log = SampleLog {
            event_name: "deployment.started".to_owned(),
            severity_number: None,
            severity_text: None,
            body: None,
            attributes: vec![],
            trace_id: None,
            span_id: None,
            instrumentation_scope: None,
            live_check_result: None,
            resource: Some(Rc::new(resource)),
            timestamp: None,
        };
        let parent = Sample::Log(log.clone());
        let sample_ref = SampleRef::Log(&log);
        let finding = make_finding(
            "entity_required_attribute_not_present",
            "Required attribute 'deployment.name' for entity 'deployment' is not present in the resource.",
            FindingLevel::Violation,
            Some("log"),
            Some("deployment.started"),
            json!({"attribute_key": "deployment.name", "entity_type": "deployment"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // 5. Flush then shutdown the emitter.
    //    The batch exporter schedules sends on the Tokio runtime, so we yield
    //    briefly to let the batch task trigger before calling force_flush.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    emitter.force_flush().expect("Failed to flush OtlpEmitter");
    emitter.shutdown().expect("Failed to shutdown OtlpEmitter");

    // 6. Stop the run. The report is ready once /stop returns, so no sleep is needed.
    stop(admin_port);

    // 7. Read the (large) report late, the way a busy client does; see
    //    `collect_report_slowly`. Then shut weaver down and wait for it to exit.
    //    Exit code may be non-zero if there are violations — we check that separately below.
    let report_body = collect_report_slowly(admin_port, &mut guard);
    shutdown(guard, admin_port);

    // 8. Validate the report
    let report: serde_json::Value =
        serde_json::from_str(&report_body).expect("Failed to parse live-check report as JSON");

    let statistics = report["statistics"]
        .as_object()
        .expect("Report should have a statistics object");

    let total_entities = statistics["total_entities"]
        .as_u64()
        .expect("total_entities should be a u64");
    assert!(
        total_entities > 0,
        "Expected total_entities > 0 (data flowed through), got {total_entities}"
    );

    let samples = report["samples"].as_array().expect("report samples");
    let emitted_finding = samples
        .iter()
        .filter_map(|sample| sample.get("log"))
        .find(|log| {
            log["trace_id"] == "00000000000000000000000000000001"
                && log["span_id"] == "0000000000000001"
        })
        .expect("captured finding log correlated with its source span");
    assert!(
        emitted_finding["timestamp"].is_string(),
        "captured finding log should retain its OTLP timestamp"
    );
    let attributes = emitted_finding["attributes"]
        .as_array()
        .expect("captured finding log attributes");
    assert!(
        attributes
            .iter()
            .any(|attribute| attribute["name"] == "weaver.finding.id"),
        "captured finding log should retain generated finding attributes"
    );
    assert!(
        attributes
            .iter()
            .any(|attribute| attribute["name"] == "weaver.finding.context.attribute_key"),
        "captured finding log should retain generated finding context attributes"
    );

    // Read violation count from statistics
    let violation_count = statistics
        .get("advice_level_counts")
        .and_then(|c| c.get("violation"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_advisories = statistics
        .get("total_advisories")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let no_advice_count = statistics
        .get("no_advice_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let registry_coverage = statistics
        .get("registry_coverage")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    eprintln!("--- Live-check emit roundtrip report ---");
    eprintln!("  total_entities:     {total_entities}");
    eprintln!("  total_advisories:   {total_advisories}");
    eprintln!("  no_advice_count:    {no_advice_count}");
    eprintln!("  violations found:   {violation_count}");
    eprintln!("  registry_coverage:  {registry_coverage:.1}%");

    // Collect and print any violation messages, then assert zero violations.
    if violation_count > 0 {
        let mut violation_messages = Vec::new();
        if let Some(samples) = report["samples"].as_array() {
            for sample in samples {
                for (_key, entity) in sample.as_object().into_iter().flatten() {
                    collect_violation_messages(entity, &mut violation_messages);
                }
            }
        }
        for msg in &violation_messages {
            eprintln!("  VIOLATION: {msg}");
        }
    }

    assert_eq!(
        violation_count, 0,
        "Expected zero violations (emitted findings should conform to the model), \
         but found {violation_count}. This indicates the generated OTLP log records \
         do not match the live_check.yaml model."
    );

    assert!(
        (registry_coverage - 1.0).abs() < f64::EPSILON,
        "Expected 100% registry coverage, got {registry_coverage:.4}. \
         All attributes and events in the model should be seen in the emitted findings."
    );
}
