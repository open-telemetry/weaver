// SPDX-License-Identifier: Apache-2.0

//! Conversion routines for OTLP to Sample

use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use weaver_live_check::{
    sample_attribute::SampleAttribute,
    sample_instrumentation_scope::SampleInstrumentationScope,
    sample_log::SampleLog,
    sample_metric::{DataPoints, SampleInstrument, SampleMetric},
    sample_profile::SampleProfile,
    sample_span::{Status, StatusCode},
};
use weaver_semconv::v1::group::{InstrumentSpec, SpanKindSpec};

use super::grpc_stubs::proto::profiles::v1development::{
    KeyValueAndUnit, Profile, ProfilesDictionary,
};
use super::grpc_stubs::proto::trace::v1::status::StatusCode as OtlpStatusCode;
use super::grpc_stubs::proto::{
    common::v1::{any_value::Value as GrpcValue, AnyValue, InstrumentationScope, KeyValue},
    logs::v1::LogRecord,
    metrics::v1::{metric::Data, HistogramDataPoint, Metric, NumberDataPoint},
    trace::v1::span::SpanKind,
};

/// Converts OTLP instrumentation scope metadata and its containing schema URL.
///
/// A missing scope with an empty schema URL carries no ownership metadata and
/// remains absent. A non-empty schema URL is preserved even when the OTLP scope
/// message itself is missing.
pub fn otlp_instrumentation_scope_to_sample(
    scope: Option<&InstrumentationScope>,
    schema_url: &str,
) -> Option<SampleInstrumentationScope> {
    if scope.is_none() && schema_url.is_empty() {
        return None;
    }

    // In OTLP, proto3 strings default to empty when unset.
    Some(SampleInstrumentationScope {
        name: scope.map_or_else(String::new, |scope| scope.name.clone()),
        version: scope.and_then(|scope| (!scope.version.is_empty()).then(|| scope.version.clone())),
        schema_url: (!schema_url.is_empty()).then(|| schema_url.to_owned()),
        attributes: scope.map_or_else(Vec::new, |scope| {
            scope
                .attributes
                .iter()
                .map(sample_attribute_from_key_value)
                .collect()
        }),
        dropped_attributes_count: scope.map_or(0, |scope| scope.dropped_attributes_count),
        live_check_result: None,
    })
}

/// Converts an OTLP value to plain JSON with bytes as base64 strings; unset values become `None`.
fn maybe_to_json(value: Option<&AnyValue>) -> Option<Value> {
    let value = match value?.value.as_ref()? {
        GrpcValue::StringValue(string) => Value::String(string.clone()),
        GrpcValue::IntValue(int_value) => Value::from(*int_value),
        GrpcValue::DoubleValue(double_value) => json!(double_value),
        GrpcValue::BoolValue(bool_value) => Value::Bool(*bool_value),
        // Unset elements become `null` to preserve the array length.
        GrpcValue::ArrayValue(array_value) => Value::Array(
            array_value
                .values
                .iter()
                .map(|value| maybe_to_json(Some(value)).unwrap_or(Value::Null))
                .collect(),
        ),
        // Unset values become `null`; duplicate keys keep the last value.
        GrpcValue::KvlistValue(kvlist_value) => Value::Object(
            kvlist_value
                .values
                .iter()
                .map(|kv| {
                    (
                        kv.key.clone(),
                        maybe_to_json(kv.value.as_ref()).unwrap_or(Value::Null),
                    )
                })
                .collect(),
        ),
        GrpcValue::BytesValue(bytes_value) => Value::String(BASE64_STANDARD.encode(bytes_value)),
    };
    Some(value)
}

/// Converts an OTLP KeyValue to a SampleAttribute
pub fn sample_attribute_from_key_value(key_value: &KeyValue) -> SampleAttribute {
    let value = maybe_to_json(key_value.value.as_ref());
    let r#type = match value {
        Some(ref val) => SampleAttribute::infer_type(val),
        None => None,
    };
    SampleAttribute {
        name: key_value.key.clone(),
        value,
        r#type,
        live_check_result: None,
    }
}

/// Converts an OTLP span kind to a SpanKindSpec
pub fn span_kind_from_otlp_kind(kind: SpanKind) -> SpanKindSpec {
    match kind {
        SpanKind::Server => SpanKindSpec::Server,
        SpanKind::Client => SpanKindSpec::Client,
        SpanKind::Producer => SpanKindSpec::Producer,
        SpanKind::Consumer => SpanKindSpec::Consumer,
        _ => SpanKindSpec::Internal,
    }
}

