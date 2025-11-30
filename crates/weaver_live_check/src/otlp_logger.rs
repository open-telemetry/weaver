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
