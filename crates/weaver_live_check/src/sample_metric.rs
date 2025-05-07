// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_semconv::group::InstrumentSpec;

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Error, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, SampleRef, MISSING_METRIC_ADVICE_TYPE,
};

/// The data point types of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataPoints {
    /// Number data points
    Number(Vec<SampleNumberDataPoint>),
    /// Histogram data points
    Histogram(Vec<SampleHistogramDataPoint>),
    /// Exponential histogram data points
    ExponentialHistogram(Vec<SampleExponentialHistogramDataPoint>),
    /// Summary data points
    Summary(Vec<SampleSummaryDataPoint>),
}

/// Represents a single data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleNumberDataPoint {
    /// The value of the data point
    pub value: Value,
    /// The attributes of the data point
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

/// Represents a single histogram data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleHistogramDataPoint {
    /// The attributes of the data point
    pub attributes: Vec<SampleAttribute>,
    /// The count of the data point
    pub count: u64,
    /// The sum of the data point
    pub sum: Option<f64>,
    /// The bucket counts of the data point
    pub bucket_counts: Vec<u64>,
    /// The minimum of the data point
    pub min: Option<f64>,
    /// The maximum of the data point
    pub max: Option<f64>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

/// Represents a single exponential histogram data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleExponentialHistogramDataPoint {
    /// The attributes of the data point
    pub attributes: Vec<SampleAttribute>,
}

/// Represents a single summary data point of a metric
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSummaryDataPoint {
    /// The attributes of the data point
    pub attributes: Vec<SampleAttribute>,
}

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleMetric {
    /// Metric name.
    pub name: String,
    /// Set of attributes
    //    #[serde(default)]
    //    pub attributes: Vec<SampleAttribute>,
    /// Type of the metric (e.g. gauge, histogram, ...).
    pub instrument: InstrumentSpec,
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
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        // find the metric in the registry
        let semconv_metric = live_checker.find_metric(&self.name);
        if semconv_metric.is_none() {
            result.add_advice(Advice {
                advice_type: MISSING_METRIC_ADVICE_TYPE.to_owned(),
                value: Value::String(self.name.clone()),
                message: "Does not exist in the registry".to_owned(),
                advice_level: AdviceLevel::Violation,
            });
        };
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list =
                advisor.advise(SampleRef::Metric(self), None, semconv_metric.as_ref())?;
            result.add_advice_list(advice_list);
        }
        // Get advice for the data points
        match &mut self.data_points {
            Some(DataPoints::Number(number_data_points)) => {
                for point in number_data_points.iter_mut() {
                    let mut point_result = LiveCheckResult::new();
                    for advisor in live_checker.advisors.iter_mut() {
                        let advice_list = advisor.advise(
                            SampleRef::NumberDataPoint(point),
                            None,
                            semconv_metric.as_ref(),
                        )?;
                        point_result.add_advice_list(advice_list);
                    }
                    point.live_check_result = Some(point_result);
                    stats.inc_entity_count("data_point");
                    stats.maybe_add_live_check_result(point.live_check_result.as_ref());

                    for attribute in &mut point.attributes {
                        attribute.run_live_check(live_checker, stats)?;
                    }
                }
            }
            Some(DataPoints::Histogram(histogram_data_points)) => {
                for point in histogram_data_points.iter_mut() {
                    let mut point_result = LiveCheckResult::new();
                    for advisor in live_checker.advisors.iter_mut() {
                        let advice_list = advisor.advise(
                            SampleRef::HistogramDataPoint(point),
                            None,
                            semconv_metric.as_ref(),
                        )?;
                        point_result.add_advice_list(advice_list);
                    }
                    point.live_check_result = Some(point_result);
                    stats.inc_entity_count("data_point");
                    stats.maybe_add_live_check_result(point.live_check_result.as_ref());

                    for attribute in &mut point.attributes {
                        attribute.run_live_check(live_checker, stats)?;
                    }
                }
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
