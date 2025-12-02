// SPDX-License-Identifier: Apache-2.0

//! OTLP logger provider for emitting policy findings as log records.

use opentelemetry::logs::LogRecord as _;
use opentelemetry::logs::LoggerProvider;
use opentelemetry::logs::{Logger, Severity};
use opentelemetry::{Key, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::Resource;
use serde_json::Value as JsonValue;
use weaver_checker::{FindingLevel, PolicyFinding};

use crate::{Error, SampleRef};

/// The service name for weaver resources
const WEAVER_SERVICE_NAME: &str = "weaver";

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
            .with_resource(
                Resource::builder()
                    .with_service_name(WEAVER_SERVICE_NAME)
                    .build(),
            )
            .with_batch_exporter(exporter)
            .build();

        Ok(OtlpEmitter { provider })
    }

    /// Create a new OTLP emitter with stdout export (for debugging)
    #[must_use]
    pub fn new_stdout() -> Self {
        let exporter = opentelemetry_stdout::LogExporter::default();

        let provider = SdkLoggerProvider::builder()
            .with_resource(
                Resource::builder()
                    .with_service_name(WEAVER_SERVICE_NAME)
                    .build(),
            )
            .with_simple_exporter(exporter)
            .build();

        OtlpEmitter { provider }
    }

    /// Emit a policy finding as an OTLP log record
    pub fn emit_finding(&self, finding: &PolicyFinding, sample_ref: &SampleRef<'_>) {
        log::debug!("Emitting finding: {} - {}", finding.id, finding.message);
        let severity = finding_level_to_severity(&finding.level);

        // Build attributes from finding and sample context
        let mut attributes = Vec::new();

        // Add finding attributes
        attributes.push(KeyValue::new("weaver.finding.id", finding.id.clone()));
        attributes.push(KeyValue::new(
            "weaver.finding.level",
            finding.level.to_string(),
        ));

        // Flatten and add finding context
        attributes.extend(flatten_finding_context(&finding.context));

        // Add sample context attributes
        attributes.extend(extract_sample_context_attributes(sample_ref, finding));

        // Get a logger from the provider
        let logger = self.provider.logger("weaver.live_check");

        // Create and emit the log record
        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(severity);
        log_record.set_severity_text(severity.name());
        log_record.set_body(finding.message.clone().into());
        log_record.set_event_name("weaver.live_check.finding");

        // Add attributes - convert from KeyValue to individual key/value pairs
        for attr in attributes {
            use opentelemetry::Value;
            match attr.value {
                Value::Bool(b) => log_record.add_attribute(attr.key, b),
                Value::I64(i) => log_record.add_attribute(attr.key, i),
                Value::F64(f) => log_record.add_attribute(attr.key, f),
                Value::String(s) => log_record.add_attribute(attr.key, s.to_string()),
                Value::Array(_) => {} // Skip arrays for now
                _ => {}               // Skip other unsupported types
            }
        }

        logger.emit(log_record);
    }

    /// Shutdown the provider, flushing any pending log records
    pub fn shutdown(&self) -> Result<(), Error> {
        self.provider.shutdown().map_err(|e| Error::OutputError {
            error: format!("Failed to shutdown OTLP log provider: {}", e),
        })
    }
}

/// Convert FindingLevel to OpenTelemetry Severity
fn finding_level_to_severity(level: &FindingLevel) -> Severity {
    match level {
        FindingLevel::Violation => Severity::Error,
        FindingLevel::Improvement => Severity::Warn,
        FindingLevel::Information => Severity::Info,
    }
}

