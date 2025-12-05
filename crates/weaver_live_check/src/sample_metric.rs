// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_checker::FindingLevel;
use weaver_semconv::group::InstrumentSpec;

use crate::{
    advice::FindingBuilder, live_checker::LiveChecker, sample_attribute::SampleAttribute,
    Advisable, Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
    VersionedSignal, MISSING_METRIC_ADVICE_TYPE,
};

/// Represents the instrument type of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SampleInstrument {
    /// Supported instrument types
    Supported(InstrumentSpec),
    /// Unsupported instrument types
    Unsupported(String),
}

/// The data point types of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DataPoints {
    /// Number data points
    Number(Vec<SampleNumberDataPoint>),
    /// Histogram data points
    Histogram(Vec<SampleHistogramDataPoint>),
    /// Exponential histogram data points
    ExponentialHistogram(Vec<SampleExponentialHistogramDataPoint>),
}

/// Represents a single data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleNumberDataPoint {
    /// The set of key/value pairs that uniquely identify the timeseries from
    /// where this point belongs
    pub attributes: Vec<SampleAttribute>,
    /// The value of the data point, can be a double or int64
    pub value: Value,
    /// Flags that apply to this specific data point
    #[serde(default)]
    pub flags: u32,
    /// List of exemplars collected from measurements that were used to form the data point
    #[serde(default)]
    pub exemplars: Vec<SampleExemplar>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleNumberDataPoint {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::NumberDataPoint(self)
    }

    fn entity_type(&self) -> &str {
        "data_point"
    }
}

impl LiveCheckRunner for SampleNumberDataPoint {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        self.exemplars
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        Ok(())
    }
}

/// Represents a single histogram data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleHistogramDataPoint {
    /// The set of key/value pairs that uniquely identify the timeseries from
    /// where this point belongs
    pub attributes: Vec<SampleAttribute>,
    /// Number of values in the population. Must be non-negative. This
    /// value must be equal to the sum of the "bucket_counts" fields.
    pub count: u64,
    /// Sum of the values in the population. If count is zero then this field
    /// must be zero.
    pub sum: Option<f64>,
    /// Array of bucket counts. The sum of the bucket_counts must equal the value in the count field.
    #[serde(default)]
    pub bucket_counts: Vec<u64>,
    /// Explicit bounds for the bucket boundaries.
    #[serde(default)]
    pub explicit_bounds: Vec<f64>,
    /// Minimum value over the time period
    pub min: Option<f64>,
    /// Maximum value over the time period
    pub max: Option<f64>,
    /// Flags that apply to this specific data point
    #[serde(default)]
    pub flags: u32,
    /// List of exemplars collected from measurements that were used to form the data point
    #[serde(default)]
    pub exemplars: Vec<SampleExemplar>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleHistogramDataPoint {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::HistogramDataPoint(self)
    }

    fn entity_type(&self) -> &str {
        "data_point"
    }
}

impl LiveCheckRunner for SampleHistogramDataPoint {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        self.exemplars
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        Ok(())
    }
}

/// Represents a set of buckets in an exponential histogram
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleExponentialHistogramBuckets {
    /// Bucket index of the first entry in the bucket_counts array
    pub offset: i32,
    /// Array of count values for buckets
    pub bucket_counts: Vec<u64>,
}

