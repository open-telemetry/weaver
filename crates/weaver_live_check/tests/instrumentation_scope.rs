// SPDX-License-Identifier: Apache-2.0

//! Compatibility tests for instrumentation scope metadata on live-check signals.

use serde_json::{json, Value};
use weaver_live_check::{sample_instrumentation_scope::SampleInstrumentationScope, Sample};

fn scope_json(name: &str) -> Value {
    json!({
        "name": name,
        "version": "1.2.3",
        "schema_url": "https://opentelemetry.io/schemas/1.32.0",
        "attributes": [{"name": "scope.environment", "value": "test"}],
        "dropped_attributes_count": 2
    })
}

#[test]
fn legacy_signal_json_without_scope_remains_valid_and_omits_the_field() {
    let inputs = [
        json!({"span": {"name": "operation", "kind": "internal"}}),
        json!({"metric": {"name": "requests", "instrument": "counter", "unit": "1"}}),
        json!({"log": {"event_name": "request.completed"}}),
    ];

    for input in inputs {
        let sample: Sample = serde_json::from_value(input).expect("legacy JSON must deserialize");
        let output = serde_json::to_value(sample).expect("sample must serialize");
        let signal = output
            .as_object()
            .and_then(|root| root.values().next())
            .and_then(Value::as_object)
            .expect("sample must contain one signal object");
        assert!(!signal.contains_key("instrumentation_scope"));
    }
}

#[test]
fn scope_metadata_round_trips_for_every_signal_type() {
    let inputs = [
        json!({"span": {
            "name": "operation",
            "kind": "internal",
            "instrumentation_scope": scope_json("trace-library")
        }}),
        json!({"metric": {
            "name": "requests",
            "instrument": "counter",
            "unit": "1",
            "instrumentation_scope": scope_json("metric-library")
        }}),
        json!({"log": {
            "event_name": "request.completed",
            "instrumentation_scope": scope_json("log-library")
        }}),
    ];

    for input in inputs {
        let sample: Sample =
            serde_json::from_value(input.clone()).expect("scoped JSON must deserialize");
        let scope: &SampleInstrumentationScope = match &sample {
            Sample::Span(span) => span.instrumentation_scope.as_ref(),
            Sample::Metric(metric) => metric.instrumentation_scope.as_ref(),
            Sample::Log(log) => log.instrumentation_scope.as_ref(),
            _ => unreachable!("test only supplies whole signals"),
        }
        .expect("scope metadata must be attached to the signal");

        assert_eq!(scope.version, "1.2.3");
        assert_eq!(scope.schema_url, "https://opentelemetry.io/schemas/1.32.0");
        assert_eq!(scope.attributes.len(), 1);
        assert_eq!(scope.dropped_attributes_count, 2);

        let expected_scope = serde_json::to_value(scope).expect("scope must serialize");
        let output = serde_json::to_value(sample).expect("sample must serialize");
        let output_scope = output
            .as_object()
            .and_then(|root| root.values().next())
            .and_then(|signal| signal.get("instrumentation_scope"))
            .expect("serialized signal must contain scope metadata");
        assert_eq!(output_scope, &expected_scope);
    }
}