/// Converts an OTLP status to a Status
pub fn status_from_otlp_status(
    status: Option<super::grpc_stubs::proto::trace::v1::Status>,
) -> Option<Status> {
    if let Some(status) = status {
        let code = match status.code() {
            OtlpStatusCode::Unset => StatusCode::Unset,
            OtlpStatusCode::Ok => StatusCode::Ok,
            OtlpStatusCode::Error => StatusCode::Error,
        };
        return Some(Status {
            code,
            message: status.message,
        });
    }
    None
}

/// Converts an OTLP metric to a SampleMetric.
pub fn otlp_metric_to_sample(otlp_metric: Metric) -> SampleMetric {
    SampleMetric {
        name: otlp_metric.name,
        instrument: otlp_data_to_instrument(&otlp_metric.data),
        unit: otlp_metric.unit,
        data_points: otlp_data_to_data_points(&otlp_metric.data),
        instrumentation_scope: None,
        live_check_result: None,
        resource: None,
    }
}

/// Converts OTLP data to a SampleMetric instrument
/// Mapping:
/// counter → Sum with is_monotonic: true
/// updowncounter → Sum with is_monotonic: false
/// gauge → Gauge
/// histogram → Histogram
/// histogram → ExponentialHistogram
/// summary → Summary (this will cause a legacy_instrument violation)
fn otlp_data_to_instrument(data: &Option<Data>) -> SampleInstrument {
    match data {
        Some(Data::Sum(sum)) => {
            if sum.is_monotonic {
                SampleInstrument::Supported(InstrumentSpec::Counter)
            } else {
                SampleInstrument::Supported(InstrumentSpec::UpDownCounter)
            }
        }
        Some(Data::Gauge(_)) => SampleInstrument::Supported(InstrumentSpec::Gauge),
        Some(Data::Histogram(_)) => SampleInstrument::Supported(InstrumentSpec::Histogram),
        Some(Data::ExponentialHistogram(_)) => {
            SampleInstrument::Supported(InstrumentSpec::Histogram)
        }
        Some(Data::Summary(_)) => SampleInstrument::Unsupported("Summary".to_owned()),
        None => SampleInstrument::Unsupported("Unspecified".to_owned()),
    }
}

/// Converts OTLP data to SampleMetric data points
fn otlp_data_to_data_points(data: &Option<Data>) -> Option<DataPoints> {
    match data {
        Some(Data::Sum(sum)) => Some(otlp_number_data_points(&sum.data_points)),
        Some(Data::Gauge(gauge)) => Some(otlp_number_data_points(&gauge.data_points)),
        Some(Data::Histogram(histogram)) => {
            Some(otlp_histogram_data_points(&histogram.data_points))
        }
        Some(Data::ExponentialHistogram(exponential_histogram)) => Some(
            otlp_exponential_histogram_data_points(&exponential_histogram.data_points),
        ),
        _ => None,
    }
}

/// Builds the raw-context (start/end time only; resource and scope are
/// filled in by the caller, which holds them as `Rc`s shared across every
/// data point in the same metric) for one metric data point.
fn otlp_data_point_times(
    start_time_unix_nano: u64,
    time_unix_nano: u64,
) -> (Option<String>, Option<String>) {
    (
        optional_unix_nanos_to_utc(start_time_unix_nano),
        optional_unix_nanos_to_utc(time_unix_nano),
    )
}

/// Converts an OTLP Exemplar to a SampleExemplar
fn otlp_exemplar_to_sample_exemplar(
    exemplar: &super::grpc_stubs::proto::metrics::v1::Exemplar,
) -> weaver_live_check::sample_metric::SampleExemplar {
    weaver_live_check::sample_metric::SampleExemplar {
        filtered_attributes: exemplar
            .filtered_attributes
            .iter()
            .map(sample_attribute_from_key_value)
            .collect(),
        value: match &exemplar.value {
            Some(value) => match value {
                super::grpc_stubs::proto::metrics::v1::exemplar::Value::AsDouble(double) => {
                    json!(double)
                }
                super::grpc_stubs::proto::metrics::v1::exemplar::Value::AsInt(int) => {
                    Value::Number((*int).into())
                }
            },
            None => Value::Null,
        },
        timestamp: unix_nanos_to_utc(exemplar.time_unix_nano),
        span_id: span_id_hex(&exemplar.span_id),
        trace_id: trace_id_hex(&exemplar.trace_id),
        live_check_result: None,
    }
}

