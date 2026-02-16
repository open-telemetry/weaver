// SPDX-License-Identifier: Apache-2.0

//! OTLP logger provider for emitting policy findings as log records.

use opentelemetry::logs::AnyValue;
use opentelemetry::logs::LogRecord as _;
use opentelemetry::logs::LoggerProvider;
use opentelemetry::logs::{Logger, Severity};
use opentelemetry::Key;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::resource::ResourceDetector;
use opentelemetry_sdk::Resource;
use serde_json::Value as JsonValue;
use weaver_checker::{FindingLevel, PolicyFinding};

use crate::{Error, Sample, SampleRef};

/// Type alias for log attributes as (Key, AnyValue) pairs
type LogAttribute = (Key, AnyValue);

/// The service name for weaver resources
const WEAVER_SERVICE_NAME: &str = "weaver";

/// Custom resource detector that provides weaver-specific defaults
struct WeaverResourceDetector;

impl ResourceDetector for WeaverResourceDetector {
    fn detect(&self) -> Resource {
        // Check if OTEL_SERVICE_NAME is set - if so, don't override it
        if std::env::var("OTEL_SERVICE_NAME").is_ok() {
            return Resource::builder_empty().build();
        }

        // Check if service.name is in OTEL_RESOURCE_ATTRIBUTES
        if let Ok(attrs) = std::env::var("OTEL_RESOURCE_ATTRIBUTES") {
            if attrs.contains("service.name=") {
                return Resource::builder_empty().build();
            }
        }

        // No service name from env, provide our default
        Resource::builder_empty()
            .with_attributes([KeyValue::new("service.name", WEAVER_SERVICE_NAME)])
            .build()
    }
}

/// Build a Resource with environment variable detection and fallback defaults
fn build_resource() -> Resource {
    Resource::builder()
        .with_detector(Box::new(WeaverResourceDetector))
        .build()
}

/// OTLP emitter for policy findings
pub struct OtlpEmitter {
    provider: SdkLoggerProvider,
}

impl OtlpEmitter {
    /// Create a new OTLP emitter with gRPC export
    ///
    /// NOTE: This must be called from within an active Tokio runtime context
    /// because the batch exporter spawns background tasks.
    pub fn new_grpc(endpoint: &str) -> Result<Self, Error> {
        let exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| Error::OutputError {
                error: format!("Failed to create OTLP log exporter: {}", e),
            })?;

        let provider = SdkLoggerProvider::builder()
            .with_resource(build_resource())
            .with_batch_exporter(exporter)
            .build();

        Ok(OtlpEmitter { provider })
    }

    /// Create a new OTLP emitter with stdout export (for debugging)
    #[must_use]
    pub fn new_stdout() -> Self {
        let exporter = opentelemetry_stdout::LogExporter::default();

        let provider = SdkLoggerProvider::builder()
            .with_resource(build_resource())
            .with_simple_exporter(exporter)
            .build();

        OtlpEmitter { provider }
    }

    /// Emit a policy finding as an OTLP log record
    pub fn emit_finding(
        &self,
        finding: &PolicyFinding,
        sample_ref: &SampleRef<'_>,
        parent_signal: &Sample,
    ) {
        log::debug!("Emitting finding: {} - {}", finding.id, finding.message);
        let severity = finding_level_to_severity(&finding.level);
        let attributes = build_finding_attributes(finding, sample_ref, parent_signal);

        // Get a logger from the provider
        let logger = self.provider.logger("weaver.live_check");

        // Create and emit the log record
        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(severity);
        log_record.set_severity_text(severity.name());
        log_record.set_body(finding.message.clone().into());
        log_record.set_event_name("weaver.live_check.finding");

        // Add attributes using the add_attributes method
        log_record.add_attributes(attributes);

        logger.emit(log_record);
    }

    /// Shutdown the provider, flushing any pending log records
    pub fn shutdown(&self) -> Result<(), Error> {
        self.provider.shutdown().map_err(|e| Error::OutputError {
            error: format!("Failed to shutdown OTLP log provider: {}", e),
        })
    }
}