/// Extract sample context attributes for correlation
fn extract_sample_context_attributes(
    sample_ref: &SampleRef<'_>,
    finding: &PolicyFinding,
) -> Vec<KeyValue> {
    let mut attributes = Vec::new();

    // Add sample type
    attributes.push(KeyValue::new(
        "weaver.sample.type",
        sample_ref.sample_type().to_owned(),
    ));

    // Add signal type and name from finding
    if let Some(ref signal_type) = finding.signal_type {
        attributes.push(KeyValue::new(
            "weaver.sample.signal_type",
            signal_type.clone(),
        ));
    }

    if let Some(ref signal_name) = finding.signal_name {
        attributes.push(KeyValue::new(
            "weaver.sample.signal_name",
            signal_name.clone(),
        ));
    }

    attributes
}

/// Flatten finding context JSON into key-value pairs with dot notation
fn flatten_finding_context(context: &JsonValue) -> Vec<KeyValue> {
    let mut attributes = Vec::new();
    flatten_json_recursive(context, "weaver.finding.context", &mut attributes);
    attributes
}

/// Recursively flatten JSON into key-value pairs
fn flatten_json_recursive(value: &JsonValue, prefix: &str, attributes: &mut Vec<KeyValue>) {
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
        JsonValue::String(s) => {
            attributes.push(KeyValue::new(Key::from(prefix.to_owned()), s.clone()));
        }
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                attributes.push(KeyValue::new(Key::from(prefix.to_owned()), i));
            } else if let Some(f) = n.as_f64() {
                attributes.push(KeyValue::new(Key::from(prefix.to_owned()), f));
            }
        }
        JsonValue::Bool(b) => {
            attributes.push(KeyValue::new(Key::from(prefix.to_owned()), *b));
        }
        JsonValue::Null => {
            // Skip null values
        }
    }
}

#[cfg(test)]
mod tests {
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
            .any(|attr| attr.key.as_str() == "weaver.finding.context.key1"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.key2"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.key3"));
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
            attributes[0].key.as_str(),
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
            .any(|attr| attr.key.as_str() == "weaver.finding.context.items.0"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.items.1"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.items.2"));
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
            .any(|attr| attr.key.as_str() == "weaver.finding.context.string"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.int"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.float"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.finding.context.bool"));
    }

    #[test]
    fn test_flatten_json_empty_object() {
        let json = json!({});
        let attributes = flatten_finding_context(&json);
        assert_eq!(attributes.len(), 0);
    }

    #[test]
    fn test_extract_sample_context_attributes_with_signal() {
        let sample = create_test_attribute("test.attribute");
        let sample_ref = SampleRef::Attribute(&sample);
        let finding = create_test_finding(
            "test_id",
            "test message",
            FindingLevel::Information,
            Some("metric"),
            Some("http.request.duration"),
            json!({}),
        );

        let attributes = extract_sample_context_attributes(&sample_ref, &finding);

        assert_eq!(attributes.len(), 3);
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.sample.type"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.sample.signal_type"));
        assert!(attributes
            .iter()
            .any(|attr| attr.key.as_str() == "weaver.sample.signal_name"));
    }

    #[test]
    fn test_extract_sample_context_attributes_without_signal() {
        let sample = create_test_attribute("test.attribute");
        let sample_ref = SampleRef::Attribute(&sample);
        let finding = create_test_finding(
            "test_id",
            "test message",
            FindingLevel::Information,
            None,
            None,
            json!({}),
        );

        let attributes = extract_sample_context_attributes(&sample_ref, &finding);

        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].key.as_str(), "weaver.sample.type");
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

        emitter.emit_finding(&finding, &sample_ref);

        assert!(emitter.shutdown().is_ok());
    }

    #[test]
    fn test_emit_finding_all_severity_levels() {
        let emitter = OtlpEmitter::new_stdout();
        let sample = create_test_attribute("test.attribute");
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
            emitter.emit_finding(&finding, &sample_ref);
        }

        assert!(emitter.shutdown().is_ok());
    }

    #[test]
    fn test_emit_finding_with_complex_context() {
        let emitter = OtlpEmitter::new_stdout();
        let sample = create_test_metric("test.metric");
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

        emitter.emit_finding(&finding, &sample_ref);

        assert!(emitter.shutdown().is_ok());
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