/// Represents a single exponential histogram data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleExponentialHistogramDataPoint {
    /// The set of key/value pairs that uniquely identify the timeseries from
    /// where this point belongs
    pub attributes: Vec<SampleAttribute>,
    /// Number of values in the population. Must be non-negative and equal to the sum of
    /// bucket counts plus zero_count
    pub count: u64,
    /// Sum of the values in the population. If count is zero then this field must be zero
    pub sum: Option<f64>,
    /// Resolution of the histogram, defining the power base where base = (2^(2^-scale))
    #[serde(default)]
    pub scale: i32,
    /// Count of values that are exactly zero or within the zero region
    #[serde(default)]
    pub zero_count: u64,
    /// Contains the positive range of exponential bucket counts
    pub positive: Option<SampleExponentialHistogramBuckets>,
    /// Contains the negative range of exponential bucket counts
    pub negative: Option<SampleExponentialHistogramBuckets>,
    /// Flags that apply to this specific data point
    #[serde(default)]
    pub flags: u32,
    /// Minimum value over the time period
    pub min: Option<f64>,
    /// Maximum value over the time period
    pub max: Option<f64>,
    /// Width of the zero region defined as [-ZeroThreshold, ZeroThreshold]
    #[serde(default)]
    pub zero_threshold: f64,
    /// List of exemplars collected from measurements that were used to form the data point
    #[serde(default)]
    pub exemplars: Vec<SampleExemplar>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleExponentialHistogramDataPoint {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::ExponentialHistogramDataPoint(self)
    }

    fn entity_type(&self) -> &str {
        "data_point"
    }
}

impl LiveCheckRunner for SampleExponentialHistogramDataPoint {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        self.exemplars
            .run_live_check(live_checker, stats, parent_group.clone(), parent_signal)?;
        Ok(())
    }
}

/// Represents an exemplar, which is a sample input measurement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleExemplar {
    /// Key/value pairs that were filtered out by the aggregator, but recorded alongside the measurement
    pub filtered_attributes: Vec<SampleAttribute>,
    /// Value of the measurement that was recorded (as double or int)
    pub value: Value,
    /// Time when the exemplar was recorded
    pub timestamp: String,
    /// Span ID of the exemplar trace
    pub span_id: String,
    /// Trace ID of the exemplar trace
    pub trace_id: String,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleExemplar {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::Exemplar(self)
    }

    fn entity_type(&self) -> &str {
        "exemplar"
    }
}

impl LiveCheckRunner for SampleExemplar {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.filtered_attributes.run_live_check(
            live_checker,
            stats,
            parent_group.clone(),
            parent_signal,
        )?;
        Ok(())
    }
}

/// Represents a single summary data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleSummaryDataPoint {
    /// The attributes of the data point
    pub attributes: Vec<SampleAttribute>,
}

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleMetric {
    /// Metric name.
    pub name: String,
    /// Type of the metric (e.g. gauge, histogram, ...).
    pub instrument: SampleInstrument,
    /// Unit of the metric.
    pub unit: String,
    /// Data points of the metric.
    pub data_points: Option<DataPoints>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleMetric {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        // find the metric in the registry
        let semconv_metric = live_checker.find_metric(&self.name);
        if semconv_metric.is_none() {
            let finding = FindingBuilder::new(MISSING_METRIC_ADVICE_TYPE)
                .message("Metric does not exist in the registry.")
                .level(FindingLevel::Violation)
                .signal(parent_signal)
                .build_and_emit(
                    &SampleRef::Metric(self),
                    live_checker.otlp_emitter.as_ref().map(|rc| rc.as_ref()),
                );

            result.add_advice(finding);
        };
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(
                SampleRef::Metric(self),
                parent_signal,
                None,
                semconv_metric.clone(),
                live_checker.otlp_emitter.clone(),
            )?;
            result.add_advice_list(advice_list);
        }
        // Get advice for the data points
        match &mut self.data_points {
            Some(DataPoints::Number(points)) => {
                points.run_live_check(
                    live_checker,
                    stats,
                    semconv_metric.clone(),
                    parent_signal,
                )?;
            }
            Some(DataPoints::Histogram(points)) => {
                points.run_live_check(
                    live_checker,
                    stats,
                    semconv_metric.clone(),
                    parent_signal,
                )?;
            }
            Some(DataPoints::ExponentialHistogram(points)) => {
                points.run_live_check(
                    live_checker,
                    stats,
                    semconv_metric.clone(),
                    parent_signal,
                )?;
            }
            _ => (),
        }

        self.live_check_result = Some(result);
        stats.inc_entity_count("metric");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        stats.add_metric_name_to_coverage(self.name.clone());
        Ok(())
    }
}