/// Build the attribute list for a finding log record from the finding,
/// sample reference, and parent signal context.
fn build_finding_attributes(
    finding: &PolicyFinding,
    sample_ref: &SampleRef<'_>,
    parent_signal: &Sample,
) -> Vec<LogAttribute> {
    let mut attributes: Vec<LogAttribute> = Vec::new();

    // Add finding attributes
    attributes.push((
        Key::from("weaver.finding.id"),
        AnyValue::from(finding.id.clone()),
    ));
    attributes.push((
        Key::from("weaver.finding.level"),
        AnyValue::from(finding.level.to_string()),
    ));
    attributes.push((
        Key::from("weaver.finding.sample_type"),
        AnyValue::from(sample_ref.sample_type().to_owned()),
    ));

    if let Some(ref signal_type) = finding.signal_type {
        attributes.push((
            Key::from("weaver.finding.signal_type"),
            AnyValue::from(signal_type.clone()),
        ));
    }

    if let Some(ref signal_name) = finding.signal_name {
        attributes.push((
            Key::from("weaver.finding.signal_name"),
            AnyValue::from(signal_name.clone()),
        ));
    }

    // Flatten and add finding context
    attributes.extend(flatten_finding_context(&finding.context));

    // Add resource attributes from the parent signal.
    // Resource attributes are always flat primitives, not nested objects.
    if let Some(resource) = parent_signal.resource() {
        for attr in &resource.attributes {
            if let Some(value) = &attr.value {
                if let Some(any_value) = json_value_to_any_value(value) {
                    let key = format!("weaver.finding.resource_attribute.{}", attr.name);
                    attributes.push((Key::from(key), any_value));
                }
            }
        }
    }

    attributes
}

/// Convert FindingLevel to OpenTelemetry Severity
fn finding_level_to_severity(level: &FindingLevel) -> Severity {
    match level {
        FindingLevel::Violation => Severity::Error,
        FindingLevel::Improvement => Severity::Warn,
        FindingLevel::Information => Severity::Info,
    }
}

/// Flatten finding context JSON into key-value pairs with dot notation
fn flatten_finding_context(context: &JsonValue) -> Vec<LogAttribute> {
    let mut attributes = Vec::new();
    flatten_json_recursive(context, "weaver.finding.context", &mut attributes);
    attributes
}

/// Convert a primitive JSON value to an OpenTelemetry `AnyValue`.
/// Returns `None` for null and object values (objects are not valid OTel attribute values).
fn json_value_to_any_value(value: &JsonValue) -> Option<AnyValue> {
    match value {
        JsonValue::String(s) => Some(AnyValue::from(s.clone())),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(AnyValue::from(i))
            } else {
                n.as_f64().map(AnyValue::from)
            }
        }
        JsonValue::Bool(b) => Some(AnyValue::from(*b)),
        JsonValue::Null | JsonValue::Object(_) | JsonValue::Array(_) => None,
    }
}

