// SPDX-License-Identifier: Apache-2.0

//! Conversion routines for OTLP to Sample

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use weaver_live_check::{
    sample_attribute::SampleAttribute,
    sample_metric::{DataPoints, SampleInstrument, SampleMetric},
    sample_span::{Status, StatusCode},
};
use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};

use super::grpc_stubs::proto::trace::v1::status::StatusCode as OtlpStatusCode;
use super::grpc_stubs::proto::{
    common::v1::{AnyValue, KeyValue},
    metrics::v1::{metric::Data, HistogramDataPoint, Metric, NumberDataPoint},
    trace::v1::span::SpanKind,
};

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

/// Converts an OTLP metric to a SampleMetric
pub fn otlp_metric_to_sample(otlp_metric: Metric) -> SampleMetric {
    SampleMetric {
        name: otlp_metric.name,
        instrument: otlp_data_to_instrument(&otlp_metric.data),
        unit: otlp_metric.unit,
        data_points: otlp_data_to_data_points(&otlp_metric.data),
        live_check_result: None,
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

/// Converts a Unix timestamp in nanoseconds to a UTC string
fn unix_nanos_to_utc(time_unix_nano: u64) -> String {
    if let Ok(nanos) = time_unix_nano.try_into() {
        Utc.timestamp_nanos(nanos).to_rfc3339()
    } else {
        "".to_owned()
    }
}

/// Converts a span ID (8 bytes) to a hex string
fn span_id_hex(span_id: &[u8]) -> String {
    if span_id.len() == 8 {
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
    if trace_id.len() == 16 {
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
            };
        data_points.push(live_check_point);
    }
    DataPoints::ExponentialHistogram(data_points)
}

/// Converts OTLP Histogram data points to DataPoints::Histogram
fn otlp_histogram_data_points(otlp: &Vec<HistogramDataPoint>) -> DataPoints {
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
        };
        data_points.push(live_check_point);
    }
    DataPoints::Histogram(data_points)
}

/// Converts OTLP Number data points to DataPoints::Number
fn otlp_number_data_points(otlp: &Vec<NumberDataPoint>) -> DataPoints {
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
        };
        data_points.push(live_check_point);
    }
    DataPoints::Number(data_points)
}
