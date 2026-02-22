// SPDX-License-Identifier: Apache-2.0

//! Integration test: emit findings via `OtlpEmitter` into a `weaver registry live-check`
//! instance that uses the live_check model as its registry. Validates that the emitted
//! OTLP log records conform to the model (zero violations).

use std::process::{Child, Command as StdCommand};
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

/// Guard that kills the child process on drop (e.g., on panic) to prevent orphaned processes.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl ChildGuard {
    /// Take ownership of the child, disabling the kill-on-drop behavior.
    fn take(&mut self) -> Child {
        self.0.take().expect("child already taken")
    }
}

use serde_json::json;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_live_check::otlp_logger::OtlpEmitter;
use weaver_live_check::sample_attribute::SampleAttribute;
use weaver_live_check::sample_metric::{SampleInstrument, SampleMetric};
use weaver_live_check::sample_resource::SampleResource;
use weaver_live_check::sample_span::SampleSpan;
use weaver_live_check::{Sample, SampleRef};
use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};

/// Poll GET /health until it returns 200, with retries.
fn wait_for_health(port: u16) {
    let url = format!("http://127.0.0.1:{port}/health");
    for attempt in 0..30 {
        match ureq::get(&url).call() {
            Ok(resp) if resp.status() == 200 => return,
            _ => {
                if attempt == 29 {
                    panic!(
                        "weaver live-check did not become healthy on port {port} after 30 attempts"
                    );
                }
                sleep(Duration::from_millis(200));
            }
        }
    }
}

/// POST /stop and return the response body.
fn stop_and_collect_report(port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/stop");
    let response = ureq::post(&url).send("").expect("POST /stop failed");
    response
        .into_body()
        .read_to_string()
        .expect("Failed to read /stop response body")
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
        context,
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

#[tokio::test]
async fn test_livecheck_emit_roundtrip() {
    // 1. Allocate dynamic ports
    let grpc_port = portpicker::pick_unused_port().expect("no free port for gRPC");
    let admin_port = portpicker::pick_unused_port().expect("no free port for admin");

    // 2. Start weaver live-check as a child process using the live_check model as registry
    //    The model dir is relative to this crate's manifest, so build an absolute path.
    let model_dir = format!("{}/model", env!("CARGO_MANIFEST_DIR"));
    #[allow(deprecated)] // cargo_bin() is the only cross-crate way to find the binary
    let weaver_bin = assert_cmd::cargo::cargo_bin("weaver");
    let mut guard = ChildGuard(Some(
        StdCommand::new(weaver_bin)
            .args([
                "registry",
                "live-check",
                "-r",
                &model_dir,
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
            .expect("Failed to start weaver live-check process"),
    ));

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
            live_check_result: None,
            resource: None,
        };
        let parent = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = make_finding(
            "missing_attribute",
            "Attribute http.request.method is missing",
            FindingLevel::Violation,
            Some("span"),
            Some("http.server"),
            json!({"attribute_name": "http.request.method"}),
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
            json!({"attribute_name": "http.request.method", "stability": "development"}),
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
            json!({"attribute_name": "http.request.method", "attribute_type": "string"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // --- Finding 4: Complex nested context ---
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
                "attribute_name": "db.system",
                "attribute_value": "postgresql",
                "expected": "postgres"
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
            live_check_result: None,
            resource: Some(Rc::new(resource)),
        };
        let parent = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = make_finding(
            "deprecated",
            "Attribute db.statement is deprecated",
            FindingLevel::Improvement,
            Some("span"),
            Some("db.query"),
            json!({"attribute_name": "db.statement", "deprecation_reason": "Use db.query.text"}),
        );
        emitter.emit_finding(&finding, &sample_ref, &parent);
    }

    // 5. Flush then shutdown the emitter.
    //    The batch exporter schedules sends on the Tokio runtime, so we yield
    //    briefly to let the batch task trigger before calling force_flush.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    emitter
        .force_flush()
        .expect("Failed to flush OtlpEmitter");
    emitter.shutdown().expect("Failed to shutdown OtlpEmitter");

    // Brief delay for weaver to finish processing the received log records.
    sleep(Duration::from_millis(500));

    // 6. Collect report via POST /stop
    let report_body = stop_and_collect_report(admin_port);

    // 7. Wait for weaver to exit
    //    Exit code may be non-zero if there are violations — we check that separately below.
    //    Take the child out of the guard so it won't be killed again on drop.
    let _status = guard
        .take()
        .wait()
        .expect("Failed to wait for weaver live-check to exit");

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
