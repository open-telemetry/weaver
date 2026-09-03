// SPDX-License-Identifier: Apache-2.0

//! Conversion routines for OTLP to Sample

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use weaver_live_check::{
    sample_attribute::SampleAttribute,
    sample_context::SampleContext,
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
    common::v1::{AnyValue, InstrumentationScope, KeyValue},
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

    Some(SampleInstrumentationScope {
        name: scope.map_or_else(String::new, |scope| scope.name.clone()),
        version: scope.map_or_else(String::new, |scope| scope.version.clone()),
        schema_url: schema_url.to_owned(),
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

fn maybe_to_json(value: Option<AnyValue>) -> Option<Value> {
    if let Some(value) = value {
        if let Some(value) = value.value {
            use crate::registry::otlp::grpc_stubs::proto::common::v1::any_value::Value as GrpcValue;
            match value {
                GrpcValue::StringValue(string) => Some(Value::String(string)),
                GrpcValue::IntValue(int_value) => Some(Value::Number(int_value.into())),
                GrpcValue::DoubleValue(double_value) => Some(json!(double_value)),
                GrpcValue::BoolValue(bool_value) => Some(Value::Bool(bool_value)),
                GrpcValue::ArrayValue(array_value) => {
                    let mut vec = Vec::new();
                    for value in array_value.values {
                        if let Some(value) = maybe_to_json(Some(value)) {
                            vec.push(value);
                        }
                    }
                    Some(Value::Array(vec))
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Converts an OTLP KeyValue to a SampleAttribute
pub fn sample_attribute_from_key_value(key_value: &KeyValue) -> SampleAttribute {
    let value = maybe_to_json(key_value.value.clone());
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

/// Converts an OTLP metric to a SampleMetric. `capture_telemetry` controls
/// whether each data point's raw context is captured — see
/// `--capture-telemetry` on `registry live-check`.
pub fn otlp_metric_to_sample(otlp_metric: Metric, capture_telemetry: bool) -> SampleMetric {
    SampleMetric {
        name: otlp_metric.name,
        instrument: otlp_data_to_instrument(&otlp_metric.data),
        unit: otlp_metric.unit,
        data_points: otlp_data_to_data_points(&otlp_metric.data, capture_telemetry),
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
fn otlp_data_to_data_points(data: &Option<Data>, capture_telemetry: bool) -> Option<DataPoints> {
    match data {
        Some(Data::Sum(sum)) => Some(otlp_number_data_points(&sum.data_points, capture_telemetry)),
        Some(Data::Gauge(gauge)) => Some(otlp_number_data_points(
            &gauge.data_points,
            capture_telemetry,
        )),
        Some(Data::Histogram(histogram)) => Some(otlp_histogram_data_points(
            &histogram.data_points,
            capture_telemetry,
        )),
        Some(Data::ExponentialHistogram(exponential_histogram)) => {
            Some(otlp_exponential_histogram_data_points(
                &exponential_histogram.data_points,
                capture_telemetry,
            ))
        }
        _ => None,
    }
}

/// Builds the raw-context (start/end time only; resource and scope are
/// filled in by the caller, which holds them as `Rc`s shared across every
/// data point in the same metric) for one metric data point, or `None` when
/// `--capture-telemetry` wasn't requested.
fn otlp_data_point_context(
    capture_telemetry: bool,
    start_time_unix_nano: u64,
    time_unix_nano: u64,
) -> Option<SampleContext> {
    if !capture_telemetry {
        return None;
    }
    Some(SampleContext {
        start_time: optional_unix_nanos_to_utc(start_time_unix_nano),
        end_time: optional_unix_nanos_to_utc(time_unix_nano),
        ..SampleContext::default()
    })
}

/// Builds a span's raw context (trace/span/parent identity, tracestate,
/// start/end time). Resource and instrumentation scope are filled in by the
/// caller, which holds them as `Rc`s shared with every other span in the
/// same resource/scope. Returns `None` when `--capture-telemetry` wasn't
/// requested.
pub fn otlp_span_context(
    span: &super::grpc_stubs::proto::trace::v1::Span,
    capture_telemetry: bool,
) -> Option<SampleContext> {
    if !capture_telemetry {
        return None;
    }
    Some(SampleContext {
        trace_id: non_empty(trace_id_hex(&span.trace_id)),
        span_id: non_empty(span_id_hex(&span.span_id)),
        parent_span_id: non_empty(span_id_hex(&span.parent_span_id)),
        trace_state: non_empty(span.trace_state.clone()),
        start_time: optional_unix_nanos_to_utc(span.start_time_unix_nano),
        end_time: optional_unix_nanos_to_utc(span.end_time_unix_nano),
        ..SampleContext::default()
    })
}

/// Builds a span event's raw context — only `start_time` applies, since a
/// span event carries one timestamp rather than a range. Returns `None`
/// when `--capture-telemetry` wasn't requested.
pub fn otlp_span_event_context(
    event: &super::grpc_stubs::proto::trace::v1::span::Event,
    capture_telemetry: bool,
) -> Option<SampleContext> {
    if !capture_telemetry {
        return None;
    }
    Some(SampleContext {
        start_time: optional_unix_nanos_to_utc(event.time_unix_nano),
        ..SampleContext::default()
    })
}

/// Builds a span link's raw context — only the *linked* span's
/// `trace_id`/`span_id` apply; a link carries no timestamp of its own.
/// Returns `None` when `--capture-telemetry` wasn't requested.
pub fn otlp_span_link_context(
    link: &super::grpc_stubs::proto::trace::v1::span::Link,
    capture_telemetry: bool,
) -> Option<SampleContext> {
    if !capture_telemetry {
        return None;
    }
    Some(SampleContext {
        trace_id: non_empty(trace_id_hex(&link.trace_id)),
        span_id: non_empty(span_id_hex(&link.span_id)),
        ..SampleContext::default()
    })
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

/// `""` is how every id/timestamp conversion below signals "absent" (a zero
/// or malformed input) — this turns that convention into `Option::None` so
/// `SampleContext` fields don't carry empty strings.
fn non_empty(s: String) -> Option<String> {
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
fn optional_unix_nanos_to_utc(time_unix_nano: u64) -> Option<String> {
    if time_unix_nano == 0 {
        None
    } else {
        non_empty(unix_nanos_to_utc(time_unix_nano))
    }
}

/// Converts a span ID (8 bytes) to a hex string
fn span_id_hex(span_id: &[u8]) -> String {
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
fn trace_id_hex(trace_id: &[u8]) -> String {
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
    capture_telemetry: bool,
) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
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
                context: otlp_data_point_context(
                    capture_telemetry,
                    point.start_time_unix_nano,
                    point.time_unix_nano,
                ),
            };
        data_points.push(live_check_point);
    }
    DataPoints::ExponentialHistogram(data_points)
}

/// Converts OTLP Histogram data points to DataPoints::Histogram
fn otlp_histogram_data_points(
    otlp: &Vec<HistogramDataPoint>,
    capture_telemetry: bool,
) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
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
            context: otlp_data_point_context(
                capture_telemetry,
                point.start_time_unix_nano,
                point.time_unix_nano,
            ),
        };
        data_points.push(live_check_point);
    }
    DataPoints::Histogram(data_points)
}

/// Converts OTLP Number data points to DataPoints::Number
fn otlp_number_data_points(otlp: &Vec<NumberDataPoint>, capture_telemetry: bool) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
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
            context: otlp_data_point_context(
                capture_telemetry,
                point.start_time_unix_nano,
                point.time_unix_nano,
            ),
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
    let value = maybe_to_json(kvu.value.clone());
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

/// Converts an OTLP LogRecord to a SampleLog. `capture_telemetry` controls
/// whether raw context (trace correlation, start_time, resource, scope — the
/// latter two filled in by the caller) is captured — see
/// `--capture-telemetry` on `registry live-check`.
pub fn otlp_log_record_to_sample_log(log_record: &LogRecord, capture_telemetry: bool) -> SampleLog {
    SampleLog {
        event_name: log_record.event_name.clone(),
        severity_number: Some(log_record.severity_number),
        severity_text: Some(log_record.severity_text.clone()),
        body: log_record
            .body
            .as_ref()
            .and_then(|v| v.value.as_ref().map(|val| format!("{:?}", val))),
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
        context: capture_telemetry.then(|| SampleContext {
            trace_id: non_empty(trace_id_hex(&log_record.trace_id)),
            span_id: non_empty(span_id_hex(&log_record.span_id)),
            // OTLP's own recommendation: prefer time_unix_nano, falling
            // back to observed_time_unix_nano when the producer didn't set
            // an event time.
            start_time: optional_unix_nanos_to_utc(log_record.time_unix_nano)
                .or_else(|| optional_unix_nanos_to_utc(log_record.observed_time_unix_nano)),
            ..SampleContext::default()
        }),
    }
}
