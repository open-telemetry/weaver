// SPDX-License-Identifier: Apache-2.0

//! Context-carrier tests for instrumentation scope metadata on live-check signals.

use std::rc::Rc;

use serde_json::{json, Value};
use weaver_live_check::{
    sample_attribute::SampleAttribute, sample_instrumentation_scope::SampleInstrumentationScope,
    Sample, SampleRef, SampleType,
};

fn scope(name: &str) -> Rc<SampleInstrumentationScope> {
    Rc::new(SampleInstrumentationScope {
        name: name.to_owned(),
        version: "1.2.3".to_owned(),
        schema_url: "https://opentelemetry.io/schemas/1.32.0".to_owned(),
        attributes: vec![SampleAttribute {
            name: "scope.environment".to_owned(),
            value: Some(json!("test")),
            r#type: None,
            live_check_result: None,
        }],
        dropped_attributes_count: 2,
        live_check_result: None,
    })
}

#[test]
fn instrumentation_scope_serializes_as_a_root_sample_carrier() {
    let instrumentation_scope = scope("library");
    let sample = Sample::InstrumentationScope((*instrumentation_scope).clone());
    assert!(
        sample.instrumentation_scope().is_none(),
        "the root scope is input.sample context, not its own adjacent signal context"
    );
    let value = serde_json::to_value(sample).expect("scope sample serializes");

    assert_eq!(value["instrumentation_scope"]["name"], "library");
    assert_eq!(
        value["instrumentation_scope"]["schema_url"],
        "https://opentelemetry.io/schemas/1.32.0"
    );
    assert_eq!(
        value["instrumentation_scope"]["attributes"][0]["name"],
        "scope.environment"
    );
    let sample_ref = SampleRef::InstrumentationScope(instrumentation_scope.as_ref());
    assert_eq!(sample_ref.sample_type(), SampleType::InstrumentationScope);
    assert_eq!(sample_ref.sample_name(), Some("library"));
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
fn scope_metadata_is_not_serialized_on_every_signal() {
    let inputs = [
        (
            json!({"span": {"name": "operation", "kind": "internal"}}),
            "trace-library",
        ),
        (
            json!({"metric": {"name": "requests", "instrument": "counter", "unit": "1"}}),
            "metric-library",
        ),
        (
            json!({"log": {"event_name": "request.completed"}}),
            "log-library",
        ),
    ];

    for (input, scope_name) in inputs {
        let mut sample: Sample =
            serde_json::from_value(input).expect("sample JSON must deserialize");
        let instrumentation_scope = scope(scope_name);
        match &mut sample {
            Sample::Span(span) => span.instrumentation_scope = Some(instrumentation_scope),
            Sample::Metric(metric) => metric.instrumentation_scope = Some(instrumentation_scope),
            Sample::Log(log) => log.instrumentation_scope = Some(instrumentation_scope),
            _ => unreachable!("test only supplies whole signals"),
        }
        assert_eq!(
            sample
                .instrumentation_scope()
                .expect("signal keeps adjacent scope context")
                .name,
            scope_name
        );

        let output = serde_json::to_value(sample).expect("sample must serialize");
        let signal = output
            .as_object()
            .and_then(|root| root.values().next())
            .and_then(Value::as_object)
            .expect("sample must contain one signal object");
        assert!(
            !signal.contains_key("instrumentation_scope"),
            "shared scope context must not be repeated in serialized signals: {output}"
        );
    }
}