/// Recursively flatten JSON into key-value pairs
fn flatten_json_recursive(value: &JsonValue, prefix: &str, attributes: &mut Vec<LogAttribute>) {
    match value {
        JsonValue::Object(map) => {
            for (key, val) in map {
                let new_prefix = format!("{}.{}", prefix, key);
                flatten_json_recursive(val, &new_prefix, attributes);
            }
        }
        JsonValue::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}.{}", prefix, idx);
                flatten_json_recursive(val, &new_prefix, attributes);
            }
        }
        _ => {
            if let Some(any_value) = json_value_to_any_value(value) {
                attributes.push((Key::from(prefix.to_owned()), any_value));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::sample_attribute::SampleAttribute;
    use crate::sample_metric::{SampleInstrument, SampleMetric};
    use crate::sample_resource::SampleResource;
    use crate::sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink, Status, StatusCode};
    use serde_json::json;
    use weaver_checker::{FindingLevel, PolicyFinding};
    use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};

    // Helper function to create a test attribute
    fn create_test_attribute(name: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            value: None,
            r#type: None,
            live_check_result: None,
        }
    }

    // Helper function to create a test span
    fn create_test_span(name: &str) -> SampleSpan {
        SampleSpan {
            name: name.to_owned(),
            kind: SpanKindSpec::Internal,
            status: Some(Status {
                code: StatusCode::Ok,
                message: String::new(),
            }),
            attributes: vec![],
            span_events: vec![],
            span_links: vec![],
            live_check_result: None,
            resource: None,
        }
    }

    // Helper function to create a test metric
    fn create_test_metric(name: &str) -> SampleMetric {
        SampleMetric {
            name: name.to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Gauge),
            unit: "ms".to_owned(),
            data_points: None,
            live_check_result: None,
            resource: None,
        }
    }

    // Helper function to create a test finding
    fn create_test_finding(
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

    #[test]
    fn test_finding_level_to_severity() {
        assert_eq!(
            finding_level_to_severity(&FindingLevel::Violation),
            Severity::Error
        );
        assert_eq!(
            finding_level_to_severity(&FindingLevel::Improvement),
            Severity::Warn
        );
        assert_eq!(
            finding_level_to_severity(&FindingLevel::Information),
            Severity::Info
        );
    }

    #[test]
    fn test_flatten_json_simple_object() {
        let json = json!({
            "key1": "value1",
            "key2": 42,
            "key3": true
        });

        let attributes = flatten_finding_context(&json);

        assert_eq!(attributes.len(), 3);
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.key1"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.key2"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.key3"));
    }

    #[test]
    fn test_flatten_json_nested_object() {
        let json = json!({
            "outer": {
                "inner": {
                    "value": "nested"
                }
            }
        });

        let attributes = flatten_finding_context(&json);

        assert_eq!(attributes.len(), 1);
        assert_eq!(
            attributes[0].0.as_str(),
            "weaver.finding.context.outer.inner.value"
        );
    }

    #[test]
    fn test_flatten_json_array() {
        let json = json!({
            "items": ["first", "second", "third"]
        });

        let attributes = flatten_finding_context(&json);

        assert_eq!(attributes.len(), 3);
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.items.0"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.items.1"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.items.2"));
    }

    #[test]
    fn test_flatten_json_mixed_types() {
        let json = json!({
            "string": "text",
            "int": 123,
            "float": 45.67,
            "bool": false,
            "null": null
        });

        let attributes = flatten_finding_context(&json);

        // null should be skipped
        assert_eq!(attributes.len(), 4);
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.string"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.int"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.float"));
        assert!(attributes
            .iter()
            .any(|attr| attr.0.as_str() == "weaver.finding.context.bool"));
    }

    #[test]
    fn test_flatten_json_empty_object() {
        let json = json!({});
        let attributes = flatten_finding_context(&json);
        assert_eq!(attributes.len(), 0);
    }

    #[test]
    fn test_otlp_emitter_new_stdout() {
        let emitter = OtlpEmitter::new_stdout();
        assert!(emitter.shutdown().is_ok());
    }

    #[tokio::test]
    async fn test_otlp_emitter_new_grpc() {
        // Test with a non-existent endpoint - should create the emitter successfully
        // but won't actually connect until we try to emit
        let result = OtlpEmitter::new_grpc("http://localhost:4317");
        assert!(result.is_ok());

        if let Ok(emitter) = result {
            assert!(emitter.shutdown().is_ok());
        }
    }

    #[test]
    fn test_emit_finding_with_stdout_emitter() {
        let emitter = OtlpEmitter::new_stdout();
        let sample = create_test_span("test.span");
        let parent_signal = Sample::Span(sample.clone());
        let sample_ref = SampleRef::Span(&sample);
        let finding = create_test_finding(
            "test_finding",
            "This is a test finding",
            FindingLevel::Violation,
            Some("span"),
            Some("test.span"),
            json!({
                "attribute": "test.attr",
                "expected": "value"
            }),
        );

        emitter.emit_finding(&finding, &sample_ref, &parent_signal);

        assert!(emitter.shutdown().is_ok());
    }

    #[test]
    fn test_emit_finding_all_severity_levels() {
        let emitter = OtlpEmitter::new_stdout();
        let sample = create_test_attribute("test.attribute");
        let parent_signal = Sample::Attribute(sample.clone());
        let sample_ref = SampleRef::Attribute(&sample);

        for level in [
            FindingLevel::Violation,
            FindingLevel::Improvement,
            FindingLevel::Information,
        ] {
            let finding = create_test_finding(
                &format!("test_{:?}", level),
                &format!("Test {:?} message", level),
                level,
                None,
                None,
                json!({}),
            );
            emitter.emit_finding(&finding, &sample_ref, &parent_signal);
        }

        assert!(emitter.shutdown().is_ok());
    }

    #[test]
    fn test_emit_finding_with_complex_context() {
        let emitter = OtlpEmitter::new_stdout();
        let sample = create_test_metric("test.metric");
        let parent_signal = Sample::Metric(sample.clone());
        let sample_ref = SampleRef::Metric(&sample);
        let finding = create_test_finding(
            "complex_context_test",
            "Testing complex context",
            FindingLevel::Improvement,
            Some("metric"),
            Some("test.metric"),
            json!({
                "nested": {
                    "level1": {
                        "level2": "deep_value"
                    }
                },
                "array": [1, 2, 3],
                "mixed": {
                    "string": "text",
                    "number": 42,
                    "bool": true,
                    "null": null
                }
            }),
        );

        emitter.emit_finding(&finding, &sample_ref, &parent_signal);

        assert!(emitter.shutdown().is_ok());
    }

    #[test]
    fn test_json_value_to_any_value() {
        assert_eq!(
            json_value_to_any_value(&json!("hello")),
            Some(AnyValue::from("hello"))
        );
        assert_eq!(json_value_to_any_value(&json!(42)), Some(AnyValue::Int(42)));
        assert_eq!(
            json_value_to_any_value(&json!(45.67)),
            Some(AnyValue::Double(45.67))
        );
        assert_eq!(
            json_value_to_any_value(&json!(true)),
            Some(AnyValue::Boolean(true))
        );
        assert_eq!(json_value_to_any_value(&json!(null)), None);
        assert_eq!(json_value_to_any_value(&json!({"key": "val"})), None);
        assert_eq!(json_value_to_any_value(&json!([1, 2, 3])), None);
    }

    /// Helper to find an attribute by key in a list of log attributes
    fn find_attr<'a>(attrs: &'a [LogAttribute], key: &str) -> Option<&'a AnyValue> {
        attrs
            .iter()
            .find(|(k, _)| k.as_str() == key)
            .map(|(_, v)| v)
    }

    #[test]
    fn test_build_finding_attributes_includes_resource_attributes() {
        let mut span = create_test_span("test.span");
        span.resource = Some(Rc::new(SampleResource {
            attributes: vec![
                SampleAttribute {
                    name: "service.name".to_owned(),
                    value: Some(json!("my-service")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "service.version".to_owned(),
                    value: Some(json!("1.2.3")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "host.cpu.count".to_owned(),
                    value: Some(json!(4)),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "host.name".to_owned(),
                    value: None, // no value, should be skipped
                    r#type: None,
                    live_check_result: None,
                },
            ],
            live_check_result: None,
        }));
        let parent_signal = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = create_test_finding(
            "test_id",
            "test message",
            FindingLevel::Information,
            Some("span"),
            Some("test.span"),
            json!({}),
        );

        let attrs = build_finding_attributes(&finding, &sample_ref, &parent_signal);

        assert_eq!(
            find_attr(&attrs, "weaver.finding.resource_attribute.service.name"),
            Some(&AnyValue::from("my-service"))
        );
        assert_eq!(
            find_attr(&attrs, "weaver.finding.resource_attribute.service.version"),
            Some(&AnyValue::from("1.2.3"))
        );
        assert_eq!(
            find_attr(&attrs, "weaver.finding.resource_attribute.host.cpu.count"),
            Some(&AnyValue::Int(4))
        );
        // host.name had no value, should not be present
        assert_eq!(
            find_attr(&attrs, "weaver.finding.resource_attribute.host.name"),
            None
        );
    }

    #[test]
    fn test_build_finding_attributes_without_resource() {
        let span = create_test_span("test.span");
        let parent_signal = Sample::Span(span.clone());
        let sample_ref = SampleRef::Span(&span);
        let finding = create_test_finding(
            "test_id",
            "test message",
            FindingLevel::Violation,
            None,
            None,
            json!({}),
        );

        let attrs = build_finding_attributes(&finding, &sample_ref, &parent_signal);

        // No resource attributes should be present
        assert!(!attrs
            .iter()
            .any(|(k, _)| k.as_str().starts_with("weaver.finding.resource_attribute.")));
    }

    #[test]
    fn test_sample_ref_types() {
        let attr_sample = create_test_attribute("test");
        assert_eq!(
            SampleRef::Attribute(&attr_sample).sample_type(),
            "attribute"
        );

        let span_sample = create_test_span("test");
        assert_eq!(SampleRef::Span(&span_sample).sample_type(), "span");

        let event_sample = SampleSpanEvent {
            name: "test".to_owned(),
            attributes: vec![],
            live_check_result: None,
        };
        assert_eq!(
            SampleRef::SpanEvent(&event_sample).sample_type(),
            "span_event"
        );

        let link_sample = SampleSpanLink {
            attributes: vec![],
            live_check_result: None,
        };
        assert_eq!(SampleRef::SpanLink(&link_sample).sample_type(), "span_link");

        let resource_sample = SampleResource {
            attributes: vec![],
            live_check_result: None,
        };
        assert_eq!(
            SampleRef::Resource(&resource_sample).sample_type(),
            "resource"
        );
    }
}
