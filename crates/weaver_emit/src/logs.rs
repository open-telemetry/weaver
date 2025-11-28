// SPDX-License-Identifier: Apache-2.0

//! Translations from Weaver to Otel for log events.

use crate::attributes::{get_attribute_name_value, get_attribute_name_value_v2};
use opentelemetry::logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::{Array, Value};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use weaver_forge::{registry::ResolvedRegistry, v2::registry::ForgeResolvedRegistry};
use weaver_semconv::group::GroupType;

/// Convert an OpenTelemetry Value to AnyValue for log records
fn value_to_any_value(value: Value) -> AnyValue {
    match value {
        Value::Bool(b) => AnyValue::Boolean(b),
        Value::I64(i) => AnyValue::Int(i),
        Value::F64(f) => AnyValue::Double(f),
        Value::String(s) => AnyValue::String(s),
        Value::Array(arr) => match arr {
            Array::Bool(bools) => {
                AnyValue::ListAny(Box::new(bools.into_iter().map(AnyValue::Boolean).collect()))
            }
            Array::I64(ints) => {
                AnyValue::ListAny(Box::new(ints.into_iter().map(AnyValue::Int).collect()))
            }
            Array::F64(floats) => {
                AnyValue::ListAny(Box::new(floats.into_iter().map(AnyValue::Double).collect()))
            }
            Array::String(strings) => AnyValue::ListAny(Box::new(
                strings.into_iter().map(AnyValue::String).collect(),
            )),
            // Handle any future array variants
            _ => AnyValue::String("unsupported_array_type".into()),
        },
        // Handle any future variants by converting to string
        _ => AnyValue::String("unsupported_value".into()),
    }
}

/// Uses the provided logger_provider to emit log records for all the defined
/// events in the registry
pub(crate) fn emit_logs_for_registry(
    registry: &ResolvedRegistry,
    logger_provider: &SdkLoggerProvider,
) {
    let logger = logger_provider.logger("weaver");

    // Emit each event as a log record to the OTLP receiver.
    for group in registry.groups.iter() {
        if group.r#type == GroupType::Event {
            let event_name = group.name.as_ref().unwrap_or(&group.id).clone();

            let mut log_record = logger.create_log_record();
            log_record.set_event_name(Box::leak(event_name.clone().into_boxed_str()));
            log_record.set_severity_number(Severity::Info);
            log_record.set_body(event_name.clone().into());

            // Add attributes from the group
            for attr in &group.attributes {
                let kv = get_attribute_name_value(attr);
                log_record.add_attribute(kv.key, value_to_any_value(kv.value));
            }

            logger.emit(log_record);
        }
    }
}

pub(crate) fn emit_logs_for_registry_v2(
    registry: &ForgeResolvedRegistry,
    logger_provider: &SdkLoggerProvider,
) {
    let logger = logger_provider.logger("weaver");

    // Emit each event as a log record to the OTLP receiver.
    for event in registry.signals.events.iter() {
        let event_name = event.name.to_string();

        let mut log_record = logger.create_log_record();
        log_record.set_event_name(Box::leak(event_name.clone().into_boxed_str()));
        log_record.set_severity_number(Severity::Info);
        log_record.set_body(event_name.clone().into());

        // Add attributes from the event
        for event_attr in &event.attributes {
            let kv = get_attribute_name_value_v2(&event_attr.base);
            log_record.add_attribute(kv.key, value_to_any_value(kv.value));
        }

        logger.emit(log_record);
    }
}
