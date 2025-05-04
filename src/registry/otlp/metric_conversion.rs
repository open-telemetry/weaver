// SPDX-License-Identifier: Apache-2.0

//! Conversion routines for OTLP to Sample

use weaver_live_check::sample_metric::{Instrument, SampleMetric};

use super::{
    grpc_stubs::proto::metrics::v1::{metric::Data, Metric},
    Error,
};

/// Converts an OTLP metric to a SampleMetric
pub fn otlp_metric_to_sample(otlp_metric: Metric) -> SampleMetric {
    SampleMetric {
        name: otlp_metric.name,
        instrument: otlp_data_to_instrument(otlp_metric.data),
        unit: otlp_metric.unit,
        live_check_result: None,
    }
}

/// Converts OTLP data to a SampleMetric instrument
/// Mapping:
/// counter → Sum with is_monotonic: true
/// updowncounter → Sum with is_monotonic: false
/// gauge → Gauge
/// histogram → Histogram
fn otlp_data_to_instrument(data: Option<Data>) -> Instrument {
    match data {
        Some(Data::Sum(sum)) => {
            if sum.is_monotonic {
                Instrument::Counter
            } else {
                Instrument::UpDownCounter
            }
        }
        Some(Data::Gauge(_)) => Instrument::Gauge,
        Some(Data::Histogram(_)) => Instrument::Histogram,
        _ => Instrument::Gauge, // TODO Default to Gauge if unknown?
    }
}
