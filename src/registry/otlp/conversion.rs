// SPDX-License-Identifier: Apache-2.0

//! Conversion routines for OTLP to Sample

use serde_json::{json, Value};
use weaver_live_check::{
    sample_attribute::SampleAttribute,
    sample_metric::{DataPoints, SampleMetric},
};
use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};

use super::grpc_stubs::proto::{
    common::v1::{AnyValue, KeyValue},
    metrics::v1::{metric::Data, HistogramDataPoint, Metric, NumberDataPoint},
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
pub fn span_kind_from_otlp_kind(kind: i32) -> SpanKindSpec {
    match kind {
        2 => SpanKindSpec::Server,
        3 => SpanKindSpec::Client,
        4 => SpanKindSpec::Producer,
        5 => SpanKindSpec::Consumer,
        _ => SpanKindSpec::Internal,
    }
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
fn otlp_data_to_instrument(data: &Option<Data>) -> InstrumentSpec {
    match data {
        Some(Data::Sum(sum)) => {
            if sum.is_monotonic {
                InstrumentSpec::Counter
            } else {
                InstrumentSpec::UpDownCounter
            }
        }
        Some(Data::Gauge(_)) => InstrumentSpec::Gauge,
        Some(Data::Histogram(_)) => InstrumentSpec::Histogram,
        _ => InstrumentSpec::Gauge, // TODO Default to Gauge if unknown?
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
        _ => None,
    }
}

/// Converts OTLP Histogram data points to DataPoints::Histogram
fn otlp_histogram_data_points(otlp: &Vec<HistogramDataPoint>) -> DataPoints {
    let mut data_points = Vec::new();
    for point in otlp {
        let live_check_point = weaver_live_check::sample_metric::HistogramDataPoint {
            attributes: point
                .attributes
                .iter()
                .map(sample_attribute_from_key_value)
                .collect(),
            count: point.count,
            sum: point.sum,
            bucket_counts: point.bucket_counts.clone(),
            min: point.min,
            max: point.max,
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
        let live_check_point = weaver_live_check::sample_metric::NumberDataPoint {
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
            attributes: point
                .attributes
                .iter()
                .map(sample_attribute_from_key_value)
                .collect(),
            live_check_result: None,
        };
        data_points.push(live_check_point);
    }
    DataPoints::Number(data_points)
}