/// `""` is how every ID conversion below signals an absent or malformed input.
pub(crate) fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Converts a Unix timestamp in nanoseconds to a UTC string
fn unix_nanos_to_utc(time_unix_nano: u64) -> String {
    if let Ok(nanos) = time_unix_nano.try_into() {
        Utc.timestamp_nanos(nanos).to_rfc3339()
    } else {
        "".to_owned()
    }
}

/// Same conversion as `unix_nanos_to_utc`, but for a field that is legitimately
/// unset at zero (a metric data point's `start_time_unix_nano`, for
/// instance) rather than always populated the way a span's timestamps are.
/// `unix_nanos_to_utc(0)` would otherwise render as the 1970 epoch instead
/// of being absent.
pub(crate) fn optional_unix_nanos_to_utc(time_unix_nano: u64) -> Option<String> {
    if time_unix_nano == 0 {
        None
    } else {
        non_empty(unix_nanos_to_utc(time_unix_nano))
    }
}

/// Converts a span ID (8 bytes) to a hex string
pub(super) fn span_id_hex(span_id: &[u8]) -> String {
    if span_id.len() == 8 && span_id.iter().any(|byte| *byte != 0) {
        format!(
            "{:016x}",
            u64::from_be_bytes(span_id[0..8].try_into().unwrap_or([0; 8]))
        )
    } else {
        "".to_owned()
    }
}

/// Converts a trace ID (16 bytes) to a hex string
pub(super) fn trace_id_hex(trace_id: &[u8]) -> String {
    if trace_id.len() == 16 && trace_id.iter().any(|byte| *byte != 0) {
        format!(
            "{:032x}",
            u128::from_be_bytes(trace_id[0..16].try_into().unwrap_or([0; 16]))
        )
    } else {
        "".to_owned()
    }
}

/// Converts OTLP ExponentialHistogram data points to DataPoints::ExponentialHistogram
fn otlp_exponential_histogram_data_points(
    otlp: &Vec<super::grpc_stubs::proto::metrics::v1::ExponentialHistogramDataPoint>,
) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
        let (start_time, end_time) =
            otlp_data_point_times(point.start_time_unix_nano, point.time_unix_nano);
        let positive = point.positive.as_ref().map(|buckets| {
            weaver_live_check::sample_metric::SampleExponentialHistogramBuckets {
                offset: buckets.offset,
                bucket_counts: buckets.bucket_counts.clone(),
            }
        });

        let negative = point.negative.as_ref().map(|buckets| {
            weaver_live_check::sample_metric::SampleExponentialHistogramBuckets {
                offset: buckets.offset,
                bucket_counts: buckets.bucket_counts.clone(),
            }
        });

        let exemplars = point
            .exemplars
            .iter()
            .map(otlp_exemplar_to_sample_exemplar)
            .collect();

        let live_check_point =
            weaver_live_check::sample_metric::SampleExponentialHistogramDataPoint {
                attributes: point
                    .attributes
                    .iter()
                    .map(sample_attribute_from_key_value)
                    .collect(),
                count: point.count,
                sum: point.sum,
                scale: point.scale,
                zero_count: point.zero_count,
                positive,
                negative,
                flags: point.flags,
                min: point.min,
                max: point.max,
                zero_threshold: point.zero_threshold,
                exemplars,
                live_check_result: None,
                start_time,
                end_time,
            };
        data_points.push(live_check_point);
    }
    DataPoints::ExponentialHistogram(data_points)
}

/// Converts OTLP Histogram data points to DataPoints::Histogram
fn otlp_histogram_data_points(otlp: &Vec<HistogramDataPoint>) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
        let (start_time, end_time) =
            otlp_data_point_times(point.start_time_unix_nano, point.time_unix_nano);
        let exemplars = point
            .exemplars
            .iter()
            .map(otlp_exemplar_to_sample_exemplar)
            .collect();

        let live_check_point = weaver_live_check::sample_metric::SampleHistogramDataPoint {
            attributes: point
                .attributes
                .iter()
                .map(sample_attribute_from_key_value)
                .collect(),
            count: point.count,
            sum: point.sum,
            bucket_counts: point.bucket_counts.clone(),
            explicit_bounds: point.explicit_bounds.clone(),
            min: point.min,
            max: point.max,
            flags: point.flags,
            exemplars,
            live_check_result: None,
            start_time,
            end_time,
        };
        data_points.push(live_check_point);
    }
    DataPoints::Histogram(data_points)
}

