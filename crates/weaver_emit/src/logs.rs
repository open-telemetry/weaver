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

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::{Array, Value};

    #[test]
    fn test_value_to_any_value_bool() {
        let result = value_to_any_value(Value::Bool(true));
        assert_eq!(result, AnyValue::Boolean(true));

        let result = value_to_any_value(Value::Bool(false));
        assert_eq!(result, AnyValue::Boolean(false));
    }

    #[test]
    fn test_value_to_any_value_i64() {
        let result = value_to_any_value(Value::I64(42));
        assert_eq!(result, AnyValue::Int(42));

        let result = value_to_any_value(Value::I64(-100));
        assert_eq!(result, AnyValue::Int(-100));

        let result = value_to_any_value(Value::I64(0));
        assert_eq!(result, AnyValue::Int(0));
    }

    #[test]
    fn test_value_to_any_value_f64() {
        let result = value_to_any_value(Value::F64(3.15));
        assert_eq!(result, AnyValue::Double(3.15));

        let result = value_to_any_value(Value::F64(-2.719));
        assert_eq!(result, AnyValue::Double(-2.719));

        let result = value_to_any_value(Value::F64(0.0));
        assert_eq!(result, AnyValue::Double(0.0));
    }

    #[test]
    fn test_value_to_any_value_string() {
        let result = value_to_any_value(Value::String("hello".into()));
        assert_eq!(result, AnyValue::String("hello".into()));

        let result = value_to_any_value(Value::String("".into()));
        assert_eq!(result, AnyValue::String("".into()));
    }

    #[test]
    fn test_value_to_any_value_array_bool() {
        let result = value_to_any_value(Value::Array(Array::Bool(vec![true, false, true])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], AnyValue::Boolean(true));
                assert_eq!(list[1], AnyValue::Boolean(false));
                assert_eq!(list[2], AnyValue::Boolean(true));
            }
            _ => panic!("Expected ListAny"),
        }
    }

    #[test]
    fn test_value_to_any_value_array_i64() {
        let result = value_to_any_value(Value::Array(Array::I64(vec![1, 2, 3])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], AnyValue::Int(1));
                assert_eq!(list[1], AnyValue::Int(2));
                assert_eq!(list[2], AnyValue::Int(3));
            }
            _ => panic!("Expected ListAny"),
        }
    }

    #[test]
    fn test_value_to_any_value_array_f64() {
        let result = value_to_any_value(Value::Array(Array::F64(vec![1.1, 2.2, 3.3])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], AnyValue::Double(1.1));
                assert_eq!(list[1], AnyValue::Double(2.2));
                assert_eq!(list[2], AnyValue::Double(3.3));
            }
            _ => panic!("Expected ListAny"),
        }
    }

    #[test]
    fn test_value_to_any_value_array_string() {
        let result = value_to_any_value(Value::Array(Array::String(vec![
            "foo".into(),
            "bar".into(),
            "baz".into(),
        ])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], AnyValue::String("foo".into()));
                assert_eq!(list[1], AnyValue::String("bar".into()));
                assert_eq!(list[2], AnyValue::String("baz".into()));
            }
            _ => panic!("Expected ListAny"),
        }
    }

    #[test]
    fn test_value_to_any_value_empty_arrays() {
        // Empty bool array
        let result = value_to_any_value(Value::Array(Array::Bool(vec![])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 0);
            }
            _ => panic!("Expected ListAny"),
        }

        // Empty i64 array
        let result = value_to_any_value(Value::Array(Array::I64(vec![])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 0);
            }
            _ => panic!("Expected ListAny"),
        }

        // Empty f64 array
        let result = value_to_any_value(Value::Array(Array::F64(vec![])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 0);
            }
            _ => panic!("Expected ListAny"),
        }

        // Empty string array
        let result = value_to_any_value(Value::Array(Array::String(vec![])));
        match result {
            AnyValue::ListAny(list) => {
                assert_eq!(list.len(), 0);
            }
            _ => panic!("Expected ListAny"),
        }
    }
}