/// Converts OTLP Number data points to DataPoints::Number
fn otlp_number_data_points(otlp: &Vec<NumberDataPoint>) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
        let (start_time, end_time) =
            otlp_data_point_times(point.start_time_unix_nano, point.time_unix_nano);
        let exemplars = point
            .exemplars
            .iter()
            .map(otlp_exemplar_to_sample_exemplar)
            .collect();

        let live_check_point = weaver_live_check::sample_metric::SampleNumberDataPoint {
            attributes: point
                .attributes
                .iter()
                .map(sample_attribute_from_key_value)
                .collect(),
            value: match point.value {
                Some(value) => match value {
                    super::grpc_stubs::proto::metrics::v1::number_data_point::Value::AsDouble(
                        double,
                    ) => json!(double),
                    super::grpc_stubs::proto::metrics::v1::number_data_point::Value::AsInt(int) => {
                        Value::Number(int.into())
                    }
                },
                None => Value::Null,
            },
            flags: point.flags,
            exemplars,
            live_check_result: None,
            start_time,
            end_time,
        };
        data_points.push(live_check_point);
    }
    DataPoints::Number(data_points)
}

/// Converts an OTLP KeyValueAndUnit to a SampleAttribute, resolving the key from the string table.
pub fn sample_attribute_from_key_value_and_unit(
    kvu: &KeyValueAndUnit,
    string_table: &[String],
) -> SampleAttribute {
    let name = string_table
        .get(kvu.key_strindex as usize)
        .cloned()
        .unwrap_or_default();
    let value = maybe_to_json(kvu.value.as_ref());
    let r#type = match value {
        Some(ref val) => SampleAttribute::infer_type(val),
        None => None,
    };
    SampleAttribute {
        name,
        value,
        r#type,
        live_check_result: None,
    }
}

/// Converts an OTLP Profile to a SampleProfile, resolving attribute indices from the dictionary.
pub fn otlp_profile_to_sample(
    profile: &Profile,
    dictionary: Option<&ProfilesDictionary>,
) -> SampleProfile {
    let attributes = dictionary.map_or_else(Vec::new, |dict| {
        profile
            .attribute_indices
            .iter()
            .filter_map(|&idx| dict.attribute_table.get(idx as usize))
            .map(|kvu| sample_attribute_from_key_value_and_unit(kvu, &dict.string_table))
            .collect()
    });

    SampleProfile {
        original_payload_format: profile.original_payload_format.clone(),
        attributes,
        instrumentation_scope: None,
        live_check_result: None,
        resource: None,
    }
}

/// Converts an OTLP log body to a string: string bodies as-is, other values as compact JSON with sorted keys.
fn log_body_to_string(body: Option<&AnyValue>) -> Option<String> {
    match maybe_to_json(body)? {
        Value::Null => None,
        Value::String(string) => Some(string),
        value => Some(value.to_string()),
    }
}

/// Converts an OTLP LogRecord to a SampleLog.
pub fn otlp_log_record_to_sample_log(log_record: &LogRecord) -> SampleLog {
    SampleLog {
        event_name: log_record.event_name.clone(),
        severity_number: Some(log_record.severity_number),
        severity_text: Some(log_record.severity_text.clone()),
        body: log_body_to_string(log_record.body.as_ref()),
        attributes: log_record
            .attributes
            .iter()
            .map(sample_attribute_from_key_value)
            .collect(),
        trace_id: {
            let trace_id = trace_id_hex(&log_record.trace_id);
            if trace_id.is_empty() {
                None
            } else {
                Some(trace_id)
            }
        },
        span_id: {
            let span_id = span_id_hex(&log_record.span_id);
            if span_id.is_empty() {
                None
            } else {
                Some(span_id)
            }
        },
        instrumentation_scope: None,
        live_check_result: None,
        resource: None,
        timestamp: optional_unix_nanos_to_utc(log_record.time_unix_nano)
            .or_else(|| optional_unix_nanos_to_utc(log_record.observed_time_unix_nano)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::otlp::grpc_stubs::proto::common::v1::{ArrayValue, KeyValueList};
    use weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec;

    fn any(value: GrpcValue) -> AnyValue {
        AnyValue { value: Some(value) }
    }

    fn kv(key: &str, value: GrpcValue) -> KeyValue {
        KeyValue {
            key: key.to_owned(),
            value: Some(any(value)),
        }
    }

    fn kvlist(values: Vec<KeyValue>) -> GrpcValue {
        GrpcValue::KvlistValue(KeyValueList { values })
    }

    fn array(values: Vec<AnyValue>) -> GrpcValue {
        GrpcValue::ArrayValue(ArrayValue { values })
    }

    fn string(value: &str) -> GrpcValue {
        GrpcValue::StringValue(value.to_owned())
    }

    #[test]
    fn primitive_values_convert_to_json() {
        let cases = [
            (string("text"), json!("text")),
            (GrpcValue::IntValue(-42), json!(-42)),
            (GrpcValue::DoubleValue(1.5), json!(1.5)),
            (GrpcValue::BoolValue(true), json!(true)),
        ];
        for (value, expected) in cases {
            assert_eq!(maybe_to_json(Some(&any(value))), Some(expected));
        }
    }

    #[test]
    fn unset_values_convert_to_none() {
        assert_eq!(maybe_to_json(None), None);
        assert_eq!(maybe_to_json(Some(&AnyValue { value: None })), None);

        let attribute = sample_attribute_from_key_value(&KeyValue {
            key: "unset".to_owned(),
            value: None,
        });
        assert_eq!(attribute.value, None);
        assert_eq!(attribute.r#type, None);
    }

    #[test]
    fn kvlist_converts_to_nested_json_object() {
        let value = kvlist(vec![
            kv("role", string("user")),
            kv("count", GrpcValue::IntValue(2)),
            kv(
                "nested",
                kvlist(vec![kv(
                    "tags",
                    array(vec![any(string("a")), any(string("b"))]),
                )]),
            ),
            KeyValue {
                key: "unset".to_owned(),
                value: None,
            },
        ]);

        let attribute = sample_attribute_from_key_value(&kv("map", value));

        assert_eq!(
            attribute.value,
            Some(json!({
                "role": "user",
                "count": 2,
                "nested": {"tags": ["a", "b"]},
                "unset": null
            }))
        );
        assert_eq!(attribute.r#type, None);
    }

    #[test]
    fn array_of_kvlists_keeps_every_element() {
        let value = array(vec![
            any(kvlist(vec![
                kv("role", string("user")),
                kv(
                    "parts",
                    array(vec![any(kvlist(vec![
                        kv("type", string("text")),
                        kv("content", string("hello")),
                    ]))]),
                ),
            ])),
            any(kvlist(vec![kv("role", string("assistant"))])),
        ]);

        let attribute = sample_attribute_from_key_value(&kv("gen_ai.input.messages", value));

        assert_eq!(
            attribute.value,
            Some(json!([
                {"role": "user", "parts": [{"type": "text", "content": "hello"}]},
                {"role": "assistant"}
            ]))
        );
    }

    #[test]
    fn unset_array_elements_become_null() {
        let value = array(vec![
            any(string("a")),
            AnyValue { value: None },
            any(string("c")),
        ]);

        let attribute = sample_attribute_from_key_value(&kv("list", value));

        assert_eq!(attribute.value, Some(json!(["a", null, "c"])));
        assert_eq!(attribute.r#type, Some(PrimitiveOrArrayTypeSpec::Strings));
    }

    #[test]
    fn bytes_convert_to_base64_string() {
        let value = GrpcValue::BytesValue(b"hello\x00\xff".to_vec());

        let attribute = sample_attribute_from_key_value(&kv("payload", value));

        assert_eq!(attribute.value, Some(json!("aGVsbG8A/w==")));
        assert_eq!(attribute.r#type, Some(PrimitiveOrArrayTypeSpec::String));
    }

    #[test]
    fn bytes_nested_in_kvlist_convert_to_base64_string() {
        let value = kvlist(vec![kv("data", GrpcValue::BytesValue(vec![1, 2, 3]))]);

        assert_eq!(
            maybe_to_json(Some(&any(value))),
            Some(json!({"data": "AQID"}))
        );
    }

    #[test]
    fn kvlist_duplicate_keys_keep_last_value() {
        let value = kvlist(vec![kv("key", string("first")), kv("key", string("last"))]);

        assert_eq!(
            maybe_to_json(Some(&any(value))),
            Some(json!({"key": "last"}))
        );
    }

    #[test]
    fn empty_kvlist_and_array_are_preserved() {
        let map = sample_attribute_from_key_value(&kv("map", kvlist(vec![])));
        assert_eq!(map.value, Some(json!({})));
        assert_eq!(map.r#type, None);

        let list = sample_attribute_from_key_value(&kv("list", array(vec![])));
        assert_eq!(list.value, Some(json!([])));
        assert_eq!(list.r#type, None);
    }

    #[test]
    fn nan_double_converts_to_null() {
        let value = array(vec![
            any(GrpcValue::DoubleValue(f64::NAN)),
            any(GrpcValue::DoubleValue(1.5)),
        ]);

        assert_eq!(maybe_to_json(Some(&any(value))), Some(json!([null, 1.5])));
    }

    fn log_with_body(body: Option<AnyValue>) -> SampleLog {
        otlp_log_record_to_sample_log(&LogRecord {
            body,
            ..Default::default()
        })
    }

    #[test]
    fn log_string_body_is_kept_verbatim() {
        let log = log_with_body(Some(any(string("order \"42\" created"))));
        assert_eq!(log.body.as_deref(), Some("order \"42\" created"));
    }

    #[test]
    fn log_primitive_and_bytes_bodies_convert_to_strings() {
        let cases = [
            (GrpcValue::IntValue(42), "42"),
            (GrpcValue::DoubleValue(1.5), "1.5"),
            (GrpcValue::BoolValue(false), "false"),
            (GrpcValue::BytesValue(vec![1, 2, 3]), "AQID"),
        ];
        for (value, expected) in cases {
            assert_eq!(
                log_with_body(Some(any(value))).body.as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn log_structured_body_converts_to_json_string() {
        let body = kvlist(vec![
            kv("message", string("hi")),
            kv(
                "parts",
                array(vec![
                    any(kvlist(vec![kv("type", string("text"))])),
                    AnyValue { value: None },
                ]),
            ),
        ]);

        let log = log_with_body(Some(any(body)));

        let parsed: Value =
            serde_json::from_str(log.body.as_deref().expect("body")).expect("body is JSON");
        assert_eq!(
            parsed,
            json!({"message": "hi", "parts": [{"type": "text"}, null]})
        );
    }

    #[test]
    fn log_unset_body_is_none() {
        assert_eq!(log_with_body(None).body, None);
        assert_eq!(log_with_body(Some(AnyValue { value: None })).body, None);
        assert_eq!(
            log_with_body(Some(any(GrpcValue::DoubleValue(f64::NAN)))).body,
            None
        );
    }

    #[test]
    fn profile_attribute_converts_kvlist() {
        let string_table = vec![String::new(), "profile.config".to_owned()];
        let kvu = KeyValueAndUnit {
            key_strindex: 1,
            value: Some(any(kvlist(vec![kv("rate", GrpcValue::IntValue(100))]))),
            ..Default::default()
        };

        let attribute = sample_attribute_from_key_value_and_unit(&kvu, &string_table);

        assert_eq!(attribute.name, "profile.config");
        assert_eq!(attribute.value, Some(json!({"rate": 100})));
    }

    #[test]
    fn test_otlp_scope_empty_strings_convert_to_none() {
        let scope = InstrumentationScope {
            name: "test-scope".to_owned(),
            version: String::new(),
            attributes: vec![],
            dropped_attributes_count: 0,
        };
        let sample = otlp_instrumentation_scope_to_sample(Some(&scope), "")
            .expect("scope should be converted");
        assert_eq!(sample.name, "test-scope");
        assert_eq!(sample.version, None);
        assert_eq!(sample.schema_url, None);

        let populated = otlp_instrumentation_scope_to_sample(
            Some(&InstrumentationScope {
                version: "1.0.0".to_owned(),
                ..scope
            }),
            "https://opentelemetry.io/schemas/1.32.0",
        )
        .expect("populated scope should convert");
        assert_eq!(populated.version.as_deref(), Some("1.0.0"));
        assert_eq!(
            populated.schema_url.as_deref(),
            Some("https://opentelemetry.io/schemas/1.32.0")
        );
    }
}
